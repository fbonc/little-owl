//! Throwaway host for the overlay, wired like the real app will be. Two channels
//! bridge the mock backend and the UI, exactly as owl-core will: core -> ui carries
//! UiEvents (a stream the subscription drains and folds via `apply`), ui -> core
//! carries the submitted ask. Delete this once owl-app hosts owl_ui for real.
//!
//!   cargo run -p owl-ui --example overlay_demo

use std::time::Duration;

use iced::futures::channel::mpsc;
use iced::futures::{SinkExt, Stream, StreamExt};
use iced::widget::mouse_area;
use iced::{Element, Subscription, Task, Theme, window};
use tokio::time::sleep;

use owl_types::{TextCapture, TextCaptureMethod};
use owl_ui::overlay;
use owl_ui::{Overlay, OverlayOutput, Submit, Target, UiEvent};

const WINDOW_WIDTH: f32 = 640.0;
const MIN_WINDOW_WIDTH: f32 = 520.0;
// Compact while asking; a fixed roomier panel while answering. The window resizes
// once on submit and never during the stream, so the text just scrolls inside it.
const ASKING_HEIGHT: f32 = 150.0;
const MIN_WINDOW_HEIGHT: f32 = ASKING_HEIGHT;
const ANSWER_HEIGHT: f32 = 360.0;

struct Demo {
    overlay: Overlay,
    // Send half of the ui -> core channel; None until the backend hands it over.
    to_core: Option<mpsc::Sender<Submit>>,
    window: Option<window::Id>,
}

#[derive(Debug, Clone)]
enum Message {
    WindowOpened(Option<window::Id>),
    DragWindow,
    Ready(mpsc::Sender<Submit>),
    Core(UiEvent),
    Overlay(overlay::Message),
    CheckPromptInputFocus,
}

fn main() -> iced::Result {
    iced::application(
        || {
            (
                Demo {
                    overlay: Overlay::new(),
                    to_core: None,
                    window: None,
                },
                window::latest().map(Message::WindowOpened),
            )
        },
        update,
        view,
    )
    .subscription(subscription)
    .theme(|_state: &Demo| Theme::Dark)
    .title("little owl")
    .window(window::Settings {
        size: iced::Size::new(WINDOW_WIDTH, ASKING_HEIGHT),
        min_size: Some(iced::Size::new(MIN_WINDOW_WIDTH, MIN_WINDOW_HEIGHT)),
        position: window::Position::Centered,
        decorations: false,
        transparent: true,
        resizable: true,
        level: window::Level::AlwaysOnTop,
        exit_on_close_request: false,
        ..Default::default()
    })
    .style(|_state: &Demo, _theme: &iced::Theme| iced::theme::Style {
        background_color: iced::Color::TRANSPARENT,
        text_color: iced::Color::BLACK,
    })
    .run()
}

fn update(state: &mut Demo, message: Message) -> Task<Message> {
    match message {
        Message::WindowOpened(id) => {
            state.window = id;
            Task::none()
        }
        Message::DragWindow => match state.window {
            Some(id) => window::drag(id),
            None => Task::none(),
        },
        Message::Ready(sender) => {
            state.to_core = Some(sender);
            Task::none()
        }
        // No resizing while text streams — the answer scrolls inside a fixed panel.
        Message::Core(event) => {
            let should_show = matches!(event, UiEvent::Show);
            state.overlay.apply(event);

            match (should_show, state.window) {
                (true, Some(id)) => {
                    window::set_mode(id, window::Mode::Windowed).chain(window::gain_focus(id))
                }
                _ => Task::none(),
            }
        }
        Message::Overlay(message) => match state.overlay.update(message) {
            Some(OverlayOutput::Submitted(submit)) => {
                if let Some(sender) = &mut state.to_core {
                    let _ = sender.try_send(submit);
                }
                match state.window {
                    Some(id) => window::resize(id, iced::Size::new(WINDOW_WIDTH, ANSWER_HEIGHT)),
                    None => Task::none(),
                }
            }
            Some(OverlayOutput::PhaseChanged(phase)) => match (phase, state.window) {
                (owl_ui::Phase::Prompting, Some(id)) => {
                    window::resize(id, iced::Size::new(WINDOW_WIDTH, ASKING_HEIGHT))
                }
                (owl_ui::Phase::Answering, Some(id)) => {
                    window::resize(id, iced::Size::new(WINDOW_WIDTH, ANSWER_HEIGHT))
                }
                (_, None) => Task::none(),
            },
            Some(OverlayOutput::Dismissed) => match state.window {
                Some(id) => window::set_mode(id, window::Mode::Hidden),
                None => Task::none(),
            },
            None => Task::none(),
        },
        Message::CheckPromptInputFocus => {
            if state.overlay.visible && state.overlay.phase == owl_ui::Phase::Prompting {
                state
                    .overlay
                    .check_prompt_input_focus()
                    .map(Message::Overlay)
            } else {
                Task::none()
            }
        }
    }
}

