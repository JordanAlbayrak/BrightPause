use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

use crate::schedule::Kind;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub kind: Kind,
    #[serde(rename = "type")]
    pub kind_of_item: String,
    pub title: String,
    pub text: String,
    pub source_title: String,
    pub source_url: String,
    #[serde(default)]
    pub eyes: bool,
}

impl Item {
    pub fn label_key(&self) -> &'static str {
        match (self.kind, self.eyes) {
            (Kind::Mini, true) => "kind.eye",
            (Kind::Mini, false) => "kind.short",
            (Kind::Long, _) => "kind.long",
        }
    }
}

pub fn all() -> &'static [Item] {
    static ITEMS: OnceLock<Vec<Item>> = OnceLock::new();
    ITEMS.get_or_init(|| serde_json::from_str(include_str!("../content.json")).expect("content.json"))
}

pub struct Rotation {
    recent: Vec<String>,
    seed: u64,
}

impl Rotation {
    pub fn new(seed: u64) -> Self {
        Rotation { recent: Vec::new(), seed: seed | 1 }
    }

    fn next_u64(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }

    /// The next item of this kind, from the built-in texts plus the user's own (or only the user's
    /// own when `only_own` is on and there are some for this kind).
    pub fn pick(&mut self, kind: Kind, own: &[String], only_own: bool) -> Item {
        let own: Vec<Item> = own.iter().map(|t| own_item(kind, t)).collect();
        let mut pool: Vec<Item> = if only_own && !own.is_empty() { Vec::new() } else { all().iter().filter(|i| i.kind == kind).cloned().collect() };
        pool.extend(own);
        let keep = pool.len() / 2;
        let mut fresh: Vec<&Item> = pool.iter().filter(|i| !self.recent.contains(&i.id)).collect();
        if fresh.is_empty() {
            fresh = pool.iter().collect();
        }
        let item = fresh[(self.next_u64() % fresh.len() as u64) as usize].clone();
        self.recent.push(item.id.clone());
        loop {
            let seen: Vec<String> = self.recent.iter().filter(|id| pool.iter().any(|i| &i.id == *id)).cloned().collect();
            if seen.len() <= keep {
                break;
            }
            self.recent.retain(|id| *id != seen[0]);
        }
        item
    }
}

fn own_item(kind: Kind, text: &str) -> Item {
    let text = text.trim();
    Item {
        id: format!("own:{text}"),
        kind,
        kind_of_item: "message".into(),
        title: text.into(),
        text: String::new(),
        source_title: String::new(),
        source_url: String::new(),
        eyes: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_parses_with_enough_of_each_kind_and_type() {
        let items = all();
        assert!(items.iter().filter(|i| i.kind == Kind::Mini).count() >= 8);
        assert!(items.iter().filter(|i| i.kind == Kind::Long).count() >= 6);
        for t in ["message", "tip", "challenge"] {
            assert!(items.iter().any(|i| i.kind_of_item == t), "{t}");
        }
        for i in items {
            assert!(i.text.len() <= 140, "{} too long", i.id);
            assert_eq!(i.source_title.is_empty(), i.source_url.is_empty(), "{}", i.id);
            assert!(i.source_url.is_empty() || i.source_url.starts_with("https://"));
            if i.kind_of_item == "tip" {
                assert!(!i.source_url.is_empty(), "tip {} needs a source", i.id);
            }
        }
        for id in ["far-look", "blink", "window"] {
            assert_eq!(items.iter().find(|i| i.id == id).unwrap().label_key(), "kind.eye", "{id}");
        }
        for id in ["hands", "shoulders", "neck", "posture", "breaths"] {
            assert_eq!(items.iter().find(|i| i.id == id).unwrap().label_key(), "kind.short", "{id}");
        }
        let mut ids: Vec<_> = items.iter().map(|i| &i.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), items.len());
    }

    #[test]
    fn rotation_never_repeats_within_half_the_pool() {
        let mut r = Rotation::new(42);
        let n = all().iter().filter(|i| i.kind == Kind::Mini).count();
        let picks: Vec<String> = (0..n * 6).map(|_| r.pick(Kind::Mini, &[], false).id).collect();
        for w in picks.windows(n / 2) {
            let mut v = w.to_vec();
            v.sort();
            v.dedup();
            assert_eq!(v.len(), w.len(), "{w:?}");
        }
        assert!(all().iter().filter(|i| i.kind == Kind::Mini).all(|i| picks.contains(&i.id)));
        assert_eq!(r.pick(Kind::Long, &[], false).kind, Kind::Long);
    }

    #[test]
    fn own_texts_join_the_rotation_for_their_kind() {
        let mut r = Rotation::new(7);
        let own = vec!["Drink some water".to_string(), "  Look at the plant  ".to_string()];
        let n = all().iter().filter(|i| i.kind == Kind::Mini).count() + own.len();
        let picks: Vec<Item> = (0..n * 4).map(|_| r.pick(Kind::Mini, &own, false)).collect();
        assert!(picks.iter().any(|i| i.title == "Drink some water" && i.text.is_empty()));
        assert!(picks.iter().any(|i| i.title == "Look at the plant"));
        assert!(picks.iter().any(|i| !i.id.starts_with("own:")));
        assert!(picks.iter().all(|i| i.kind == Kind::Mini));
    }

    #[test]
    fn only_mine_shows_just_the_own_texts_and_falls_back_when_there_are_none() {
        let mut r = Rotation::new(9);
        let own = vec!["Refill the water".to_string(), "Walk to the window".to_string(), "Ten squats".to_string()];
        for _ in 0..20 {
            assert!(r.pick(Kind::Long, &own, true).id.starts_with("own:"));
        }
        assert!(!r.pick(Kind::Mini, &[], true).id.starts_with("own:"));
    }

    #[test]
    fn switching_to_a_smaller_pool_never_runs_dry() {
        let mut r = Rotation::new(3);
        let own: Vec<String> = (0..10).map(|i| format!("text {i}")).collect();
        for _ in 0..30 {
            r.pick(Kind::Mini, &own, true);
        }
        let one = vec!["text 1".to_string()];
        for _ in 0..5 {
            assert_eq!(r.pick(Kind::Mini, &one, true).title, "text 1");
        }
    }
}
