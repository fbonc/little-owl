# UI and host flow

Little Owl keeps the overlay UI separate from the code that controls the native window and talks to the backend. The easiest way to think about it is:

- The **host** owns the outside world: the window, hotkey-driven visibility, resizing, focus, and channels to the backend.
- The **overlay** owns what the user sees and the current UI state.
- The **prompting component** owns the prompt input's value and focus state.

The current host example is [`overlay_demo.rs`](../crates/owl-ui/examples/overlay_demo.rs). The reusable UI lives in [`overlay`](../crates/owl-ui/src/overlay).

## Three directions of communication

```text
Backend ──CoreMessage──> Overlay
                       ▲
Widgets ──Message──────┘
                       │
Host <────Output───────┘
```

Each type has a distinct job:

- `CoreMessage` carries backend updates into the overlay, such as `Show`, `Target`, `Token`, `Done`, and `Error`. `Overlay::apply` folds these updates into UI state.
- `overlay::Message` describes something that happened inside the UI, such as editing the prompt, requesting back, or requesting dismissal. `Overlay::update` handles it.
- `overlay::Output` tells the host that the overlay needs something outside its control, such as submitting to the backend, resizing after a phase change, or hiding the native window.

Messages flow inward; outputs bubble outward.

The distinction is visible in the existing models. The host owns the overlay alongside native resources:

```rust
struct Demo {
    overlay: Overlay,
    to_core: Option<mpsc::Sender<Submit>>,
    window: Option<window::Id>,
}
```

The overlay contains UI state but no window ID or backend sender:

```rust
pub struct Overlay {
    pub visible: bool,
    pub target: Option<Target>,
    prompting: prompting::Prompting,
    pub phase: Phase,
    pub answer: String,
    pub done: bool,
    pub error: Option<String>,
}
```

The types crossing those boundaries are [`CoreMessage`](../crates/owl-types/src/lib.rs), [`Message`, and `Output`](../crates/owl-ui/src/overlay/mod.rs):

```rust
pub enum CoreMessage {
    Show,
    Target(Target),
    Token(String),
    Done,
    Error(String),
}

pub enum Message {
    Prompting(prompting::Message),
    BackRequested,
    DismissRequested,
}

pub enum Output {
    Submitted(Submit),
    PhaseChanged(Phase),
    Dismissed,
}
```

## What triggers and consumes each value

| Cause | Value produced | Consumer | Result |
| --- | --- | --- | --- |
| User types in the input | `prompting::Message::InputChanged` | `Prompting::update` | Stores the new prompt text |
| User presses Enter or the send button | `prompting::Message::SubmitRequested` | `Overlay::update` | Changes phase and emits `Output::Submitted` |
| Backend sends answer text | `CoreMessage::Token` | `Overlay::apply` | Appends text to the answer |
| User presses back | `overlay::Message::BackRequested` | `Overlay::update` | Emits `Output::PhaseChanged` |
| User presses close | `overlay::Message::DismissRequested` | `Overlay::update` | Emits `Output::Dismissed` |
| Hotkey/backend requests display | `CoreMessage::Show` | Host, then `Overlay::apply` | Restores, focuses, and marks the overlay visible |

For example, Iced creates prompt messages from the widget callbacks in [`prompting.rs`](../crates/owl-ui/src/overlay/prompting.rs):

```rust
text_input("", &self.prompt_value)
    .on_input(Message::InputChanged)
    .on_submit(Message::SubmitRequested);

button(send_icon).on_press(Message::SubmitRequested);
```

`Overlay::view` wraps those child messages so the overlay update loop can receive them:

```rust
self.prompting.view().map(Message::Prompting)
```

The submission arm in `Overlay::update` digests that message, updates local state, and returns an output:

```rust
Message::Prompting(prompting::Message::SubmitRequested) => {
    self.phase = Phase::Answering;
    self.answer.clear();
    Some(Output::Submitted(self.commit()))
}
```

The host digests the output. For `Submitted`, it sends the data to the backend and resizes the native window. For `PhaseChanged`, it resizes to the matching phase. For `Dismissed`, it hides the window with `Mode::Hidden` instead of terminating the process.

Backend traffic takes the other route. The host receives `Message::Core(core_message)` and calls `overlay.apply(core_message)`. Inside `apply`, for example:

```rust
CoreMessage::Token(token) => self.answer.push_str(&token),
CoreMessage::Done => self.done = true,
CoreMessage::Error(error) => self.error = Some(error),
```

When a later `CoreMessage::Show` arrives, the host restores the same window to `Mode::Windowed` and focuses it, while `Overlay::apply` sets `visible = true`. The window remains configured as `AlwaysOnTop`.

This boundary keeps OS behavior out of the reusable UI while still letting the overlay request the effects it needs.
