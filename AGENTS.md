# AGENTS.md

Guidance for coding agents working in this repo. Keep it current when a convention or hard-won detail changes.

## What this is

little-owl: a Rust workspace, macOS-first tool that captures highlighted text plus its surrounding context (and, when text fails, a raw screenshot) to feed LLM lookups. `crates/owl-capture` is the only crate today. See `docs/architecture.md` and `docs/idea.md` for the design.

## Conventions

- **Commits:** do NOT add a `Co-Authored-By` / Claude trailer. Message style is lowercase `area: summary` or `area/subarea: summary` (e.g. `capture/macos: ...`).
- **Worktrees:** create them inside the repo under `.worktrees/`, never as siblings. The sandbox EPERMs sibling paths.
- **Comments:** minimal. Comment only where the reason isn't obvious from the code.

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

## macOS capture: hard-won details

- **Read AX through the frontmost app element, not system-wide.** The system-wide element's focused-element read returns `kAXErrorCannotComplete (-25204)` on many setups even when `AXIsProcessTrusted()` is true. Get the frontmost pid via `NSWorkspace` and use `AXUIElement::from_pid(pid)` (what AeroSpace/yabai do). This is in `frontmost_app_element()`.
- **Chromium/Electron gate their AX tree.** Set the `AXManualAccessibility` bool attribute on the app element to force them to build it. Still unreliable, so there's a clipboard fallback.
- **Clipboard fallback** (`enigo` synthetic Cmd+C / Cmd+A + `arboard`): poll for a change, then restore the original clipboard. Known drawbacks tracked as GitHub issues (clobbers non-text clipboard data; Cmd+C-is-copy assumption; Cmd+A leaves the doc selected; Cmd+A can copy unbounded text).
- **Image capture uses `xcap`** (macOS-scoped, `default-features = false`). Its `Monitor::id()` is the `CGDirectDisplayID`; `capture_region` takes display-relative logical points and returns physical pixels, absorbing the scale factor and Y-flip. PNG encoding rides the re-exported `xcap::image` (no separate `image` dep).
- **`ScreenRect` contract:** `x/y/w/h` are logical points relative to the display's top-left; `display` is the `CGDirectDisplayID`. AX position/size come back as global top-left points and are mapped to this in `window_screen_rect`.
- **Swift shim linker workaround:** `build.rs` emits the `-L` path for `axuielement`'s Swift back-compat shims, which live in different places under Xcode vs Command Line Tools. Don't remove it; the link fails without it under CLT.

## Permissions

Accessibility (AX reads, synthetic keys) and Screen Recording (screenshots) must be granted to whatever process runs the code.
