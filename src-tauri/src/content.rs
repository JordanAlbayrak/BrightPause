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

    pub fn pick(&mut self, kind: Kind) -> Item {
        let pool: Vec<&Item> = all().iter().filter(|i| i.kind == kind).collect();
        let keep = pool.len() / 2;
        let fresh: Vec<&&Item> = pool.iter().filter(|i| !self.recent.contains(&i.id)).collect();
        let item = (*fresh[(self.next_u64() % fresh.len() as u64) as usize]).clone();
        self.recent.push(item.id.clone());
        let mine: Vec<String> = self.recent.iter().filter(|id| pool.iter().any(|i| &i.id == *id)).cloned().collect();
        if mine.len() > keep {
            let drop = mine[0].clone();
            self.recent.retain(|id| *id != drop);
        }
        item
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
        let picks: Vec<String> = (0..n * 6).map(|_| r.pick(Kind::Mini).id).collect();
        for w in picks.windows(n / 2) {
            let mut v = w.to_vec();
            v.sort();
            v.dedup();
            assert_eq!(v.len(), w.len(), "{w:?}");
        }
        assert!(all().iter().filter(|i| i.kind == Kind::Mini).all(|i| picks.contains(&i.id)));
        assert_eq!(r.pick(Kind::Long).kind, Kind::Long);
    }
}
