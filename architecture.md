# Architecture and Stack

Status: proposal. Written against `idea.md`, Aug 2026. Nothing here is committed until the first commit lands.

## What this has to be

The probe is a two-week instrumented build, so the architecture has to earn its keep immediately rather than in month three. Three constraints do most of the work.

Cross-platform intent from the first commit. This is not the same as shipping three platforms in week one, and the distinction matters more than anything else in this document. What it means concretely is that every OS-specific operation sits behind a trait defined in a platform-agnostic crate, and CI builds all three targets from day one so a Windows-hostile assumption fails in minutes rather than in month two. What it does not mean is that all three implementations land before you start reading papers. macOS first, because it is your machine and the two-week clock is real.

Lightweight and fast. Perceived latency is the overlay appearing, not the answer arriving, so the architecture optimizes for a prewarmed process that can put pixels on screen within one frame of the hotkey. Everything else is downstream of that.

Local-verifiable rather than local-only. Open source makes local-only claims checkable, and a text-first capture path lets you avoid the Screen Recording permission entirely in the common case. The honest v1 claim is that nothing leaves the machine except the highlighted span and its immediate surroundings, sent to a model provider you configured, with no server of ours in the path. That is a stronger claim than most competitors can make and it does not require shipping local inference in two weeks.

## The decision that shapes the rest

Capture text, not pixels.

The accessibility APIs on all three platforms expose the selected text and often the full text of the focused element, which gives you the span and the surrounding context in one call, in a few milliseconds, with no image encoding and no vision model. On macOS this needs Accessibility permission only. Screen Recording, the scarier of the two dialogs and the one that drives install-time drop-off, is only required if you enable the OCR fallback, so OCR becomes an opt-in setting rather than a launch blocker.

The cost is coverage, and coverage is application-dependent. This is the highest technical risk in the project and it deserves a precise statement rather than a hopeful one.

Capture is not one operation that succeeds or fails. It is three independent axes, because the underlying calls are independent: the selected text, the surrounding text of the focused element, and the identity of the document are three separate queries that fail separately. Modelling this as a single ordered cascade would silently discard context that was available, so the design treats them as three.

| Axis | Path | Realistic degradation |
|---|---|---|
| Span | AX selected text, then synthetic copy plus clipboard, then OCR | Nearly always available, because the clipboard works wherever the copy shortcut works. |
| Context | AX value of the focused element, then OCR of the window region, then nothing | Frequently unavailable. The clipboard path supplies no context at all. |
| Provenance | App name and window title, then document path via `AXDocument` or the UIA equivalent | App name and title almost always resolve. The document path is the thing you lose. |

So the common failure mode is not a failed capture. It is a span with no surrounding paragraph and no file path, which is a usable answer with weak provenance rather than nothing at all. The clipboard fallback mutates global state briefly and must restore the prior clipboard contents; OCR is slow and permission-hungry, which is why it stays off by default.

Which path fired is logged on every query, because it is instrumentation data in its own right. A corpus dominated by clipboard captures says something about where you actually read, and it also confounds one of the questions in the table. See the instrumentation section.

## Stack

| Layer | Choice | Why |
|---|---|---|
| Language | Rust, 2024 edition | Single static binary per platform, no runtime to install, predictable startup, and the best cross-platform crate ecosystem for global hotkeys and accessibility APIs. |
| UI | `egui` via `eframe` | Native GPU-rendered panel, roughly 40MB resident, no webview in the dependency graph. See below. |
| Async runtime | `tokio`, multi-threaded | Streaming HTTP, filesystem writes, and SQLite inserts off the UI thread. |
| HTTP | `reqwest` with `rustls` | No OpenSSL to link on three platforms. |
| Streaming | `eventsource-stream` over `reqwest` | Server-sent events from the model provider. |
| Global hotkey | `global-hotkey` | Maintained by the Tauri organization, covers all three platforms with a Wayland caveat. |
| Clipboard | `arboard` | Read, write, and restore across all three platforms. |
| Accessibility | Per-OS, see below | No cross-platform crate exists that is good enough to trust here. |
| Instrumentation | `rusqlite`, bundled SQLite | Zero-config embedded store, real queries for the report. |
| Config | `toml` plus `directories` | Platform-correct config paths without hand-rolling them. |
| Secrets | `keyring` | Keychain on macOS, Credential Manager on Windows, Secret Service on Linux. API keys never touch the config file. |
| Packaging | `cargo-dist` or hand-rolled CI | Signed and notarized `.app` and `.dmg`, MSI or NSIS, AppImage and `.deb`. |

