//! One poll: read the session, work out each tab's and pane's title, rename
//! what changed and is not protected.

use crate::config::Config;
use crate::herdr::Client;
use crate::owner::{Claims, Owner};
use crate::title::{self, Process};
use crate::{git, log};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub struct App {
    pub config: Config,
    pub client: Client,
    pub claims: Claims,
    tabs: Owner,
    panes: Owner,
    home: Option<String>,
}

/// What one pane says about itself.
#[derive(Debug, Clone)]
struct PaneTitle {
    context: String,
    branch: Option<String>,
    activity: Option<String>,
}

fn s<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

impl App {
    pub fn new(config: Config, client: Client, claims: Claims) -> App {
        App {
            config,
            client,
            claims,
            tabs: Owner::default(),
            panes: Owner::default(),
            home: std::env::var("HOME").ok(),
        }
    }

    pub fn tick(&mut self) -> Result<(), String> {
        let snapshot = self.client.snapshot()?;
        let empty = Vec::new();
        let panes = snapshot
            .get("panes")
            .and_then(Value::as_array)
            .unwrap_or(&empty);
        let tabs = snapshot
            .get("tabs")
            .and_then(Value::as_array)
            .unwrap_or(&empty);
        let layouts = snapshot
            .get("layouts")
            .and_then(Value::as_array)
            .unwrap_or(&empty);

        let focused_in_tab: HashMap<&str, &str> = layouts
            .iter()
            .filter_map(|l| Some((s(l, "tab_id")?, s(l, "focused_pane_id")?)))
            .collect();

        // Titles for every pane, computed once per poll.
        let mut titles: HashMap<String, PaneTitle> = HashMap::new();
        for pane in panes {
            if let Some(id) = s(pane, "pane_id") {
                if let Some(t) = self.pane_title(pane) {
                    titles.insert(id.to_string(), t);
                }
            }
        }

        // Tabs.
        let mut position_in_ws: HashMap<&str, usize> = HashMap::new();
        let mut live_tabs = HashSet::new();
        for tab in tabs {
            let (Some(tab_id), Some(ws)) = (s(tab, "tab_id"), s(tab, "workspace_id")) else {
                continue;
            };
            live_tabs.insert(tab_id.to_string());
            let position = {
                let p = position_in_ws.entry(ws).or_insert(0);
                *p += 1;
                *p
            };
            let label = s(tab, "label").unwrap_or("");
            let Some(speaker) = speaking_pane(tab_id, panes, &focused_in_tab) else {
                continue;
            };
            let Some(pt) = titles.get(speaker) else {
                continue;
            };
            let desired = title::compose(
                self.config.show_position.then_some(position),
                &pt.context,
                pt.branch.as_deref(),
                pt.activity.as_deref(),
                self.config.max_length,
            );
            let is_default = label.is_empty() || label == position.to_string();
            if let Some(new_label) =
                self.tabs
                    .decide(tab_id, label, is_default, &desired, &self.claims)
            {
                match self.client.rename_tab(tab_id, &new_label) {
                    Ok(()) => {
                        self.tabs.applied(tab_id, &new_label, &mut self.claims);
                        if self.config.debug {
                            log::line(&format!("tab {tab_id}: {label:?} -> {new_label:?}"));
                        }
                    }
                    Err(e) => log::line(&format!("rename tab {tab_id}: {e}")),
                }
            }
        }
        self.tabs.retain(&live_tabs);

        // Panes.
        let mut live_panes = HashSet::new();
        if self.config.name_panes {
            for pane in panes {
                let Some(pane_id) = s(pane, "pane_id") else {
                    continue;
                };
                live_panes.insert(pane_id.to_string());
                let Some(pt) = titles.get(pane_id) else {
                    continue;
                };
                let desired = title::compose(
                    None,
                    &pt.context,
                    None,
                    pt.activity.as_deref(),
                    self.config.max_length,
                );
                let label = s(pane, "label").unwrap_or("");
                if let Some(new_label) =
                    self.panes
                        .decide(pane_id, label, label.is_empty(), &desired, &self.claims)
                {
                    match self.client.rename_pane(pane_id, &new_label) {
                        Ok(()) => {
                            self.panes.applied(pane_id, &new_label, &mut self.claims);
                            if self.config.debug {
                                log::line(&format!("pane {pane_id}: {label:?} -> {new_label:?}"));
                            }
                        }
                        Err(e) => log::line(&format!("rename pane {pane_id}: {e}")),
                    }
                }
            }
        }
        self.panes.retain(&live_panes);

        self.claims.save();
        Ok(())
    }

    pub fn locked_counts(&self) -> (usize, usize) {
        (self.tabs.locked_count(), self.panes.locked_count())
    }

