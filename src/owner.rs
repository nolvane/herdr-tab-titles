//! Manual rename protection: decides whether a label may be changed.
//!
//! A tab or pane the user named keeps its name. The plugin only changes labels
//! that are Herdr's default (the tab position, or empty) or that it set itself.
//! Clearing a name (back to the default) hands it back to the plugin.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;

/// How many labels the plugin remembers having set, across restarts.
const CLAIM_LIMIT: usize = 2000;

/// Labels the plugin has set, remembered on disk so a plugin restart does not
/// mistake its own earlier work for the user's.
pub struct Claims {
    order: VecDeque<String>,
    set: HashSet<String>,
    path: Option<PathBuf>,
    dirty: bool,
}

impl Claims {
    pub fn load(path: Option<PathBuf>) -> Claims {
        let mut claims = Claims {
            order: VecDeque::new(),
            set: HashSet::new(),
            path,
            dirty: false,
        };
        if let Some(p) = &claims.path {
            if let Ok(text) = std::fs::read_to_string(p) {
                if let Ok(serde_json::Value::Array(items)) = serde_json::from_str(&text) {
                    for item in items {
                        if let Some(s) = item.as_str() {
                            claims.add(s);
                        }
                    }
                }
            }
        }
        claims.dirty = false;
        claims
    }

    pub fn contains(&self, label: &str) -> bool {
        self.set.contains(label)
    }

    pub fn add(&mut self, label: &str) {
        if self.set.contains(label) {
            return;
        }
        self.set.insert(label.to_string());
        self.order.push_back(label.to_string());
        while self.order.len() > CLAIM_LIMIT {
            if let Some(old) = self.order.pop_front() {
                self.set.remove(&old);
            }
        }
        self.dirty = true;
    }

    pub fn save(&mut self) {
        if !self.dirty {
            return;
        }
        if let Some(p) = &self.path {
            let list: Vec<&String> = self.order.iter().collect();
            let tmp = p.with_extension("tmp");
            if std::fs::write(&tmp, serde_json::to_string(&list).unwrap_or_default()).is_ok() {
                let _ = std::fs::rename(&tmp, p);
            }
        }
        self.dirty = false;
    }
}

/// Per-object state for one kind (tabs or panes).
#[derive(Default)]
pub struct Owner {
    /// Last label seen on or set for each id.
    seen: HashMap<String, String>,
    /// Ids the user named; left alone until the name is cleared.
    locked: HashSet<String>,
}

impl Owner {
    /// Returns the label to set, or `None` to leave the object as it is.
    pub fn decide(
        &mut self,
        id: &str,
        current: &str,
        is_default: bool,
        desired: &str,
        claims: &Claims,
    ) -> Option<String> {
        if is_default {
            self.locked.remove(id);
            self.seen.insert(id.to_string(), current.to_string());
            return (current != desired).then(|| desired.to_string());
        }
        if self.locked.contains(id) {
            return None;
        }
        if current == desired {
            self.seen.insert(id.to_string(), current.to_string());
            return None;
        }
        let ours =
            self.seen.get(id).map(|s| s == current).unwrap_or(false) || claims.contains(current);
        if ours {
            return Some(desired.to_string());
        }
        self.locked.insert(id.to_string());
        None
    }

    /// Records a rename the plugin made.
    pub fn applied(&mut self, id: &str, label: &str, claims: &mut Claims) {
        self.seen.insert(id.to_string(), label.to_string());
        claims.add(label);
    }

    /// Forgets objects that no longer exist.
    pub fn retain(&mut self, live: &HashSet<String>) {
        self.seen.retain(|id, _| live.contains(id));
        self.locked.retain(|id| live.contains(id));
    }

    pub fn locked_count(&self) -> usize {
        self.locked.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claims() -> Claims {
        Claims::load(None)
    }

    #[test]
    fn default_label_is_renamed() {
        let mut o = Owner::default();
        let c = claims();
        assert_eq!(
            o.decide("t1", "1", true, "1 · repo", &c).as_deref(),
            Some("1 · repo")
        );
    }

    #[test]
    fn manual_name_is_kept_until_cleared() {
        let mut o = Owner::default();
        let mut c = claims();
        // Plugin names the tab.
        assert!(o.decide("t1", "1", true, "1 · repo", &c).is_some());
        o.applied("t1", "1 · repo", &mut c);
        // User renames it.
        assert_eq!(o.decide("t1", "Build", false, "1 · repo › x", &c), None);
        assert_eq!(o.decide("t1", "Build", false, "1 · repo › y", &c), None);
        // User clears it: the plugin takes over again.
        assert_eq!(
            o.decide("t1", "", true, "1 · repo", &c).as_deref(),
            Some("1 · repo")
        );
    }

    #[test]
    fn existing_manual_names_survive_startup() {
        let mut o = Owner::default();
        let c = claims();
        assert_eq!(o.decide("t9", "Coordinator", false, "1 · repo", &c), None);
    }

    #[test]
    fn own_labels_from_before_a_restart_are_recognised() {
        let mut c = claims();
        c.add("2 · repo › old topic");
        let mut o = Owner::default();
        assert_eq!(
            o.decide("t2", "2 · repo › old topic", false, "2 · repo › new", &c)
                .as_deref(),
            Some("2 · repo › new")
        );
    }

    #[test]
    fn own_label_follows_changes() {
        let mut o = Owner::default();
        let mut c = claims();
        o.decide("t1", "1", true, "1 · a", &c);
        o.applied("t1", "1 · a", &mut c);
        assert_eq!(
            o.decide("t1", "1 · a", false, "1 · a › b", &c).as_deref(),
            Some("1 · a › b")
        );
    }

    #[test]
    fn claims_are_capped_and_persisted() {
        let path =
            std::env::temp_dir().join(format!("tab-titles-claims-{}.json", std::process::id()));
        let mut c = Claims::load(Some(path.clone()));
        for i in 0..(CLAIM_LIMIT + 5) {
            c.add(&format!("l{i}"));
        }
        c.save();
        let again = Claims::load(Some(path.clone()));
        assert!(!again.contains("l0"));
        assert!(again.contains(&format!("l{}", CLAIM_LIMIT + 4)));
        std::fs::remove_file(&path).ok();
    }
}
