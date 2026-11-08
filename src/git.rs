//! Reads the current git branch straight from `.git/HEAD`, without running git.

use std::path::{Path, PathBuf};

/// The branch checked out in the repository that contains `dir`, if any.
/// Detached heads return `None`.
pub fn branch(dir: &Path) -> Option<String> {
    let git_dir = find_git_dir(dir)?;
    let head = std::fs::read_to_string(git_dir.join("HEAD")).ok()?;
    head.trim()
        .strip_prefix("ref: refs/heads/")
        .map(|b| b.to_string())
}

fn find_git_dir(start: &Path) -> Option<PathBuf> {
    let mut current = Some(start);
    while let Some(dir) = current {
        let candidate = dir.join(".git");
        if candidate.is_dir() {
            return Some(candidate);
        }
        if candidate.is_file() {
            // A worktree or submodule: `.git` is a file naming the real dir.
            let text = std::fs::read_to_string(&candidate).ok()?;
            let target = text.trim().strip_prefix("gitdir:")?.trim();
            let path = Path::new(target);
            return Some(if path.is_absolute() {
                path.to_path_buf()
            } else {
                dir.join(path)
            });
        }
        current = dir.parent();
    }
    None
}

/// Branches that say nothing about the work and are left out of a title.
pub fn is_default_branch(branch: &str) -> bool {
    matches!(
        branch,
        "main" | "master" | "trunk" | "develop" | "development" | "dev" | "default"
    )
}

/// Shortens a branch for a title: a ticket key like `abc-123` wins and is
/// upper-cased, otherwise common prefixes such as `feat/` are dropped and the
/// rest is cut to `max` characters.
pub fn short_branch(branch: &str, max: usize) -> String {
    if let Some(ticket) = ticket_key(branch) {
        return ticket;
    }
    let mut rest = branch;
    for prefix in [
        "feature/",
        "feat/",
        "fix/",
        "bugfix/",
        "hotfix/",
        "chore/",
        "refactor/",
        "docs/",
    ] {
        if let Some(stripped) = rest.strip_prefix(prefix) {
            rest = stripped;
            break;
        }
    }
    crate::title::truncate(rest, max)
}

/// Finds a ticket key (`ABC-123`) in a branch name.
fn ticket_key(branch: &str) -> Option<String> {
    let chars: Vec<char> = branch.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        // A key starts at the beginning or after a separator.
        let at_boundary = i == 0 || !chars[i - 1].is_ascii_alphanumeric();
        if at_boundary && chars[i].is_ascii_alphabetic() {
            let start = i;
            let mut j = i;
            while j < chars.len() && chars[j].is_ascii_alphabetic() {
                j += 1;
            }
            let letters = j - start;
            if (2..=10).contains(&letters) && j < chars.len() && chars[j] == '-' {
                let mut k = j + 1;
                while k < chars.len() && chars[k].is_ascii_digit() {
                    k += 1;
                }
                let digits = k - (j + 1);
                let ends_clean = k == chars.len() || !chars[k].is_ascii_alphanumeric();
                if digits >= 1 && ends_clean {
                    let key: String = chars[start..k].iter().collect();
                    return Some(key.to_ascii_uppercase());
                }
            }
            i = j.max(i + 1);
        } else {
            i += 1;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_ticket_keys() {
        assert_eq!(short_branch("abc-123-login-redirect", 16), "ABC-123");
        assert_eq!(short_branch("feature/MC-13675-x", 16), "MC-13675");
        assert_eq!(
            short_branch("bugfix-asa-cpanel-uapi-mc-13675", 16),
            "MC-13675"
        );
    }

    #[test]
    fn strips_prefix_and_cuts() {
        assert_eq!(short_branch("feat/dark-mode", 16), "dark-mode");
        assert_eq!(
            short_branch("feat/a-very-long-branch-name", 10),
            "a-very-lo…"
        );
        assert_eq!(short_branch("cleanup-old-logs", 16), "cleanup-old-logs");
    }

    #[test]
    fn reads_branch_from_worktree_file() {
        let base = std::env::temp_dir().join(format!("tab-titles-git-{}", std::process::id()));
        let real = base.join("repo-git");
        let work = base.join("work/sub");
        std::fs::create_dir_all(&real).unwrap();
        std::fs::create_dir_all(&work).unwrap();
        std::fs::write(real.join("HEAD"), "ref: refs/heads/abc-1-x\n").unwrap();
        std::fs::write(
            base.join("work/.git"),
            format!("gitdir: {}\n", real.display()),
        )
        .unwrap();
        assert_eq!(branch(&work).as_deref(), Some("abc-1-x"));
        std::fs::remove_dir_all(&base).ok();
    }
}