Pin exact versions at implementation time rather than trusting this table, since several of these crates move quickly.

### Why egui and not Tauri

Tauri v2 is the obvious default and I am recommending against it for this specific surface.

The overlay is one small panel showing a captured span, a streaming markdown answer, and optionally a prompt input. That is close to the minimum viable UI, which is exactly the case where a webview's advantages in typography and CSS layout are worth least and its costs are worth most. The decisive cost is Linux. Tauri renders through WebKitGTK there, which means packaging dependencies, version skew across distributions, and rendering that differs visibly from the macOS and Windows builds. You have asked for cross-platform intent from the start, and the webview is the single largest source of per-platform divergence you could adopt.

egui gives you one rendering path on all three platforms, a binary in the 10 to 15MB range with no external runtime, resident memory around 40MB against 100 to 150MB for a webview, and a frame you can draw in a couple of milliseconds. Markdown rendering comes from `egui_commonmark`, which handles headings, code blocks, lists, and inline emphasis, which is the whole vocabulary an atomic definition needs.

What you give up is real: CSS-quality text layout, easy theming, and faster iteration on visual polish. If typography becomes the thing that makes the tool unpleasant to use daily, the UI crate is swappable, because the overlay talks to the core through a channel rather than calling into it. That swap is the documented fallback, not a hypothetical.

## Process and threading model

One process, tray-resident, launched at login.

The process starts, registers the hotkey, opens the SQLite log, and creates the overlay window hidden. Creating the window at startup rather than on first invocation is the entire latency story, because window creation is the expensive part and showing an existing window is one frame.

The UI runs on the main thread, which macOS requires and the other two platforms tolerate. Everything else runs on tokio. The hotkey callback fires on whichever thread the OS gives it, pushes an `Invoke` message onto a channel, and returns immediately, because blocking in a hotkey callback is how you get the OS to think your application has hung.

The core state machine consumes that channel and drives the request. Two things happen in parallel rather than in sequence: the overlay is shown immediately with a spinner, and the capture cascade runs on a worker. When capture completes the span is pushed to the UI, and when the first token arrives the answer begins streaming. Serializing capture before showing the overlay would make a 100ms clipboard round-trip feel like a stutter, and there is no reason to.

## Crate layout

```
little-owl/
  crates/
    owl-core/      state machine, request orchestration, no platform code
    owl-capture/   Capture trait, macos/windows/linux impls, OCR fallback
    owl-hotkey/    Hotkey trait, per-OS impls
    owl-llm/       Provider trait, streaming, openai + ollama impls
    owl-vault/     note writing, frontmatter, wikilinks, atomic filesystem ops
    owl-log/       SQLite schema, event writes, report queries
    owl-ui/        overlay panel
    owl-app/       binary: tray, config loading, wiring
```

The point of the split is that `owl-core` compiles on every platform with no `cfg` attributes anywhere in it, which is the mechanical enforcement of cross-platform intent. Platform divergence is confined to two crates, and both of them are small.

## Capture layer

This is where the real work is and where a naive design will hurt you in month two.

The trait is roughly:

```rust
pub struct Capture {
    pub span: String,
    pub context: Option<String>,   // surrounding text from the focused element
    pub source: SourceRef,
    pub method: CaptureMethod,     // Accessibility | Clipboard | Ocr
    pub elapsed_ms: u32,
}

pub struct SourceRef {
    pub app_name: String,
    pub window_title: String,
    pub document_path: Option<PathBuf>,  // AXDocument, UIA ValuePattern, etc.
    pub url: Option<String>,
    pub page_hint: Option<String>,       // best-effort, parsed from title
}

pub trait Capturer: Send + Sync {
    fn capture(&self, prefer: &[CaptureMethod]) -> Result<Capture>;
}
```

`SourceRef` is the provenance record, and it is the field to get right on the first commit, because a two-week corpus with weak provenance cannot answer the questions in your table. `document_path` in particular is the difference between a per-document view that works and one that guesses from window titles.

### macOS

The accessibility path uses `AXUIElementCopyAttributeValue` against the focused element, reading `AXSelectedText` for the span and `AXValue` for the surrounding text. `AXDocument` on the focused window returns a file URL for document-based applications including Preview and Skim, which gives you a real path rather than a title heuristic. Frontmost application name comes from `NSWorkspace`. Browser URLs come from the accessibility tree where exposed, or AppleScript where not. Crates: `accessibility` and `accessibility-sys`, with `objc2` and `objc2-app-kit` for the `NSWorkspace` parts.

