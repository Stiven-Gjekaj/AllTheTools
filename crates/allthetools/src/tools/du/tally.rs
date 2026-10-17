//! The sums of `att du`: the size of each entry down to a depth, with the
//! size of each deeper entry added to the entry above it.
//!
//! The walk of the disk gives each file and folder with its path under the
//! start, and this module does the rest. It makes no system call, so its
//! tests run on every host.

use std::collections::BTreeMap;

use serde::Serialize;

/// The sums of one start path.
#[derive(Debug, Default)]
pub struct Tally {
    depth: usize,
    sums: BTreeMap<Vec<String>, (u64, bool)>,
    pub total: u64,
}

/// One row of the result.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
pub struct Entry {
    /// The path under the start, with `/` between the names.
    pub path: String,
    pub size: u64,
    pub folder: bool,
}

impl Tally {
    /// Sums that keep each entry down to `depth` names under the start.
    pub fn new(depth: usize) -> Self {
        Tally {
            depth,
            ..Tally::default()
        }
    }

    /// Add the size of one file or folder at `names` under the start. An
    /// empty list of names is the start itself.
    pub fn add(&mut self, names: &[String], size: u64, folder: bool) {
        self.total += size;
        let deepest = names.len().min(self.depth);
        for len in 1..=deepest {
            let entry = self.sums.entry(names[..len].to_vec()).or_insert((0, false));
            entry.0 += size;
            // Each name above the last one is a folder.
            entry.1 |= len < names.len() || folder;
        }
    }

    /// The entries, the largest first, and at most `top` of them. A `top`
    /// of zero gives each one.
    pub fn largest(&self, top: usize) -> Vec<Entry> {
        let mut out: Vec<Entry> = self
            .sums
            .iter()
            .map(|(names, (size, folder))| Entry {
                path: names.join("/"),
                size: *size,
                folder: *folder,
            })
            .collect();
        out.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.path.cmp(&b.path)));
        if top > 0 {
            out.truncate(top);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(path: &str) -> Vec<String> {
        path.split('/')
            .filter(|n| !n.is_empty())
            .map(str::to_owned)
            .collect()
    }

    fn sample(depth: usize) -> Tally {
        let mut t = Tally::new(depth);
        t.add(&names(""), 4, true);
        t.add(&names("src"), 4, true);
        t.add(&names("src/main.rs"), 100, false);
        t.add(&names("src/deep/x.rs"), 1000, false);
        t.add(&names("README.md"), 50, false);
        t
    }

    #[test]
    fn a_deeper_entry_adds_to_each_folder_above_it() {
        let t = sample(1);
        assert_eq!(t.total, 1158);
        assert_eq!(
            t.largest(0),
            [
                Entry {
                    path: "src".to_owned(),
                    size: 1104,
                    folder: true
                },
                Entry {
                    path: "README.md".to_owned(),
                    size: 50,
                    folder: false
                },
            ]
        );
    }

    #[test]
    fn a_greater_depth_keeps_more_entries() {
        let t = sample(2);
        let paths: Vec<String> = t.largest(0).into_iter().map(|e| e.path).collect();
        assert_eq!(paths, ["src", "src/deep", "src/main.rs", "README.md"]);
        assert!(t.largest(0)[1].folder, "a name above a file is a folder");
    }

    #[test]
    fn top_keeps_the_largest_only() {
        assert_eq!(sample(2).largest(1).len(), 1);
        assert_eq!(sample(2).largest(1)[0].path, "src");
    }

    #[test]
    fn a_depth_of_zero_gives_only_the_total() {
        let t = sample(0);
        assert!(t.largest(0).is_empty());
        assert_eq!(t.total, 1158);
    }
}
