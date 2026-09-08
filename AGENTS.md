# AGENTS.md

Guidance for coding agents working in this repo.
## What this is

little-owl: a Rust workspace, macOS-first tool that captures highlighted text plus its surrounding context (and, when text fails, a raw screenshot) to feed LLM lookups.
## Conventions

- **Commits:** Message style is lowercase `area: summary` or `area/subarea: summary` (e.g. `capture/macos: ...`).
- **Worktrees:** create them inside the repo under `.worktrees/`, never as siblings. The sandbox EPERMs sibling paths.
- **Comments:** minimal. Comment ONLY where the reason isn't obvious from the code.

## Build / verify

Run all four before calling work done:

```
cargo build --all-targets
cargo test -p owl-capture
cargo clippy --all-targets
cargo fmt --check
```

## Testing pattern

- Pure helpers (`clamp_region`, `doc_url_to_path`, `percent_decode`, `clipboard_changed`, `non_empty`) have plain unit tests.
- Anything hitting the live system is a `#[ignore]` test with a reason string; it needs Accessibility and/or Screen Recording granted to the test runner. `examples/ax_probe.rs` is the manual probe.

## Permissions

Accessibility (AX reads, synthetic keys) and Screen Recording (screenshots) must be granted to whatever process runs the code.
