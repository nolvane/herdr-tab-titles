//! Turns what a pane knows (directory, branch, program, agent title) into a
//! short title. Everything here is pure, so it is covered by unit tests.

/// Separator between the parts of a title.
pub const SEP: &str = " › ";
/// Separator between the tab position and the title.
pub const POS_SEP: &str = " · ";

/// Shells: a pane running only its shell has no activity to report.
const SHELLS: &[&str] = &[
    "zsh", "bash", "fish", "sh", "dash", "ksh", "tcsh", "csh", "nu", "xonsh", "elvish", "pwsh",
    "login",
];

/// Agent names that say nothing about the work when used as a title.
const AGENT_NAMES: &[&str] = &[
    "claude",
    "claude code",
    "codex",
    "opencode",
    "gemini",
    "grok",
    "pi",
    "π",
    "agy",
    "antigravity",
    "hermes",
    "amp",
    "cursor",
    "aider",
];

/// One process from `pane.process_info`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Process {
    pub pid: Option<i64>,
    pub name: String,
    pub argv: Vec<String>,
    pub cwd: Option<String>,
}

/// Cuts `text` to at most `max` characters, ending in `…` when cut.
pub fn truncate(text: &str, max: usize) -> String {
    let count = text.chars().count();
    if count <= max {
        return text.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let mut out: String = text.chars().take(max - 1).collect();
    out = out.trim_end().to_string();
    out.push('…');
    out
}

/// Removes control characters, collapses whitespace and strips leading status
/// glyphs (spinners, dots, stars) that agents put in front of their titles.
pub fn sanitize(text: &str) -> String {
    let cleaned: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let collapsed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed
        .trim_start_matches(|c: char| !c.is_alphanumeric() && !"[(\"'~/.".contains(c))
        .trim()
        .to_string()
}

/// The short name of a directory: `~` for home, else its last component.
pub fn context_name(dir: &str, home: Option<&str>) -> String {
    let trimmed = dir.trim_end_matches('/');
    if trimmed.is_empty() {
        return "/".to_string();
    }
    if let Some(home) = home {
        if trimmed == home.trim_end_matches('/') {
            return "~".to_string();
        }
    }
    trimmed
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or(trimmed)
        .to_string()
}

/// What an agent's terminal title says about the work, or `None` when it only
/// repeats the agent's name, a directory or a shell prompt.
pub fn agent_activity(agent: &str, title: &str, context: &str) -> Option<String> {
    let mut text = sanitize(title);
    let agent_lower = agent.to_lowercase();

    match agent_lower.as_str() {
        // Codex: `<name> | <project>`; a bare `<project>` has no name yet.
        "codex" => {
            text = text
                .trim_start_matches("[ ! ] Action Required")
                .trim_start_matches("[ . ] Action Required")
                .trim()
                .to_string();
            text = match text.rsplit_once(" | ") {
                Some((name, _project)) => name.trim().to_string(),
                None => return None,
            };
        }
        // OpenCode: `OC | <title>`.
        "opencode" => {
            if let Some(rest) = text.strip_prefix("OC | ") {
                text = rest.trim().to_string();
            }
        }
        // Grok: `<name> - grok`, or `<spinner> - <status> - <name> - grok`.
        "grok" => {
            let parts: Vec<&str> = text.split(" - ").collect();
            if parts.len() >= 2 && parts.last() == Some(&"grok") {
                text = parts[parts.len() - 2].trim().to_string();
            }
        }
        // Pi: `π - <dir>` or `π - <name> - <dir>`.
        "pi" => {
            let parts: Vec<&str> = text.split(" - ").collect();
            text = match parts.as_slice() {
                [_, name, _dir] => name.trim().to_string(),
                [_, _dir] => return None,
                _ => text,
            };
        }
        _ => {}
    }

    let text = sanitize(&text);
    let lower = text.to_lowercase();
    if text.is_empty()
        || lower == agent_lower
        || AGENT_NAMES.contains(&lower.as_str())
        || lower == context.to_lowercase()
        || looks_like_location(&text)
    {
        return None;
    }
    Some(text)
}

/// A shell prompt title (`user@host: ~/dir`) or a bare path.
fn looks_like_location(text: &str) -> bool {
    if text.starts_with('/') || text.starts_with('~') {
        return true;
    }
    if let Some((before, _)) = text.split_once(':') {
        if before.contains('@') && !before.contains(' ') {
            return true;
        }
    }
    false
}

/// Picks the pane's foreground process: the one whose pid is the foreground
/// process group id, else the last listed.
pub fn foreground(processes: &[Process], group_id: Option<i64>) -> Option<&Process> {
    if let Some(gid) = group_id {
        if let Some(p) = processes.iter().find(|p| p.pid == Some(gid)) {
            return Some(p);
        }
    }
    processes.last()
}

fn base_name(path: &str) -> &str {
    path.trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(path)
}

fn is_shell(name: &str) -> bool {
    let name = base_name(name.trim_start_matches('-'));
    SHELLS.contains(&name)
}

/// Options of ssh and mosh that take a value, so the value is not the host.
const SSH_VALUE_OPTS: &[&str] = &[
    "-b", "-c", "-D", "-E", "-e", "-F", "-I", "-i", "-J", "-L", "-l", "-m", "-O", "-o", "-p", "-Q",
    "-R", "-S", "-W", "-w", "-B", "--ssh", "--port", "--server", "--client",
];

/// What a foreground program is doing, such as `nvim main.rs`, `ssh prod-01`
/// or `cargo build`. `None` for a shell at its prompt.
pub fn process_activity(process: &Process) -> Option<String> {
    let mut name = base_name(&process.name).to_string();
    let mut args: Vec<&str> = process.argv.iter().skip(1).map(|s| s.as_str()).collect();

    // `sudo x` is about `x`.
    if name == "sudo" || name == "doas" {
        let rest: Vec<&str> = args
            .iter()
            .copied()
            .skip_while(|a| a.starts_with('-'))
            .collect();
        let (first, tail) = rest.split_first()?;
        name = base_name(first).to_string();
        args = tail.to_vec();
    }

    if name.is_empty() || is_shell(&name) {
        return None;
    }

    if name == "ssh" || name == "mosh" {
        let mut skip_next = false;
        for arg in &args {
            if skip_next {
                skip_next = false;
                continue;
            }
            if SSH_VALUE_OPTS.contains(arg) {
                skip_next = true;
                continue;
            }
            if arg.starts_with('-') {
                continue;
            }
            let host = arg.rsplit('@').next().unwrap_or(arg);
            return Some(format!("{name}{SEP}{host}"));
        }
        return Some(name);
    }

    let first = args
        .iter()
        .find(|a| !a.starts_with('-') && !a.contains('='))
        .map(|a| base_name(a));
    match first {
        Some(arg) if !arg.is_empty() && arg.chars().count() <= 24 => Some(format!("{name} {arg}")),
        _ => Some(name),
    }
}

/// Joins the parts of a title and fits it into `max` characters. The activity
/// is what changes most and says most, so it is kept longest: first the branch
/// goes, then the activity is shortened, then the directory goes.
pub fn compose(
    position: Option<usize>,
    context: &str,
    branch: Option<&str>,
    activity: Option<&str>,
    max: usize,
) -> String {
    let prefix = position
        .map(|p| format!("{p}{POS_SEP}"))
        .unwrap_or_default();
    let budget = max.saturating_sub(prefix.chars().count()).max(1);
    let len = |s: &str| s.chars().count();
    let sep = len(SEP);

    let head_with_branch = match branch {
        Some(b) if !b.is_empty() => format!("{context}{SEP}{b}"),
        _ => context.to_string(),
    };

    let body = match activity {
        None => truncate(&head_with_branch, budget),
        Some(activity) => {
            let full = format!("{head_with_branch}{SEP}{activity}");
            if len(&full) <= budget {
                full
            } else {
                let short = format!("{context}{SEP}{activity}");
                if len(&short) <= budget {
                    short
                } else {
                    let room = budget.saturating_sub(len(context) + sep);
                    if room >= 12 {
                        format!("{context}{SEP}{}", truncate(activity, room))
                    } else {
                        truncate(activity, budget)
                    }
                }
            }
        }
    };
    format!("{prefix}{body}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proc(name: &str, argv: &[&str]) -> Process {
        Process {
            pid: None,
            name: name.into(),
            argv: argv.iter().map(|s| s.to_string()).collect(),
            cwd: None,
        }
    }

    #[test]
    fn context_names() {
        assert_eq!(context_name("/Users/me", Some("/Users/me")), "~");
        assert_eq!(
            context_name("/Users/me/work/dashboard/", Some("/Users/me")),
            "dashboard"
        );
        assert_eq!(context_name("/", None), "/");
    }

    #[test]
    fn sanitize_strips_spinners() {
        assert_eq!(sanitize("✳ Release  coordinator"), "Release coordinator");
        assert_eq!(sanitize("⠙ working\tnow"), "working now");
        assert_eq!(sanitize("~/dir"), "~/dir");
    }

    #[test]
    fn claude_titles() {
        assert_eq!(
            agent_activity("claude", "◐ Fix the login redirect", "x").as_deref(),
            Some("Fix the login redirect")
        );
        assert_eq!(agent_activity("claude", "✳ Claude Code", "x"), None);
        assert_eq!(agent_activity("claude", "", "x"), None);
        assert_eq!(agent_activity("claude", "repo", "repo"), None);
        assert_eq!(agent_activity("claude", "me@host: ~/repo", "repo"), None);
    }

    #[test]
    fn other_agent_frames() {
        assert_eq!(
            agent_activity("codex", "[ ! ] Action Required Fix login | app", "app").as_deref(),
            Some("Fix login")
        );
        assert_eq!(agent_activity("codex", "app", "app"), None);
        assert_eq!(
            agent_activity("opencode", "OC | Add tests", "x").as_deref(),
            Some("Add tests")
        );
        assert_eq!(
            agent_activity("grok", "⠙ - Thinking - Refactor db - grok", "x").as_deref(),
            Some("Refactor db")
        );
        assert_eq!(agent_activity("pi", "π - repo", "repo"), None);
        assert_eq!(
            agent_activity("pi", "π - auth flow - repo", "repo").as_deref(),
            Some("auth flow")
        );
    }

    #[test]
    fn process_activities() {
        assert_eq!(process_activity(&proc("zsh", &["-zsh"])), None);
        assert_eq!(process_activity(&proc("-zsh", &[])), None);
        assert_eq!(
            process_activity(&proc("nvim", &["nvim", "src/main.rs"])).as_deref(),
            Some("nvim main.rs")
        );
        assert_eq!(
            process_activity(&proc("ssh", &["ssh", "-p", "22", "me@prod-01"])).as_deref(),
            Some("ssh › prod-01")
        );
        assert_eq!(
            process_activity(&proc("caffeinate", &["caffeinate", "-d", "-i"])).as_deref(),
            Some("caffeinate")
        );
        assert_eq!(
            process_activity(&proc("sudo", &["sudo", "-E", "vim", "/etc/hosts"])).as_deref(),
            Some("vim hosts")
        );
        assert_eq!(
            process_activity(&proc("cargo", &["cargo", "build", "--release"])).as_deref(),
            Some("cargo build")
        );
    }

    #[test]
    fn picks_foreground_by_group() {
        let mut a = proc("node", &[]);
        a.pid = Some(2);
        let mut b = proc("npm", &[]);
        b.pid = Some(1);
        let list = vec![a, b.clone()];
        assert_eq!(foreground(&list, Some(1)), Some(&b));
        assert_eq!(
            foreground(&list, Some(9)).map(|p| p.name.as_str()),
            Some("npm")
        );
    }

    #[test]
    fn compose_fits() {
        assert_eq!(compose(Some(1), "repo", None, None, 40), "1 · repo");
        assert_eq!(
            compose(Some(2), "repo", Some("ABC-1"), Some("Fix it"), 40),
            "2 · repo › ABC-1 › Fix it"
        );
        // Branch goes first.
        assert_eq!(
            compose(
                Some(2),
                "my-app",
                Some("ABC-123"),
                Some("Login redirect fix"),
                40
            ),
            "2 · my-app › Login redirect fix"
        );
        // Then the activity is cut.
        let t = compose(
            Some(3),
            "my-app",
            None,
            Some("Login redirect and session timeout"),
            40,
        );
        assert!(t.chars().count() <= 40);
        assert!(t.starts_with("3 · my-app › Login redirect"));
        assert!(t.ends_with('…'));
        // Without position.
        assert_eq!(compose(None, "~", None, Some("htop"), 40), "~ › htop");
    }

    #[test]
    fn truncate_counts_chars() {
        assert_eq!(truncate("äöü", 3), "äöü");
        assert_eq!(truncate("abcdef", 4), "abc…");
    }
}

/// Whether `haystack` already names `needle`, ignoring case and punctuation,
/// so a worktree folder `app-abc42` counts as naming branch `ABC-42`.
pub fn mentions(haystack: &str, needle: &str) -> bool {
    let norm = |s: &str| -> String {
        s.chars()
            .filter(|c| c.is_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect()
    };
    let needle = norm(needle);
    !needle.is_empty() && norm(haystack).contains(&needle)
}

#[cfg(test)]
mod mention_tests {
    use super::mentions;

    #[test]
    fn ignores_case_and_punctuation() {
        assert!(mentions("app-abc42", "ABC-42"));
        assert!(mentions("abc-123", "ABC-123"));
        assert!(!mentions("my-app", "ABC-123"));
    }
}
