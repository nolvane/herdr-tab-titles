# Herdr Tab Titles

A [Herdr](https://herdr.dev) plugin, written in Rust, that names tabs and panes
automatically from what runs in them:

```
1 · my-app › Fix the login redirect…     2 · tmp › vim notes.txt     3 · app-abc42
```

A title is built from the tab's focused pane:

- **Directory**: the last part of the pane's working directory (`~` for home).
- **Git branch**: shown when it is not `main`, `master`, `develop` or similar.
  A ticket key in the branch (`abc-123-…`) is shown as `ABC-123`; a branch the
  directory name already mentions is left out.
- **Activity**: for a coding agent (Claude Code, Codex, OpenCode, Grok, Pi …)
  the topic the agent puts in its terminal title; for anything else the
  foreground program and its first argument (`vim notes.txt`, `ssh › prod-01`,
  `cargo build`). A shell at its prompt has no activity.
- **Position**: the number in front is the tab's position, the key that
  switches to it.

If the title is too long, the branch goes first, then the activity is cut.

Panes are named too (without the position), which is what Herdr's goto panel
lists them by.

## Your own names win

A tab or pane you rename yourself keeps its name. Clear the name
(`herdr tab rename <tab> ""` or the rename dialog with an empty name) and the
plugin takes over again. Labels the plugin set are remembered across restarts,
so it never mistakes its own work for yours.

## Install

Needs Herdr 0.9.0 or newer and Rust (`cargo`), on macOS or Linux.

```sh
herdr plugin install nolvane/herdr-tab-titles
herdr plugin action invoke tab-titles.restart
```

Herdr builds the plugin while installing it, and starts it by itself with
every server start from then on. To update it:

```sh
herdr plugin update tab-titles
herdr plugin action invoke tab-titles.restart
```

Herdr does not restart a running plugin after an update, so without the
restart action the old version keeps running until the next Herdr server start.

Recommended in `~/.config/herdr/config.toml`, so new tabs are created without a
name prompt and get their name from the plugin:

```toml
[ui]
prompt_new_tab_name = false
```

To work on the code, clone the repository, run `sh scripts/build.sh` and
`herdr plugin link "$PWD"`. After each change: `sh scripts/build.sh`, then
`herdr plugin action invoke tab-titles.restart`.

## Actions

| Action | What it does |
|---|---|
| `tab-titles.restart` | Starts a fresh instance; the running one hands over and exits |
| `tab-titles.stop` | Stops naming until the next restart; names stay as they are |
| `tab-titles.status` | Says whether it runs (see `herdr plugin log list --plugin tab-titles`) |

Bind one to a key in `config.toml`:

```toml
[[keys.command]]
key = "prefix+T"
type = "plugin_action"
command = "tab-titles.restart"
description = "Restart Tab Titles"
```

## Settings

Optional. Copy `config.toml.example` to the directory
`herdr plugin config-dir tab-titles` prints, name it `config.toml`, and run the
restart action.

| Setting | Default | Meaning |
|---|---|---|
| `poll_ms` | `1000` | How often the session is read (ms) |
| `max_length` | `40` | Maximum title width, position included |
| `show_position` | `true` | Tab position in front of the title |
| `show_branch` | `true` | Show a non-default git branch |
| `branch_max` | `16` | Maximum branch width |
| `name_panes` | `true` | Name panes as well as tabs |
| `debug` | `false` | Log every rename |

The log is `tab-titles.log` in the plugin's state directory
(`~/.local/state/herdr/plugins/tab-titles/` on macOS).

## How it works

A small background loop reads `session.snapshot` over Herdr's socket API, asks
`pane.process_info` for panes without an agent, and calls `tab.rename` and
`pane.rename` only when a title changes. It reads the git branch from
`.git/HEAD` directly, runs no git commands, makes no network calls and sends
nothing anywhere. One instance runs per Herdr session; it exits when its
server stops.

## Development

```sh
sh scripts/check.sh
```

This runs a secret scan (needs `gitleaks`), `cargo fmt --check`,
`cargo clippy`, the tests and the release build. Design decisions are in
[docs/decisions.md](docs/decisions.md).
