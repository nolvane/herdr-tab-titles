# Decisions

## 2026-11-08: Rust, one dependency

The plugin is written in Rust with `serde_json` (MIT or Apache-2.0) as its
only dependency. The socket client, config parser and git branch reader use
the standard library, so the binary is small and the build needs nothing
beyond `cargo`. `serde_json`'s licence allows this use, including
redistribution in source and binary form.

## 2026-11-08: Titles from local state only, no LLM

Titles come from what Herdr and the file system already know: the directory,
the git branch, the foreground program and the agent's own terminal title.
Claude Code and the other agents already summarise their work in that title,
so a language model would add cost, latency and data leaving the machine for
little gain. The plugin makes no network calls.

## 2026-11-08: Poll the snapshot instead of subscribing to events

The loop reads `session.snapshot` once a second. Herdr's event stream replays
a backlog of past revisions on subscribe, which can delay live events by
seconds. A snapshot always describes the present, and one costs about a
millisecond.

## 2026-11-08: Manual names win, recognised without a lock file

A label is the plugin's to change when it is Herdr's default (empty, or the
tab's position) or one the plugin set. Labels the plugin set are kept in
`labels-<socket hash>.json` in the state directory (at most 2000), so after a
restart its own earlier labels are not mistaken for the user's. Anything else
is a name the user chose and is left alone until it is cleared. Existing
manual names therefore survive the first start.

## 2026-11-08: Detached background loop with pid-file handover

Herdr runs startup hooks and actions with piped output and expects them to
finish. `start` and `restart` therefore spawn `run` in its own process group
with null stdio and return at once. The running instance owns
`instance-<socket hash>.pid`; a new instance writes its own pid there, and the
old one sees that on its next poll and exits. `stop` removes the file. The loop
also exits when the socket's device and inode change (a new server) or after
60 failed polls, so no instance outlives its server for long.

## 2026-11-08: Public repository, listed in the Herdr marketplace

The repository is public on GitHub with the topic `herdr-plugin`, which is
how Herdr's marketplace finds plugins, so it installs with
`herdr plugin install nolvane/herdr-tab-titles`. The checks run locally
through `scripts/check.sh`; there is no CI workflow.
