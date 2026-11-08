# Changelog

All notable changes to this plugin are documented in this file. The format is
based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this
project uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.0] - 2026-11-08

### Added

- Tabs and panes are named automatically from the directory, the git branch
  (when it is not a default branch such as `main`), and what runs in them: a
  coding agent's topic or the program and its first argument.
- The tab's position is shown in front of its name, and long names are
  shortened.
- Names you set yourself are kept; clearing a name hands it back to the
  plugin, also across restarts.
- Actions to restart, stop and check the plugin, and optional settings in
  `config.toml`.
- Instructions for updating the plugin, including the restart that loads the
  new version (#2).

[Unreleased]: https://github.com/nolvane/herdr-tab-titles/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/nolvane/herdr-tab-titles/releases/tag/v1.0.0
