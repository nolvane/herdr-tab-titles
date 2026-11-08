# Herdr Tab Titles

A [Herdr](https://herdr.dev) plugin that names tabs and panes from their
directory, git branch, program and agent topic. What it does and how users
install it: [README.md](README.md). Why it is built this way:
[docs/decisions.md](docs/decisions.md).

## Stack

- Rust (edition 2021), one binary built by `scripts/build.sh` into the plugin
  root, where `herdr-plugin.toml` starts it.
- `serde_json` is the only dependency. The socket client, config parser and
  git branch reader use the standard library; keep it that way unless a
  decision in `docs/decisions.md` says otherwise.
- Needs Herdr 0.9.0 or newer (`min_herdr_version` in `herdr-plugin.toml`).

## Conventions

- Code, comments and docs are in English.
- Notable technical decisions go into `docs/decisions.md` (decision, why,
  date).
- The repository is public: no personal data, no secrets, no private setup.
- Titles come from local state only: no network calls, nothing sent anywhere.

## Checks

```sh
sh scripts/check.sh            # default stage: merge
sh scripts/check.sh --stage quick
```

Every stage (`quick`, `merge`, `release`) runs the same steps: gitleaks secret
scan, `cargo fmt --check`, `cargo clippy` with warnings as errors, the unit
tests and the release build. There are no integration tests, since they would
need a running Herdr server. To try a change in Herdr, see "Try a release
before it reaches users" below.

## Non-goals

- No language model or other outside service for titles.
- No CI workflow; the checks run locally.
- No event subscription; the loop polls `session.snapshot` (see
  `docs/decisions.md`).

## Releasing

**Version.** It lives in two places that must always match: `version` in
`herdr-plugin.toml` and `[package] version` in `Cargo.toml`. After changing
it, run `cargo build --release` once (without `--locked`) so `Cargo.lock`
picks up the new version, then `sh scripts/check.sh --stage release`, which
runs with `--locked`. The binary reports its version with
`./herdr-tab-titles --version` and in the `start v…` line of its log.

**What ships.** There is no store and no separate test channel. Users install
from GitHub, and `herdr plugin update` fetches the newest commit of the ref
recorded at install (the default branch, unless they installed with `--ref`)
and builds it. A merge to `main` is therefore what reaches users, at their
next update. The running instance keeps the old binary until the restart
action or the next Herdr server start.

**Try a release before it reaches users.** Either way replaces the installed
plugin, ends with the restart action, and `herdr plugin list` shows which
source runs.

1. A local checkout (works over a GitHub install):

   ```sh
   sh scripts/build.sh
   herdr plugin link "$PWD"
   herdr plugin action invoke tab-titles.restart
   ```

   `herdr plugin list` shows `local:<path>`. Keep the checkout until you are
   back on the normal install: the running binary is the one in it.
2. A pushed branch:

   ```sh
   herdr plugin install nolvane/herdr-tab-titles --ref <branch> --yes
   herdr plugin action invoke tab-titles.restart
   ```

   `herdr plugin list` shows `github:nolvane/herdr-tab-titles@<branch>`.

Back to the normal install (after a link, run `herdr plugin unlink tab-titles`
first, since Herdr refuses to install over a linked plugin):

```sh
herdr plugin install nolvane/herdr-tab-titles --yes
herdr plugin action invoke tab-titles.restart
```

`herdr plugin list` then shows `github:nolvane/herdr-tab-titles@<commit>`,
which follows the default branch again.

**Changelog and GitHub release.** `CHANGELOG.md` follows the
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) format: an
`[Unreleased]` section, then one section per version (`[X.Y.Z] - YYYY-MM-DD`,
newest first) with Added, Changed, Fixed and Removed. The first release
creates the file. A release ends with an annotated tag `vX.Y.Z` on the
released commit and a GitHub release of that tag whose notes are the
version's section of `CHANGELOG.md`.
