---
cairn: tasks
change: cli
---

- [x] Cargo.toml: `cli` feature, `[[bin]]` with `required-features`
- [x] src/main.rs: arguments, the three TIME forms, `--backend`, `--verbose`, exit codes; unit tests for the parsers and the UTC formatter
- [x] Tests workflow on `--all-features`; Binaries workflow uploading the four targets
- [x] Local checks: fmt, clippy (host, Windows, macOS, Android targets), tests, runs on `timerfd` and `polling`
- [x] CI green, binaries uploaded
- [x] Fold into spec/cli.md, log, archive
- [ ] The user's hand checks on macOS and Windows, suspend included, recorded in the timer check table (user)
