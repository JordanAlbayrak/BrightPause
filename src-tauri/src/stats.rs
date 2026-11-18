use jiff::civil::{Date, DateTime};
use jiff::{Span, ZonedDateTime};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::Path;

use crate::schedule::{Kind, Outcome};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Entry {
    pub at: DateTime,
    pub kind: Kind,
    pub outcome: Outcome,
}

#[derive(Debug, Default, Serialize, PartialEq)]
pub struct Summary {
    pub taken: u32,
    pub skipped: u32,
    pub postponed: u32,
    pub streak_days: u32,
    pub week: Vec<(String, u32)>,
}

pub fn append(path: &Path, kind: Kind, outcome: Outcome) -> std::io::Result<()> {
    append_at(path, ZonedDateTime::now().datetime(), kind, outcome)
}

pub fn append_at(path: &Path, at: DateTime, kind: Kind, outcome: Outcome) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(f, "{}", serde_json::to_string(&Entry { at, kind, outcome }).unwrap())
}

pub fn summary(path: &Path, today: Date) -> Summary {
    let entries: Vec<Entry> = std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    let taken_on = |d: Date| entries.iter().filter(|e| e.at.date() == d && e.outcome == Outcome::Taken).count() as u32;
    let mut s = Summary::default();
    for e in entries.iter().filter(|e| e.at.date() == today) {
        match e.outcome {
            Outcome::Taken => s.taken += 1,
            Outcome::Skipped => s.skipped += 1,
            Outcome::Postponed => s.postponed += 1,
        }
    }
    let mut d = today;
    while taken_on(d) > 0 {
        s.streak_days += 1;
        d = d - Span::new().days(1);
    }
    s.week = (0..7)
        .rev()
        .map(|i| {
            let d = today - Span::new().days(i);
            (d.to_string(), taken_on(d))
        })
        .collect();
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(d: i8, h: i8) -> DateTime {
        jiff::civil::date(2030, 1, d).at(h, 0, 0, 0)
    }

    #[test]
    fn summary_counts_today_streak_and_week() {
        let path = std::env::temp_dir().join(format!("brightpause-stats-{}.jsonl", std::process::id()));
        std::fs::remove_file(&path).ok();
        for (d, h, o) in [(1, 9, Outcome::Taken), (3, 9, Outcome::Taken), (4, 9, Outcome::Taken), (4, 10, Outcome::Skipped), (4, 11, Outcome::Postponed), (4, 12, Outcome::Taken)] {
            append_at(&path, at(d, h), Kind::Mini, o).unwrap();
        }
        let s = summary(&path, jiff::civil::date(2030, 1, 4));
        assert_eq!((s.taken, s.skipped, s.postponed, s.streak_days), (2, 1, 1, 2));
        assert_eq!(s.week.len(), 7);
        assert_eq!(s.week.last().unwrap().1, 2);
        assert_eq!(summary(&path, jiff::civil::date(2030, 1, 6)).streak_days, 0);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn reads_history_written_by_older_versions() {
        let at: DateTime = serde_json::from_str("\"2026-11-17T08:45:12.123456789\"").unwrap();
        assert_eq!(at.date(), jiff::civil::date(2026, 11, 17));
    }

    #[test]
    fn missing_file_is_empty() {
        let s = summary(Path::new("Z:/no/such/file.jsonl"), jiff::civil::date(2030, 1, 4));
        assert_eq!((s.taken, s.streak_days), (0, 0));
    }
}