    fn pane_title(&self, pane: &Value) -> Option<PaneTitle> {
        let agent = s(pane, "agent");
        let mut dir = s(pane, "cwd")
            .or_else(|| s(pane, "foreground_cwd"))
            .map(str::to_string);
        let mut activity = None;

        match agent {
            Some(agent) => {
                let context = dir
                    .as_deref()
                    .map(|d| title::context_name(d, self.home.as_deref()))
                    .unwrap_or_default();
                // The agent's own title is the best statement of its work.
                let raw = s(pane, "title").or_else(|| s(pane, "terminal_title_stripped"));
                activity = raw.and_then(|t| title::agent_activity(agent, t, &context));
                if activity.is_none() {
                    activity = Some(agent.to_string());
                }
            }
            None => {
                if let Some(id) = s(pane, "pane_id") {
                    if let Ok(info) = self.client.process_info(id) {
                        let processes = parse_processes(&info);
                        let gid = info
                            .get("foreground_process_group_id")
                            .and_then(Value::as_i64);
                        if let Some(fg) = title::foreground(&processes, gid) {
                            // The foreground process's directory is where the
                            // pane really works.
                            if let Some(cwd) = &fg.cwd {
                                dir = Some(cwd.clone());
                            }
                            activity = title::process_activity(fg);
                        }
                    }
                }
            }
        }

        let dir = dir?;
        let context = title::context_name(&dir, self.home.as_deref());
        let branch = if self.config.show_branch {
            git::branch(Path::new(&dir))
                .filter(|b| !git::is_default_branch(b))
                .map(|b| git::short_branch(&b, self.config.branch_max))
                .filter(|b| !title::mentions(&context, b))
        } else {
            None
        };
        Some(PaneTitle {
            context,
            branch,
            activity,
        })
    }
}

/// The pane a tab is named after: the one focused within the tab, else a busy
/// agent, else the first.
fn speaking_pane<'a>(
    tab_id: &str,
    panes: &'a [Value],
    focused_in_tab: &HashMap<&str, &str>,
) -> Option<&'a str> {
    let in_tab: Vec<&Value> = panes
        .iter()
        .filter(|p| s(p, "tab_id") == Some(tab_id))
        .collect();
    if let Some(focused) = focused_in_tab.get(tab_id) {
        if let Some(p) = in_tab.iter().find(|p| s(p, "pane_id") == Some(focused)) {
            return s(p, "pane_id");
        }
    }
    if let Some(p) = in_tab.iter().find(|p| {
        p.get("agent").is_some()
            && matches!(s(p, "agent_status"), Some("working") | Some("blocked"))
    }) {
        return s(p, "pane_id");
    }
    in_tab.first().and_then(|p| s(p, "pane_id"))
}

fn parse_processes(info: &Value) -> Vec<Process> {
    info.get("foreground_processes")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|p| {
                    let name = s(p, "name")?.to_string();
                    let argv = p
                        .get("argv")
                        .and_then(Value::as_array)
                        .map(|a| {
                            a.iter()
                                .filter_map(|x| x.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default();
                    Some(Process {
                        pid: p.get("pid").and_then(Value::as_i64),
                        name,
                        argv,
                        cwd: s(p, "cwd").map(String::from),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn speaking_pane_prefers_focused_then_busy_agent() {
        let panes = vec![
            json!({"pane_id": "w1:p1", "tab_id": "w1:t1"}),
            json!({"pane_id": "w1:p2", "tab_id": "w1:t1", "agent": "claude", "agent_status": "working"}),
            json!({"pane_id": "w1:p3", "tab_id": "w1:t2"}),
        ];
        let mut focused = HashMap::new();
        focused.insert("w1:t1", "w1:p1");
        assert_eq!(speaking_pane("w1:t1", &panes, &focused), Some("w1:p1"));
        focused.clear();
        assert_eq!(speaking_pane("w1:t1", &panes, &focused), Some("w1:p2"));
        assert_eq!(speaking_pane("w1:t2", &panes, &focused), Some("w1:p3"));
    }

    #[test]
    fn parses_process_info() {
        let info = json!({
            "foreground_process_group_id": 7,
            "foreground_processes": [
                {"pid": 8, "name": "node", "argv": ["node", "x.js"], "cwd": "/a"},
                {"pid": 7, "name": "npm", "argv": null, "cwd": "/b"}
            ]
        });
        let list = parse_processes(&info);
        assert_eq!(list.len(), 2);
        assert_eq!(
            title::foreground(&list, Some(7)).unwrap().cwd.as_deref(),
            Some("/b")
        );
    }
}