Accessibility permission is required, and posting synthetic key events for the clipboard fallback requires it too, so there is exactly one permission prompt in the common path. Worth knowing during development: the permission grant is keyed to the code signature, so rebuilding invalidates it and you will re-grant constantly. Sign development builds with a stable identity to avoid that.

Expected coverage across the reading surfaces that matter, to be verified rather than trusted:

| Surface | Span | Context | Document path |
|---|---|---|---|
| Native Cocoa views: TextEdit, Mail, Notes | Yes | Yes | Yes |
| Preview, Skim, PDFKit-based readers, text-layer PDFs | Yes | Yes | Yes, via `AXDocument` |
| Scanned PDFs, no text layer | OCR only | OCR only | Yes |
| Safari, Firefox | Yes | Yes | URL |
| Chromium: Chrome, Edge, Brave, Arc | Yes | Yes | URL |
| Electron, contenteditable surfaces such as Notion | Usually | Usually | Title only |
| Electron, custom canvas editors such as the VS Code editor pane | Often fails, clipboard | Rarely | Title only |
| Adobe Acrobat | Assume clipboard | Rarely | Title only |
| DRM readers: Kindle, some journal and O'Reilly web viewers | Often blocked entirely | No | Title only |

Chromium is worth a specific note. It builds its accessibility tree lazily on detecting an assistive client, so the first query after launch can lag noticeably while the tree materializes. Budget for it rather than treating it as a bug.

The set that defeats even the clipboard is small but real: scanned PDFs, viewers that intercept copy, and figures. That set is OCR or nothing, which is the argument for keeping the OCR path in the trait even though most users will never enable it.

Because this table is an estimate and your reading stack is specific, the first thing to build is a throwaway probe binary that walks the accessibility tree of the frontmost application and prints what each axis returns. Run it against the applications you actually read in. That is an afternoon of work and it replaces the table above with facts, which is worth doing before the architecture depends on it.

### Windows

UI Automation is the equivalent and is generally better implemented by applications than macOS accessibility is. `TextPattern.GetSelection` gives the span, the enclosing `TextRange` gives context, `ValuePattern` on browser address bars gives the URL, and `GetForegroundWindow` plus process lookup gives the application name. Document paths are harder than on macOS and will often fall back to title parsing. Crate: `windows`. No permission prompt at all, which makes Windows the easiest of the three.

### Linux

AT-SPI2 over D-Bus, via the `atspi` crate, provides `Text.GetSelectedText` and the surrounding text. Window title and active window come from X11 properties, `_NET_ACTIVE_WINDOW` and `_NET_WM_NAME`.

Wayland is the weak surface and should be stated plainly rather than discovered later. There is no global window introspection and no X11-style key grabbing, so hotkeys go through the `org.freedesktop.portal.GlobalShortcuts` portal where the compositor implements it, and window metadata is largely unavailable. On Wayland the realistic v1 is clipboard-based capture with degraded provenance, plus a tray or CLI trigger where the shortcut portal is missing. X11 sessions get the full experience. Say this in the README rather than letting a user discover it.

### OCR fallback

Off by default. When enabled it uses the Vision framework on macOS, `Windows.Media.Ocr` on Windows, and Tesseract on Linux, all against a capture of the focused window region. Cost is 100 to 400ms and, on macOS, the Screen Recording permission. Its value is scanned PDFs, which are common enough in academic reading that the path should exist even if most users never turn it on.

## Hotkey layer

`RegisterHotKey` on Windows, `RegisterEventHotKey` or a `CGEventTap` on macOS, `XGrabKey` on X11, and the GlobalShortcuts portal on Wayland. The `global-hotkey` crate covers the first three. Three chords are registered, one each for Explain, Define, and Prompt, all user-configurable, because default chords will collide with something on somebody's machine.

Right-click menu integration is a separate and much larger problem, since it means a system service on macOS, a shell extension on Windows, and nothing portable on Linux. `idea.md` lists it alongside the hotkey, and I would defer it past the probe. The hotkey is sufficient to generate the corpus, and shell integration is weeks of platform-specific work that produces no new evidence.

## Inference layer

A provider trait with streaming, two implementations.

```rust
pub trait Provider: Send + Sync {
    async fn stream(&self, req: Request) -> Result<impl Stream<Item = Result<String>>>;
}
```

The cloud implementation targets the OpenAI API with streaming enabled. The default model is `gpt-5.6-luna`.

