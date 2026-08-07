# Reading Tool — Concept Doc

Status: committed to a two-week instrumented probe. Slot 3 of the portfolio (user-facing). Aug 2026.

No stack or architecture decisions yet. No speculative features — the probe decides what comes next.

## The problem

Monolithic, single-branch chat interfaces are an inefficient form of information seeking at the level of atomic explanations and definitions. The friction is sharpest when reading a text you can't edit — an academic paper being the standard case.

Two failures compound:

- **Structural.** A linear thread is the wrong container for a set of small, unrelated lookups. Ten definitions from one paper become ten turns in one conversation, tangled with each other and with whatever else was in the thread.
- **Contextual.** Every query starts from zero.

This is my own pain, consistently, over a long period. I am the user.

## What gets built

Highlight text. Hotkey or right-click gives `Explain` / `Define` / `Prompt`. The app captures the surrounding window as context, answers, and writes an atomic markdown note into a vault with provenance recorded — source document, location, span text.

That's it. Nothing else.

Design commitments that hold regardless of what comes later:

- **Low-intelligence tasks, answered fast.** These are lookups, not reasoning. Latency is a feature.
- **A byproduct of work already being done.** Notes accumulate as a side effect of asking. No curation, ever.
- **Atomic, linked, local, plain files.** Obsidian's ideology, populated by reading rather than by note-taking.
- **Follow-ups spawn new linked notes, never a thread.** The moment there's a scrollback, the chat problem has been rebuilt.

## Scoping decisions already made

**No rendered persistent highlights.** Pinning an overlay to a span inside an application I don't control — surviving scroll, resize, and reflow — is the genuinely hard part. Record the anchor in frontmatter instead; that yields a per-document view (everything I asked while reading this paper) at near-zero cost. On a surface where the document model is mine, rendered highlights can be added later.

**Consumer, not enterprise.** Sold to individuals for personal use. No org buy-in, no admin approval, no sales motion.

**Lead with papers and personal learning** — where I'm the user, the machine is mine, and nobody has to approve anything.

**Open source, developed in public from the first commit.** Not really optional: users are granting screen recording and accessibility permissions to a binary from an unknown solo developer that records what they read and didn't understand. Open code is how local-only becomes verifiable rather than promised. Costs nothing defensively — a fork gets a cold-start product with an empty vault.

## Competitive position

- **System-wide capture with screen context:** Raycast Screen Awareness (macOS v2 beta v0.71, shipped 2026-08-06 — not in stable, never implicit; invoked via *Send Focused Window to AI*, the `@` menu, or "summarise this"). ChatGPT's Mac app: option-space, screenshot input, Work With Apps.
- **Highlight → anchored atomic note → linked vault:** Obsidian plugins. HiNote attaches comments to the exact passage with export to linked notes. HiLighter turns highlights into note cards. Both only work on text already inside the vault.
- **Capture from anywhere you're reading, artifact lands in a vault with provenance:** open.

Raycast has Notes, but it's a scratchpad — nothing anchors an artifact to a span of source text.

Highlight-to-explain is a feature, and features get absorbed. That matters for a company thesis and not for this.

## The real function of v1

The explain feature is a floor I will undoubtedly use. It is also **an instrument**.

Every note is a record of something I didn't know, at a specific moment, in a specific document, with context attached. Two weeks of that is a corpus of my own atomic confusions with provenance — the friction log I never managed to keep by hand, automated and far more specific.

**The intelligence layer does not get designed. It gets read off that corpus.** Every extension previously considered — retrieval over prior notes, private/non-public knowledge, spaced probing, whole-document context — was speculation. None had evidence. The log produces the evidence, or kills the idea.

## The probe

Build the stateless version. Use it while reading papers for two weeks.

**Instrument it.** Log every query: source document, the span, whether the answer was sufficient, whether a follow-up was needed.

Questions the log answers, in order of what it would change:

| Observed pattern | What it argues for |
|---|---|
| Asking about something already asked | Retrieval over prior notes |
| Answer isn't publicly available | Private knowledge base |
| Asking about something demonstrably learned earlier | Spaced probing |
| Span alone is insufficient | Whole-document context modelling |
| Low reach count overall | Kill it |

The output is not "did I like it." It's a ranked list of what the tool failed at.

## Why this one

Every previous candidate died for lacking what this has: it came from a problem I actually have, generated rather than selected. Pull is a hard gate that predicts completion for me specifically, and this is the first candidate to pass it. It clears the "nameable person with recurring pain" bar that no externally-sourced idea could.

**Verdict scope:** good project, settled. Good company, not settled, and not due now.

## Known risks

- **n=1.** One confirmed user with this pain, and it's me. Fine for starting; not validation.
- **Permission surface.** Screen recording and accessibility on macOS, with alarming dialogs and real install-time drop-off. Pushes toward local-only processing. Hard to retrofit — decide early.
- **The floor may be the ceiling.** The single-shot feature may be all there is. That is not failure: the original complaint was structural, and the single-shot version solves it completely.