fn view(state: &Demo) -> Element<'_, Message> {
    // The whole panel is a drag handle (borderless: no title bar to grab).
    mouse_area(owl_ui::view(&state.overlay).map(Message::Overlay))
        .on_press(Message::DragWindow)
        .into()
}

fn subscription(_state: &Demo) -> Subscription<Message> {
    Subscription::batch([
        Subscription::run(mock_core),
        iced::event::listen_with(|event, _status, _window| match event {
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(_))
            | iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { .. }) => {
                Some(Message::CheckPromptInputFocus)
            }
            iced::Event::Window(window::Event::CloseRequested) => {
                Some(Message::Overlay(overlay::Message::DismissRequested))
            }

            _ => None,
        }),
    ])
}

/// Stand-in for owl-core. Opens the ui -> core channel and hands the UI its sender,
/// streams the opening Show + Target, then streams an answer for each ask it
/// receives. Same shape as the real core; only the contents are canned.
fn mock_core() -> impl Stream<Item = Message> {
    iced::stream::channel(16, |mut to_ui: mpsc::Sender<Message>| async move {
        let (to_core, mut asks) = mpsc::channel::<Submit>(1);
        let _ = to_ui.send(Message::Ready(to_core)).await;

        let _ = to_ui.send(Message::Core(UiEvent::Show)).await;
        let _ = to_ui
            .send(Message::Core(UiEvent::Target(sample_target())))
            .await;

        while let Some(submit) = asks.next().await {
            for event in answer_events(submit.prompt.as_deref()) {
                let _ = to_ui.send(Message::Core(event)).await;
                sleep(Duration::from_millis(55)).await;
            }
        }
    })
}

fn sample_target() -> Target {
    Target::Text(TextCapture {
        text: "epistemic uncertainty".into(),
        method: TextCaptureMethod::Accessibility,
    })
}

const NONE_ANSWER: &str = "
A transformer is a neural network architecture designed to process sequences such as text. It is the basic architecture behind many modern language models.

The main idea is attention.

Instead of reading a sentence strictly one word at a time, a transformer can examine many words at once and determine which words are relevant to each other.

For example, in the sentence:

The dog chased the ball because it was moving.

When processing the word “it,” the model can use attention to decide that “it” is strongly related to “the ball.”

Before attention happens, each word or token is converted into a vector called an embedding. An embedding is just a collection of numbers that represents information about that token.

Because transformers process tokens in parallel, they also need information about word order. This is added using positional information, so the model can distinguish between sentences such as:

Dog bites man.

and

Man bites dog.

The central mechanism is called self-attention.

For every token, the transformer creates three representations called a query, a key, and a value.

The query roughly represents what the token is looking for.

The key represents what information a token contains or how it can be matched.

The value represents the information that can actually be passed along.

The transformer compares each token’s query with the keys of other tokens. Tokens with stronger matches receive more attention. Their values are then combined to create a new representation of the current token.

Transformers usually use multi-head attention. This means several attention mechanisms operate at the same time. Different heads can learn different kinds of relationships. One might focus on grammar, another on nearby words, and another on long-distance relationships.

After the attention step, the information passes through a small feed-forward neural network. This lets the model transform and refine the information it gathered through attention.

A transformer contains many of these layers stacked on top of each other. Early layers may learn relatively simple patterns, while later layers can represent increasingly complex relationships.

Two other important components are residual connections and layer normalization. Residual connections help information flow through many layers without being lost, while layer normalization helps keep the network numerically stable during training.

The original Transformer architecture had two major parts: an encoder and a decoder.

The encoder reads and builds representations of input text.

The decoder generates output text while paying attention to both previously generated tokens and information from the encoder.

Translation systems often use this encoder-decoder structure.

Models like the original GPT family primarily use the decoder part of the Transformer. They generate text one token at a time. When predicting the next token, they are prevented from looking at future tokens. This is called causal or masked self-attention.

A simplified GPT-style process looks like this:

Text is split into tokens.

Tokens become numerical embeddings.

Positional information is added.

The embeddings pass through many transformer layers.

Each layer performs self-attention and feed-forward processing.

The final representation is converted into probabilities for possible next tokens.

One token is selected.

That token is added to the sequence.

The process repeats.

So, in very simple terms, a transformer repeatedly asks:

What parts of the text should I pay attention to right now?

It uses the answer to build richer representations of the text and, in a language model, predict what should come next.
";

fn answer_events(ask: Option<&str>) -> Vec<UiEvent> {
    let body = match ask {
        Some(ask) => format!("You asked: {ask}. Here is a mocked streamed reply."),
        None => NONE_ANSWER.to_string(),
    };
    let mut events: Vec<UiEvent> = body
        .split_inclusive(' ')
        .map(|token| UiEvent::Token(token.to_string()))
        .collect();
    events.push(UiEvent::Done);
    events
}