The GPT-5.6 family has three tiers, all at 1.05M context and 128K max output: `gpt-5.6-sol` at $5 per million input tokens and $30 output, `gpt-5.6-terra` at $2 and $12, and `gpt-5.6-luna` at $0.20 and $1.20. Luna is the right default and it is not a close call, since it is 25 times cheaper than Sol on input and an atomic definition is the canonical case where flagship capability buys nothing. The 1.05M context window is irrelevant here, because the request is a span and a paragraph. Note that the bare `gpt-5.6` alias routes to Sol, so it must not become the default by accident.

The more important knob is `reasoning_effort`, which accepts `none`, `low`, `medium`, `high`, `xhigh`, and `max`. OpenAI's guidance is `low` for latency-sensitive workloads, and `none` is the right starting point for a lookup because there is nothing to reason about in a definition. Pair it with `text.verbosity` set to `low`, which maps directly onto the atomic note format. Verify the exact parameter nesting against the current Responses API reference at implementation time rather than trusting the spelling here.

Effort dominates latency far more than tier does, and there is a useful piece of evidence for that. A benchmark aggregator reports Luna at 117 seconds to first token, which is Luna at `max` effort rather than base latency. It is the kind of number that would talk you out of the correct model, and the correct reading of it is that effort is the lever.

Two things are simpler than they would be on a provider with explicit cache control. Cached input is automatic at 10% of the input rate on repeated prefixes, so the stable prefix of system prompt plus note-format instructions caches with no breakpoints to place and no minimum-prefix threshold to design around. And there are no reasoning blocks to strip from the response, so the streaming parser handles only text deltas.

If Luna's answer quality on academic terminology proves insufficient, `gpt-5.6-terra` is the step up at ten times the input cost, and the change is one config value. That is a quality tradeoff and therefore your call, which is the reason the model is configuration rather than a constant.

The local implementation talks to Ollama over HTTP on localhost, which sidesteps bundling `llama.cpp` and gives you a genuine no-network path for users who want the local-only claim to be literal rather than architectural. Answer quality on academic terminology will be meaningfully worse with a small local model, so this is an option rather than the default.

API keys live in the OS keychain via `keyring`, never in the config file, because the config file is the thing users paste into issues.

## Vault writer

Plain markdown with YAML frontmatter, written atomically as a temporary file plus rename so Obsidian's file watcher never observes a partial note.

Filenames follow `YYYY-MM-DD HHmm <slug>.md` where the slug derives from the span, truncated and sanitized for all three filesystems, which in practice means the Windows reserved-character set since it is the strictest. Collisions get a numeric suffix.

Frontmatter carries the provenance:

```yaml
---
type: lookup
mode: explain
created: 2026-08-06T18:14:22-03:00
span: "epistemic uncertainty"
source:
  app: Preview
  title: "Attention Is All You Need"
  path: /Users/felipe/Papers/attention.pdf
  page: 4
model: gpt-5.6-luna
latency_ms: 612
capture_method: accessibility
follow_up_of: null
---
```

Each note links to a per-document hub note, created on demand, which is what buys you the per-document view described in `idea.md` at close to zero cost. Follow-ups write a new note with `follow_up_of` set and a wikilink back, which is the structural commitment against rebuilding the chat thread. There is no scrollback because there is no place to put one.

## Instrumentation

This is the actual deliverable of v1, so it is first-class rather than a logging afterthought.

SQLite, in the platform data directory, deliberately outside the vault so that instrumentation never pollutes the notes. One row per query:

```sql
CREATE TABLE query (
  id              INTEGER PRIMARY KEY,
  ts              TEXT NOT NULL,
  mode            TEXT NOT NULL,           -- explain | define | prompt
  app             TEXT,
  doc_id          TEXT,                    -- hash of path or url
  doc_title       TEXT,
  span            TEXT NOT NULL,
  span_norm       TEXT NOT NULL,           -- lowercased, punctuation stripped
  span_len        INTEGER NOT NULL,
  context_len     INTEGER,
  capture_method  TEXT NOT NULL,
  capture_ms      INTEGER NOT NULL,
  ttft_ms         INTEGER,
  total_ms        INTEGER,
  answer_len      INTEGER,
  dismissed_ms    INTEGER,                 -- overlay lifetime, proxy for read/not-read
  sufficient      INTEGER,                 -- explicit thumb, nullable
  follow_up_of    INTEGER REFERENCES query(id),
  note_path       TEXT NOT NULL
);
```

The five rows of your table map onto queries over this schema, which is the test of whether the schema is right.

Asking about something already asked needs near-duplicate detection rather than exact matching, and trigram Jaccard similarity over `span_norm` is enough for a two-week corpus. No embedding model, no extra dependency, computed on insert against the existing rows for the same document and then globally. A duplicate rate that clusters within documents argues for something different than one that clusters across them.

