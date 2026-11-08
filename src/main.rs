//! Herdr Tab Titles: names Herdr tabs and panes automatically.
//!
//! Commands:
//! - `start`   (startup hook) starts the naming loop in the background
//! - `restart` (action) starts a fresh loop; the old one hands over and exits
//! - `stop`    (action) stops the loop
//! - `status`  (action) says whether the loop runs
//! - `run`     the loop itself, in the foreground

mod app;
mod config;
mod git;
mod herdr;
mod log;
mod owner;
mod title;

use std::os::unix::fs::MetadataExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

/// Consecutive failed polls after which the loop gives up (the server is gone).
const MAX_FAILURES: u32 = 60;

struct Paths {
    socket: PathBuf,
    state_dir: PathBuf,
    config_dir: Option<PathBuf>,
    pid_file: PathBuf,
    claims_file: PathBuf,
}

fn paths() -> Paths {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    let socket = std::env::var_os("HERDR_SOCKET_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(&home).join(".config/herdr/herdr.sock"));
    let state_dir = std::env::var_os("HERDR_PLUGIN_STATE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(&home).join(".local/state/herdr-tab-titles"));
    let config_dir = std::env::var_os("HERDR_PLUGIN_CONFIG_DIR").map(PathBuf::from);
    // The state dir is shared by all Herdr sessions; the socket is per session.
    let key = fnv1a(socket.to_string_lossy().as_bytes());
    Paths {
        pid_file: state_dir.join(format!("instance-{key:016x}.pid")),
        claims_file: state_dir.join(format!("labels-{key:016x}.json")),
        socket,
        state_dir,
        config_dir,
    }
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in bytes {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn main() {
    let command = std::env::args().nth(1).unwrap_or_else(|| "start".into());
    let p = paths();
    let _ = std::fs::create_dir_all(&p.state_dir);
    log::init(Some(p.state_dir.join("tab-titles.log")));

    let code = match command.as_str() {
        "start" => {
            spawn_loop(&p);
            0
        }
        "restart" => {
            spawn_loop(&p);
            herdr::Client::new(p.socket.clone()).notify("Tab Titles", "Restarted");
            0
        }
        "stop" => {
            let _ = std::fs::remove_file(&p.pid_file);
            println!("stopped");
            herdr::Client::new(p.socket.clone()).notify("Tab Titles", "Stopped");
            0
        }
        "status" => {
            status(&p);
            0
        }
        "run" => run(&p),
        "--version" | "-V" => {
            println!("herdr-tab-titles {}", env!("CARGO_PKG_VERSION"));
            0
        }
        other => {
            eprintln!("unknown command {other:?}; use start, restart, stop, status or run");
            2
        }
    };
    std::process::exit(code);
}

/// Starts `run` detached from Herdr's pipes and process group, so the startup
/// hook or action returns at once.
fn spawn_loop(p: &Paths) {
    let exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("cannot find own binary: {e}");
            return;
        }
    };
    match Command::new(exe)
        .arg("run")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
    {
        Ok(child) => {
            println!("started pid {} for {}", child.id(), p.socket.display());
        }
        Err(e) => {
            eprintln!("start failed: {e}");
            log::line(&format!("start failed: {e}"));
        }
    }
}

fn read_pid(p: &Paths) -> Option<u32> {
    std::fs::read_to_string(&p.pid_file)
        .ok()?
        .trim()
        .parse()
        .ok()
}

fn alive(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn status(p: &Paths) {
    match read_pid(p) {
        Some(pid) if alive(pid) => println!("running, pid {pid}, socket {}", p.socket.display()),
        Some(pid) => println!("not running (stale pid {pid})"),
        None => println!("not running"),
    }
}

/// Identifies the server behind the socket: a new server binds a new socket
/// file, so its device and inode change.
fn socket_identity(socket: &Path) -> Option<(u64, u64)> {
    std::fs::metadata(socket).ok().map(|m| (m.dev(), m.ino()))
}

fn run(p: &Paths) -> i32 {
    let me = std::process::id();
    // Taking over the pid file tells an older instance to hand over and exit.
    if std::fs::write(&p.pid_file, me.to_string()).is_err() {
        log::line("cannot write pid file");
        return 1;
    }
    let config = config::Config::load(p.config_dir.as_deref());
    log::line(&format!(
        "start v{} socket {} config {config:?}",
        env!("CARGO_PKG_VERSION"),
        p.socket.display()
    ));

    let identity = socket_identity(&p.socket);
    let claims = owner::Claims::load(Some(p.claims_file.clone()));
    let poll = Duration::from_millis(config.poll_ms);
    let mut app = app::App::new(config, herdr::Client::new(p.socket.clone()), claims);
    let mut failures = 0u32;
    let mut last_locked = (0, 0);

    loop {
        if read_pid(p) != Some(me) {
            log::line("another instance took over or stop was requested; exiting");
            break;
        }
        if identity.is_some() && socket_identity(&p.socket) != identity {
            log::line("the Herdr server changed; exiting");
            break;
        }
        match app.tick() {
            Ok(()) => failures = 0,
            Err(e) => {
                failures += 1;
                if failures == 1 || failures.is_multiple_of(10) {
                    log::line(&format!("poll failed ({failures}): {e}"));
                }
                if failures >= MAX_FAILURES {
                    log::line("Herdr unreachable; exiting");
                    break;
                }
            }
        }
        let locked = app.locked_counts();
        if locked != last_locked {
            log::line(&format!(
                "manually named, left alone: {} tabs, {} panes",
                locked.0, locked.1
            ));
            last_locked = locked;
        }
        sleep(poll);
    }

    if read_pid(p) == Some(me) {
        let _ = std::fs::remove_file(&p.pid_file);
    }
    0
}
