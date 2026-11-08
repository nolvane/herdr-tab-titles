//! Minimal client for the Herdr socket API.
//!
//! The wire format is newline-delimited JSON: one request `{id, method, params}`
//! per connection, answered by one line `{id, result}` or `{id, error}`, after
//! which Herdr closes the connection.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

pub struct Client {
    pub socket: PathBuf,
    timeout: Duration,
}

impl Client {
    pub fn new(socket: PathBuf) -> Self {
        Client {
            socket,
            timeout: Duration::from_secs(3),
        }
    }

    pub fn call(&self, method: &str, params: Value) -> Result<Value, String> {
        let mut stream = UnixStream::connect(&self.socket)
            .map_err(|e| format!("connect {}: {e}", self.socket.display()))?;
        stream.set_read_timeout(Some(self.timeout)).ok();
        stream.set_write_timeout(Some(self.timeout)).ok();

        let id = format!("tab-titles:{}", NEXT_ID.fetch_add(1, Ordering::Relaxed));
        let request = json!({ "id": id, "method": method, "params": params });
        let mut line = request.to_string();
        line.push('\n');
        stream
            .write_all(line.as_bytes())
            .map_err(|e| format!("write {method}: {e}"))?;

        let mut reader = BufReader::new(stream);
        let mut answer = String::new();
        reader
            .read_line(&mut answer)
            .map_err(|e| format!("read {method}: {e}"))?;
        if answer.trim().is_empty() {
            return Err(format!("{method}: empty answer"));
        }
        let value: Value =
            serde_json::from_str(answer.trim()).map_err(|e| format!("parse {method}: {e}"))?;
        if let Some(error) = value.get("error") {
            return Err(format!("{method}: {error}"));
        }
        value
            .get("result")
            .cloned()
            .ok_or_else(|| format!("{method}: answer without result"))
    }

    pub fn snapshot(&self) -> Result<Value, String> {
        let result = self.call("session.snapshot", json!({}))?;
        result
            .get("snapshot")
            .cloned()
            .ok_or_else(|| "session.snapshot: no snapshot".to_string())
    }

    pub fn process_info(&self, pane_id: &str) -> Result<Value, String> {
        let result = self.call("pane.process_info", json!({ "pane_id": pane_id }))?;
        result
            .get("process_info")
            .cloned()
            .ok_or_else(|| "pane.process_info: no process_info".to_string())
    }

    pub fn rename_tab(&self, tab_id: &str, label: &str) -> Result<(), String> {
        self.call("tab.rename", json!({ "tab_id": tab_id, "label": label }))
            .map(|_| ())
    }

    pub fn rename_pane(&self, pane_id: &str, label: &str) -> Result<(), String> {
        self.call("pane.rename", json!({ "pane_id": pane_id, "label": label }))
            .map(|_| ())
    }

    pub fn notify(&self, title: &str, body: &str) {
        let _ = self.call("notification.show", json!({ "title": title, "body": body }));
    }
}
