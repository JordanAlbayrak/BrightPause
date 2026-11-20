use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Mini,
    Long,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    Taken,
    Skipped,
    Postponed,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Timing {
    pub mini_every: f64,
    pub mini_length: f64,
    pub long_every_nth: u32,
    pub long_length: f64,
    pub idle_reset: f64,
    pub heads_up_mini: f64,
    pub heads_up_long: f64,
    pub postpone_mini: f64,
    pub postpone_long: f64,
    pub postpone_limit: u32,
}

impl Default for Timing {
    fn default() -> Self {
        Timing {
            mini_every: 20.0 * 60.0,
            mini_length: 20.0,
            long_every_nth: 3,
            long_length: 5.0 * 60.0,
            idle_reset: 5.0 * 60.0,
            heads_up_mini: 10.0,
            heads_up_long: 30.0,
            postpone_mini: 2.0 * 60.0,
            postpone_long: 5.0 * 60.0,
            postpone_limit: 1,
        }
    }
}

impl Timing {
    pub fn length(&self, kind: Kind) -> f64 {
        match kind {
            Kind::Mini => self.mini_length,
            Kind::Long => self.long_length,
        }
    }
    fn heads_up(&self, kind: Kind) -> f64 {
        match kind {
            Kind::Mini => self.heads_up_mini,
            Kind::Long => self.heads_up_long,
        }
    }
    pub fn postpone(&self, kind: Kind) -> f64 {
        match kind {
            Kind::Mini => self.postpone_mini,
            Kind::Long => self.postpone_long,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    Working { since: f64, due: f64, kind: Kind, heads_up_sent: bool, postponed: u32, idle: bool },
    Break { kind: Kind, started: f64, ends: f64, postponed: u32 },
    Paused { until: Option<f64> },
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    HeadsUp { kind: Kind, seconds: u32 },
    /// The break waits this long because the screen is busy or a call is on.
    Held { kind: Kind, seconds: u32 },
    BreakStarted { kind: Kind, length: f64, can_postpone: bool },
    BreakEnded { kind: Kind, outcome: Outcome },
    IdleReset,
    Paused { until: Option<f64> },
    Resumed,
}

pub struct Scheduler {
    pub timing: Timing,
    pub phase: Phase,
    minis_since_long: u32,
    last_tick: f64,
}

const SLEEP_GAP: f64 = 60.0;
const BUSY_RETRY: f64 = 60.0;

impl Scheduler {
    pub fn new(timing: Timing, now: f64) -> Self {
        let mut s = Scheduler { timing, phase: Phase::Paused { until: None }, minis_since_long: 0, last_tick: now };
        s.start_working(now, 0);
        s
    }

    pub fn next_kind(&self) -> Kind {
        if self.timing.long_every_nth > 0 && self.minis_since_long + 1 >= self.timing.long_every_nth {
            Kind::Long
        } else {
            Kind::Mini
        }
    }

    fn start_working(&mut self, now: f64, postponed: u32) {
        let kind = self.next_kind();
        self.phase = Phase::Working { since: now, due: now + self.timing.mini_every, kind, heads_up_sent: false, postponed, idle: false };
    }

    fn count(&mut self, kind: Kind) {
        match kind {
            Kind::Mini => self.minis_since_long += 1,
            Kind::Long => self.minis_since_long = 0,
        }
    }

    pub fn tick(&mut self, now: f64, idle_seconds: f64) -> Vec<Event> {
        self.tick_busy(now, idle_seconds, false)
    }

    pub fn tick_busy(&mut self, now: f64, idle_seconds: f64, busy: bool) -> Vec<Event> {
        let mut events = Vec::new();
        let gap = now - self.last_tick;
        self.last_tick = now;
        if gap > SLEEP_GAP {
            if let Phase::Break { kind, .. } = self.phase {
                events.push(Event::BreakEnded { kind, outcome: Outcome::Taken });
            }
            if !matches!(self.phase, Phase::Paused { .. }) {
                self.minis_since_long = 0;
                self.start_working(now, 0);
                events.push(Event::IdleReset);
                return events;
            }
        }
        match self.phase.clone() {
            Phase::Paused { until: Some(until) } if now >= until => {
                self.start_working(now, 0);
                events.push(Event::Resumed);
            }
            Phase::Paused { .. } => {}
            Phase::Working { due, kind, heads_up_sent, postponed, idle, .. } => {
                if idle_seconds >= self.timing.idle_reset {
                    if !idle {
                        events.push(Event::IdleReset);
                    }
                    self.minis_since_long = 0;
                    self.start_working(now, 0);
                    if let Phase::Working { idle, .. } = &mut self.phase {
                        *idle = true;
                    }
                    return events;
                }
                if idle {
                    if let Phase::Working { idle, .. } = &mut self.phase {
                        *idle = false;
                    }
                }
                if busy && due - now <= self.timing.heads_up(kind) {
                    let wait = self.timing.heads_up(kind) + BUSY_RETRY;
                    if let Phase::Working { due, .. } = &mut self.phase {
                        *due = now + wait;
                    }
                    events.push(Event::Held { kind, seconds: wait as u32 });
                    return events;
                }
                if now >= due {
                    let length = self.timing.length(kind);
                    self.phase = Phase::Break { kind, started: now, ends: now + length, postponed };
                    events.push(Event::BreakStarted { kind, length, can_postpone: postponed < self.timing.postpone_limit });
                } else if !heads_up_sent && due - now <= self.timing.heads_up(kind) {
                    if let Phase::Working { heads_up_sent, .. } = &mut self.phase {
                        *heads_up_sent = true;
                    }
                    events.push(Event::HeadsUp { kind, seconds: (due - now).ceil() as u32 });
                }
            }
            Phase::Break { kind, ends, .. } => {
                if now >= ends {
                    events.extend(self.end_break(now, Outcome::Taken));
                }
                let _ = kind;
            }
        }
        events
    }

    pub fn end_break(&mut self, now: f64, outcome: Outcome) -> Vec<Event> {
        let Phase::Break { kind, postponed, .. } = self.phase else { return vec![] };
        match outcome {
            Outcome::Postponed => {
                if postponed >= self.timing.postpone_limit {
                    return vec![];
                }
                let delay = self.timing.postpone(kind);
                self.phase = Phase::Working { since: now, due: now + delay, kind, heads_up_sent: true, postponed: postponed + 1, idle: false };
            }
            Outcome::Taken | Outcome::Skipped => {
                self.count(kind);
                self.start_working(now, 0);
            }
        }
        vec![Event::BreakEnded { kind, outcome }]
    }

    /// True from the heads-up until the break starts.
    pub fn in_heads_up(&self, now: f64) -> bool {
        matches!(self.phase, Phase::Working { due, kind, heads_up_sent: true, .. } if due > now && due - now <= self.timing.heads_up(kind))
    }

    pub fn can_snooze(&self) -> bool {
        matches!(self.phase, Phase::Working { postponed, .. } if postponed < self.timing.postpone_limit)
    }

    /// Snooze from the heads-up: the same delay as postponing the break, and it counts toward the limit.
    pub fn snooze(&mut self, now: f64) -> Vec<Event> {
        let Phase::Working { kind, postponed, .. } = self.phase else { return vec![] };
        if !self.in_heads_up(now) || !self.can_snooze() {
            return vec![];
        }
        let due = now + self.timing.postpone(kind);
        self.phase = Phase::Working { since: now, due, kind, heads_up_sent: false, postponed: postponed + 1, idle: false };
        vec![Event::BreakEnded { kind, outcome: Outcome::Postponed }]
    }

    /// Skip from the heads-up: this break only; the next one comes after the usual interval.
    pub fn skip_upcoming(&mut self, now: f64) -> Vec<Event> {
        let Phase::Working { kind, .. } = self.phase else { return vec![] };
        if !self.in_heads_up(now) {
            return vec![];
        }
        self.count(kind);
        self.start_working(now, 0);
        vec![Event::BreakEnded { kind, outcome: Outcome::Skipped }]
    }

    pub fn break_now(&mut self, now: f64) -> Vec<Event> {
        if matches!(self.phase, Phase::Break { .. }) {
            return vec![];
        }
        let kind = self.next_kind();
        let length = self.timing.length(kind);
        self.phase = Phase::Break { kind, started: now, ends: now + length, postponed: self.timing.postpone_limit };
        vec![Event::BreakStarted { kind, length, can_postpone: false }]
    }

    pub fn reset(&mut self, now: f64) {
        if matches!(self.phase, Phase::Working { .. }) {
            self.start_working(now, 0);
        }
    }

    pub fn pause(&mut self, now: f64, until: Option<f64>) -> Vec<Event> {
        let mut events = Vec::new();
        if let Phase::Break { kind, .. } = self.phase {
            events.push(Event::BreakEnded { kind, outcome: Outcome::Skipped });
        }
        let _ = now;
        self.phase = Phase::Paused { until };
        events.push(Event::Paused { until });
        events
    }

    pub fn resume(&mut self, now: f64) -> Vec<Event> {
        if !matches!(self.phase, Phase::Paused { .. }) {
            return vec![];
        }
        self.start_working(now, 0);
        vec![Event::Resumed]
    }

    pub fn set_timing(&mut self, timing: Timing) {
        self.timing = timing;
        let next = self.next_kind();
        if let Phase::Working { since, due, kind, postponed, heads_up_sent, .. } = &mut self.phase {
            if *postponed == 0 {
                *kind = next;
                *due = *since + self.timing.mini_every;
                *heads_up_sent = false;
            }
        }
    }

    pub fn seconds_to_next(&self, now: f64) -> Option<(Kind, f64)> {
        match self.phase {
            Phase::Working { due, kind, .. } => Some((kind, (due - now).max(0.0))),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(s: &mut Scheduler, from: f64, to: f64) -> Vec<Event> {
        let mut out = Vec::new();
        let mut t = from;
        while t <= to {
            out.extend(s.tick(t, 0.0));
            t += 1.0;
        }
        out
    }

    fn started(events: &[Event]) -> Vec<Kind> {
        events.iter().filter_map(|e| match e { Event::BreakStarted { kind, .. } => Some(*kind), _ => None }).collect()
    }

    #[test]
    fn first_break_is_mini_after_twenty_minutes() {
        let mut s = Scheduler::new(Timing::default(), 0.0);
        assert!(started(&run(&mut s, 0.0, 1199.0)).is_empty());
        assert_eq!(started(&s.tick(1200.0, 0.0)), vec![Kind::Mini]);
    }

    #[test]
    fn heads_up_ten_seconds_before_mini_and_only_once() {
        let mut s = Scheduler::new(Timing::default(), 0.0);
        let ev = run(&mut s, 0.0, 1199.0);
        let ups: Vec<_> = ev.iter().filter(|e| matches!(e, Event::HeadsUp { .. })).collect();
        assert_eq!(ups, vec![&Event::HeadsUp { kind: Kind::Mini, seconds: 10 }]);
    }

    #[test]
    fn break_ends_taken_after_its_length_and_every_third_is_long() {
        let mut s = Scheduler::new(Timing::default(), 0.0);
        let mut kinds = Vec::new();
        let mut t = 0.0;
        while kinds.len() < 6 {
            for e in s.tick(t, 0.0) {
                if let Event::BreakStarted { kind, .. } = e {
                    kinds.push(kind);
                }
            }
            t += 1.0;
        }
        assert_eq!(kinds, vec![Kind::Mini, Kind::Mini, Kind::Long, Kind::Mini, Kind::Mini, Kind::Long]);
    }

    #[test]
    fn mini_break_lasts_twenty_seconds() {
        let mut s = Scheduler::new(Timing::default(), 0.0);
        run(&mut s, 0.0, 1200.0);
        assert!(s.tick(1219.0, 0.0).is_empty());
        assert_eq!(s.tick(1220.0, 0.0), vec![Event::BreakEnded { kind: Kind::Mini, outcome: Outcome::Taken }]);
        assert_eq!(s.seconds_to_next(1220.0), Some((Kind::Mini, 1200.0)));
    }

    #[test]
    fn postpone_limit_two_allows_two_snoozes_then_no_more() {
        let mut s = Scheduler::new(Timing { postpone_limit: 2, ..Timing::default() }, 0.0);
        let ev = run(&mut s, 0.0, 1200.0);
        assert_eq!(ev.last(), Some(&Event::BreakStarted { kind: Kind::Mini, length: 20.0, can_postpone: true }));
        assert!(!s.end_break(1201.0, Outcome::Postponed).is_empty());
        let ev = run(&mut s, 1202.0, 1321.0);
        assert_eq!(started(&ev), vec![Kind::Mini]);
        assert_eq!(ev.last(), Some(&Event::BreakStarted { kind: Kind::Mini, length: 20.0, can_postpone: true }));
        assert!(!s.end_break(1322.0, Outcome::Postponed).is_empty());
        let ev = run(&mut s, 1323.0, 1442.0);
        assert_eq!(ev.last(), Some(&Event::BreakStarted { kind: Kind::Mini, length: 20.0, can_postpone: false }));
        assert!(s.end_break(1443.0, Outcome::Postponed).is_empty());
    }

    #[test]
    fn postpone_once_then_not_again() {
        let mut s = Scheduler::new(Timing::default(), 0.0);
        run(&mut s, 0.0, 1200.0);
        assert_eq!(s.end_break(1205.0, Outcome::Postponed), vec![Event::BreakEnded { kind: Kind::Mini, outcome: Outcome::Postponed }]);
        assert_eq!(s.seconds_to_next(1205.0), Some((Kind::Mini, 120.0)));
        let ev = run(&mut s, 1206.0, 1325.0);
        assert_eq!(ev, vec![Event::BreakStarted { kind: Kind::Mini, length: 20.0, can_postpone: false }]);
        assert!(s.end_break(1326.0, Outcome::Postponed).is_empty());
        assert_eq!(s.end_break(1327.0, Outcome::Skipped), vec![Event::BreakEnded { kind: Kind::Mini, outcome: Outcome::Skipped }]);
    }

    #[test]
    fn five_minutes_idle_resets_the_timer_once() {
        let mut s = Scheduler::new(Timing::default(), 0.0);
        run(&mut s, 0.0, 1000.0);
        assert_eq!(s.tick(1001.0, 300.0), vec![Event::IdleReset]);
        for t in 1002..=1500 {
            assert!(s.tick(t as f64, (t - 701) as f64).is_empty(), "{t}");
        }
        s.tick(1501.0, 0.0);
        assert_eq!(s.seconds_to_next(1501.0), Some((Kind::Mini, 1199.0)));
    }

    #[test]
    fn sleep_gap_resets() {
        let mut s = Scheduler::new(Timing::default(), 0.0);
        run(&mut s, 0.0, 500.0);
        assert_eq!(s.tick(4000.0, 0.0), vec![Event::IdleReset]);
        assert_eq!(s.seconds_to_next(4000.0), Some((Kind::Mini, 1200.0)));
    }

    #[test]
    fn pause_until_then_resume_by_itself() {
        let mut s = Scheduler::new(Timing::default(), 0.0);
        s.pause(10.0, Some(1810.0));
        assert!(run(&mut s, 10.0, 1809.0).is_empty());
        assert_eq!(s.tick(1810.0, 0.0), vec![Event::Resumed]);
        assert_eq!(s.seconds_to_next(1810.0), Some((Kind::Mini, 1200.0)));
    }

    #[test]
    fn pause_indefinitely_ignores_time_until_resume() {
        let mut s = Scheduler::new(Timing::default(), 0.0);
        s.pause(0.0, None);
        assert!(run(&mut s, 1.0, 5000.0).is_empty());
        assert_eq!(s.resume(5001.0), vec![Event::Resumed]);
    }

    #[test]
    fn pausing_during_a_break_ends_it_as_skipped() {
        let mut s = Scheduler::new(Timing::default(), 0.0);
        run(&mut s, 0.0, 1200.0);
        let ev = s.pause(1201.0, None);
        assert_eq!(ev[0], Event::BreakEnded { kind: Kind::Mini, outcome: Outcome::Skipped });
    }

    #[test]
    fn break_now_starts_next_kind_without_postpone() {
        let mut s = Scheduler::new(Timing::default(), 0.0);
        assert_eq!(s.break_now(5.0), vec![Event::BreakStarted { kind: Kind::Mini, length: 20.0, can_postpone: false }]);
        assert!(s.break_now(6.0).is_empty());
    }

    #[test]
    fn new_timing_applies_right_away() {
        let mut s = Scheduler::new(Timing::default(), 0.0);
        run(&mut s, 0.0, 100.0);
        s.set_timing(Timing { mini_every: 600.0, ..Timing::default() });
        assert_eq!(s.seconds_to_next(100.0), Some((Kind::Mini, 500.0)));
    }

    #[test]
    fn busy_screen_defers_the_break_until_it_is_free() {
        let mut s = Scheduler::new(Timing::default(), 0.0);
        let mut starts = Vec::new();
        for t in 0..=1500 {
            for e in s.tick_busy(t as f64, 0.0, t < 1400) {
                if matches!(e, Event::BreakStarted { .. }) {
                    starts.push(t);
                }
            }
        }
        assert_eq!(starts.len(), 1, "{starts:?}");
        assert!((1400..=1470).contains(&starts[0]), "{starts:?}");
    }

    #[test]
    fn a_held_break_says_so_each_time_it_waits() {
        let mut s = Scheduler::new(Timing::default(), 0.0);
        run(&mut s, 0.0, 1189.0);
        assert_eq!(s.tick_busy(1190.0, 0.0, true), vec![Event::Held { kind: Kind::Mini, seconds: 70 }]);
        let mut held = 0;
        for t in 1191..=1400 {
            held += s.tick_busy(t as f64, 0.0, true).iter().filter(|e| matches!(e, Event::Held { .. })).count();
        }
        assert_eq!(held, 3);
    }

    #[test]
    fn snooze_from_the_heads_up_delays_by_the_postpone_time_and_warns_again() {
        let mut s = Scheduler::new(Timing { postpone_limit: 2, ..Timing::default() }, 0.0);
        assert!(s.snooze(1185.0).is_empty(), "no snooze before the warning");
        run(&mut s, 0.0, 1191.0);
        assert!(s.in_heads_up(1191.0));
        assert_eq!(s.snooze(1191.0), vec![Event::BreakEnded { kind: Kind::Mini, outcome: Outcome::Postponed }]);
        assert!(!s.in_heads_up(1191.0));
        assert_eq!(s.seconds_to_next(1191.0), Some((Kind::Mini, 120.0)));
        let ev = run(&mut s, 1192.0, 1311.0);
        assert_eq!(ev, vec![Event::HeadsUp { kind: Kind::Mini, seconds: 10 }, Event::BreakStarted { kind: Kind::Mini, length: 20.0, can_postpone: true }]);
    }

    #[test]
    fn snoozes_count_toward_the_postpone_limit() {
        let mut s = Scheduler::new(Timing { postpone_limit: 2, ..Timing::default() }, 0.0);
        run(&mut s, 0.0, 1191.0);
        assert!(!s.snooze(1191.0).is_empty());
        run(&mut s, 1192.0, 1302.0);
        assert!(!s.snooze(1302.0).is_empty());
        run(&mut s, 1303.0, 1413.0);
        assert!(s.in_heads_up(1413.0) && !s.can_snooze());
        assert!(s.snooze(1413.0).is_empty());
        let ev = run(&mut s, 1414.0, 1423.0);
        assert_eq!(ev, vec![Event::BreakStarted { kind: Kind::Mini, length: 20.0, can_postpone: false }]);
    }

    #[test]
    fn strict_mode_has_no_snooze() {
        let mut s = Scheduler::new(Timing { postpone_limit: 0, ..Timing::default() }, 0.0);
        run(&mut s, 0.0, 1191.0);
        assert!(!s.can_snooze());
        assert!(s.snooze(1191.0).is_empty());
    }

    #[test]
    fn skip_from_the_heads_up_skips_this_break_only() {
        let mut s = Scheduler::new(Timing::default(), 0.0);
        assert!(s.skip_upcoming(100.0).is_empty(), "no skip before the warning");
        run(&mut s, 0.0, 1195.0);
        assert_eq!(s.skip_upcoming(1195.0), vec![Event::BreakEnded { kind: Kind::Mini, outcome: Outcome::Skipped }]);
        assert_eq!(s.seconds_to_next(1195.0), Some((Kind::Mini, 1200.0)));
        assert!(started(&run(&mut s, 1196.0, 2394.0)).is_empty());
        assert_eq!(started(&s.tick(2395.0, 0.0)), vec![Kind::Mini]);
    }

    #[test]
    fn a_skipped_break_still_counts_toward_the_movement_break() {
        let mut s = Scheduler::new(Timing::default(), 0.0);
        run(&mut s, 0.0, 1195.0);
        s.skip_upcoming(1195.0);
        run(&mut s, 1196.0, 2395.0);
        s.end_break(2396.0, Outcome::Taken);
        assert_eq!(s.next_kind(), Kind::Long);
    }

    #[test]
    fn the_heads_up_window_ends_when_a_call_defers_the_break() {
        let mut s = Scheduler::new(Timing::default(), 0.0);
        run(&mut s, 0.0, 1191.0);
        assert!(s.in_heads_up(1192.0));
        s.tick_busy(1192.0, 0.0, true);
        assert!(!s.in_heads_up(1192.0));
        assert!(s.skip_upcoming(1192.0).is_empty());
    }

    #[test]
    fn long_every_nth_one_means_only_long_breaks() {
        let s = Scheduler::new(Timing { long_every_nth: 1, ..Timing::default() }, 0.0);
        assert_eq!(s.next_kind(), Kind::Long);
    }
}
