//! Settings, read once at startup from `config.toml` in the plugin's config
//! directory. Only flat `key = value` lines are understood; anything else is
//! ignored, so a typo never stops the plugin.

use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// How often the session is read, in milliseconds.
    pub poll_ms: u64,
    /// Maximum tab title width in characters, position included.
    pub max_length: usize,
    /// Put the tab's position (the key that switches to it) in front.
    pub show_position: bool,
    /// Show a non-default git branch next to the directory.
    pub show_branch: bool,
    /// Maximum branch width; longer branches are cut.
    pub branch_max: usize,
    /// Also name panes (shown in Herdr's goto panel).
    pub name_panes: bool,
    /// Write a log line for every rename.
    pub debug: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            poll_ms: 1000,
            max_length: 40,
            show_position: true,
            show_branch: true,
            branch_max: 16,
            name_panes: true,
            debug: false,
        }
    }
}

impl Config {
    pub fn load(dir: Option<&Path>) -> Config {
        let Some(dir) = dir else {
            return Config::default();
        };
        match std::fs::read_to_string(dir.join("config.toml")) {
            Ok(text) => Config::parse(&text),
            Err(_) => Config::default(),
        }
    }

    pub fn parse(text: &str) -> Config {
        let mut config = Config::default();
        for raw in text.lines() {
            let line = raw.split('#').next().unwrap_or("").trim();
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let value = value.trim().trim_matches('"');
            match key {
                "poll_ms" => set_num(&mut config.poll_ms, value, 200, 60_000),
                "max_length" => set_num(&mut config.max_length, value, 10, 200),
                "branch_max" => set_num(&mut config.branch_max, value, 4, 80),
                "show_position" => set_bool(&mut config.show_position, value),
                "show_branch" => set_bool(&mut config.show_branch, value),
                "name_panes" => set_bool(&mut config.name_panes, value),
                "debug" => set_bool(&mut config.debug, value),
                _ => {}
            }
        }
        config
    }
}

fn set_num<T>(target: &mut T, value: &str, min: T, max: T)
where
    T: std::str::FromStr + PartialOrd + Copy,
{
    if let Ok(n) = value.parse::<T>() {
        if n >= min && n <= max {
            *target = n;
        }
    }
}

fn set_bool(target: &mut bool, value: &str) {
    match value.to_ascii_lowercase().as_str() {
        "true" | "yes" | "on" | "1" => *target = true,
        "false" | "no" | "off" | "0" => *target = false,
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_keys_and_ignores_rest() {
        let c = Config::parse(
            "poll_ms = 500\nmax_length = 30 # comment\nshow_position = false\nunknown = 1\nbroken line\nbranch_max = 2\n",
        );
        assert_eq!(c.poll_ms, 500);
        assert_eq!(c.max_length, 30);
        assert!(!c.show_position);
        assert_eq!(c.branch_max, 16, "out of range keeps default");
    }
}