Answer not publicly available cannot be measured automatically and needs the explicit `sufficient` flag, so the overlay carries a two-key affordance and the report reports how often you actually used it.

Asking about something demonstrably learned earlier is the duplicate query filtered by a time gap, which is why the timestamp needs to be real rather than a date.

Span alone insufficient is the follow-up rate, and specifically the follow-up rate conditioned on short spans, which is why `span_len` and `context_len` are columns rather than derived.

This row has a confound serious enough to change the conclusion, so it is worth stating explicitly. Because context availability varies by application, a follow-up may have been caused by a thin span or by a clipboard capture that supplied no context at all. Those are different findings: the first argues for whole-document context modelling, the second argues only that your PDF reader has a bad accessibility tree. The report therefore has to partition on `capture_method` and treat `context_len` of zero as its own bucket. Read naively, this row measures capture coverage while appearing to measure context sufficiency.

Low reach count is the count, and the honest version of it is the count of days with at least one query rather than the total, because a single enthusiastic afternoon is not a habit.

`little-owl report` runs all of it and prints the ranked failure list. Writing that command in week one rather than week three is what makes the log trustworthy, because a schema whose report has never run is a schema with a missing column.

## Latency budget

| Stage | Target | Notes |
|---|---|---|
| Hotkey to overlay visible | under 16ms | Prewarmed hidden window, one frame. |
| Capture via accessibility | 5 to 30ms | Runs in parallel with showing the overlay. |
| Capture via clipboard | 40 to 120ms | Keystroke synthesis plus clipboard round-trip. |
| Capture via OCR | 100 to 400ms | Opt-in path only. |
| Prompt assembly | under 1ms | |
| Time to first token | 250 to 600ms | Dominated by the provider. Streaming is not optional. |

The number the user feels is the first row. Everything after it is covered by the overlay already being on screen with the captured span visible, which is why the parallel structure matters more than any individual optimization below it.

## Distribution

GitHub Actions with a three-runner matrix, building natively on each rather than cross-compiling, since the accessibility bindings make cross-compilation more trouble than a second runner.

macOS needs a Developer ID signature and notarization. This is not optional at a hundred dollars a year, because an unsigned binary asking for Accessibility permission from an unknown solo developer is the worst possible first impression and Gatekeeper friction will lose most installs before the permission dialog even appears. Windows ships MSI or NSIS, unsigned initially, accepting SmartScreen friction until download volume justifies a certificate. Linux ships an AppImage and a `.deb`, with an AUR package later if anyone asks.

## Build order

The two-week clock argues for a specific sequence, and the sequence is not the same as the architecture.

Day one: the accessibility probe binary described above, run against your real reading stack. Everything downstream depends on what it reports, and it is a few hours. If it comes back saying your primary PDF reader exposes nothing, that is a different project and you want to know on day one rather than day nine.

Days one and two: crate skeleton, all traits defined, CI matrix green on three platforms with stub implementations. This is the cross-platform commitment, and it is cheap now and expensive later.

Days three and four: macOS clipboard capture, hotkey, prewarmed overlay, streaming answer on screen. Clipboard before accessibility because it is simpler and works everywhere, which gets you to a usable tool a day earlier.

Day five: vault writer with full frontmatter, and the SQLite log with the report command. Both before the tool is pleasant, because a corpus with a missing column cannot be retrofitted.

Days six through fourteen: use it while reading papers. Fix only what blocks daily use. macOS accessibility capture lands here when clipboard fallback proves annoying, which it will, and near-duplicate detection lands here once there is enough corpus for it to mean anything.

Windows and Linux implementations land after the probe reports. The traits and the CI matrix mean they are a week of filling in two files rather than an architectural rewrite, which is what "cross-platform from the start" actually buys.

## What I would flag

Text capture is not application-independent, and no architecture makes it so. The span is close to universal because the clipboard is, but context and document identity degrade per application, and the degradation is invisible to the user. This is the risk most likely to make the tool feel unreliable in daily use, and the probe binary is the cheapest way to size it before committing.

Right-click menu integration is scoped out of the probe above, and that is a real narrowing of `idea.md`. It is weeks of per-platform work that produces no evidence the hotkey does not already produce.

The `sufficient` flag is the weakest part of the instrumentation, because it depends on you pressing a key in the moment, and the failure log will quietly under-report insufficiency. The overlay lifetime column is a partial proxy. I do not have a better answer, and I would rather name the gap than pretend the column is clean.

Wayland will be a worse product than X11 and there is no architecture that fixes it. Documenting the degradation is the whole mitigation.
