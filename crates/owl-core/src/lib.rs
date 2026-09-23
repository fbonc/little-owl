use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use futures_channel::mpsc;
use futures_util::{Stream, StreamExt, stream};
use owl_capture::new_capturer;
use owl_types::{Capture, ContextCapture, ImageCapture, Target};
use tokio::time::sleep;

const HOTKEY_ACCELERATOR: &str = "Ctrl+Shift+KeyA";

pub type Sender = mpsc::Sender<Input>;

#[derive(Debug, Clone)]
pub enum Input {
    Submit { prompt: Option<String> },
    SelectRegion,
}

#[derive(Debug, Clone)]
pub struct ProviderRequest {
    pub prompt: Option<String>,
    pub target: Option<Target>,
    pub context: Option<ContextCapture>,
}

#[derive(Debug, Clone)]
pub enum Output {
    ShowRequested,
    TargetCaptured(Target),
    RegionSelectionFinished(Result<Option<ImageCapture>, String>),
    AnswerChunk(String),
    AnswerCompleted,
    RequestFailed(String),
}

#[derive(Debug, Default)]
struct CoreState {
    capture_id: u64,
    current_capture: Option<Capture>,
}

impl CoreState {
    fn begin_capture(&mut self) -> u64 {
        self.capture_id = self.capture_id.wrapping_add(1);
        self.current_capture = None;
        self.capture_id
    }

    fn finish_capture(&mut self, capture_id: u64, capture: Capture) {
        if self.capture_id == capture_id {
            self.current_capture = Some(capture);
        }
    }

    fn apply_region(&mut self, capture_id: u64, image: ImageCapture) -> bool {
        if self.capture_id != capture_id {
            return false;
        }

        let Some(capture) = &mut self.current_capture else {
            return false;
        };

        capture.target = Some(Target::Image(image));
        true
    }

    fn provider_request(&self, prompt: Option<String>) -> ProviderRequest {
        ProviderRequest {
            prompt,
            target: self
                .current_capture
                .as_ref()
                .and_then(|capture| capture.target.clone()),
            context: self
                .current_capture
                .as_ref()
                .and_then(|capture| capture.context.clone()),
        }
    }
}

pub fn start() -> (Sender, impl Stream<Item = Output>) {
    let (sender, inputs) = mpsc::channel(1);
    let state = Arc::new(Mutex::new(CoreState::default()));
    let hotkey_outputs = hotkey_outputs(Arc::clone(&state));
    let responses = inputs.flat_map(move |input| {
        let state = Arc::clone(&state);

        match input {
            Input::Submit { prompt } => {
                let request = state
                    .lock()
                    .expect("core state mutex poisoned")
                    .provider_request(prompt);

                stream::iter(answer_events(&request))
                    .enumerate()
                    .then(|(index, output)| async move {
                        if index > 0 {
                            sleep(Duration::from_millis(55)).await;
                        }
                        output
                    })
                    .boxed()
            }
            Input::SelectRegion => {
                let capture_id = state.lock().expect("core state mutex poisoned").capture_id;

                stream::once(async move {
                    let result = tokio::task::spawn_blocking(|| {
                        new_capturer()
                            .select_region()
                            .map_err(|error| error.to_string())
                    })
                    .await
                    .map_err(|error| format!("region selection task failed: {error}"))
                    .and_then(|result| result);

                    let result = match result {
                        Ok(Some(image)) => {
                            let applied = state
                                .lock()
                                .expect("core state mutex poisoned")
                                .apply_region(capture_id, image.clone());
                            Ok(applied.then_some(image))
                        }
                        result => result,
                    };

                    Output::RegionSelectionFinished(result)
                })
                .boxed()
            }
        }
    });

    (sender, stream::select(hotkey_outputs, responses))
}

fn hotkey_outputs(state: Arc<Mutex<CoreState>>) -> impl Stream<Item = Output> {
    let (outputs, receiver) = mpsc::unbounded();

    match owl_hotkey::new_hotkey(HOTKEY_ACCELERATOR) {
        Ok(hotkey) => {
            thread::spawn(move || {
                loop {
                    match hotkey.recv() {
                        Ok(()) => {
                            let capture_id = state
                                .lock()
                                .expect("core state mutex poisoned")
                                .begin_capture();
                            let started = Instant::now();
                            let capturer = new_capturer();
                            let provenance = capturer.capture_provenance().ok();
                            let target = match capturer.capture_text() {
                                Ok(capture) => Some(Target::Text(capture)),
                                Err(error) => {
                                    let _ = outputs.unbounded_send(Output::RequestFailed(format!(
                                        "failed to capture text: {error:?}"
                                    )));
                                    None
                                }
                            };
                            let context = capturer.capture_context().ok();
                            let elapsed_ms =
                                u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX);
                            let capture = Capture {
                                target: target.clone(),
                                context,
                                provenance,
                                elapsed_ms,
                            };

                            state
                                .lock()
                                .expect("core state mutex poisoned")
                                .finish_capture(capture_id, capture);

                            if outputs.unbounded_send(Output::ShowRequested).is_err() {
                                break;
                            }

                            if let Some(target) = target
                                && outputs
                                    .unbounded_send(Output::TargetCaptured(target))
                                    .is_err()
                            {
                                break;
                            }
                        }
                        Err(error) => {
                            let _ = outputs.unbounded_send(Output::RequestFailed(format!(
                                "failed to receive hotkey: {error:?}"
                            )));
                            break;
                        }
                    };
                }
            });
        }
        Err(error) => {
            let _ = outputs.unbounded_send(Output::RequestFailed(format!(
                "failed to register {HOTKEY_ACCELERATOR}: {error:?}"
            )));
        }
    }

    receiver
}

const DEFAULT_ANSWER: &str = r"
## Epistemic Uncertainty

**Epistemic uncertainty** is uncertainty caused by a **lack of knowledge**. It arises when we do not know enough about a system, its parameters, its underlying mechanisms, or the relevant facts.

The key feature of epistemic uncertainty is that it is **potentially reducible**. By collecting more data, making better measurements, running experiments, or improving our models, we can often decrease it.

### Example

Suppose you find a coin but do not know whether it is fair. You might be uncertain whether:

$$
P(\text{heads}) = 0.5
$$

or perhaps \(0.6\), \(0.7\), or some other value.

Your uncertainty about the coin's true probability of heads is **epistemic uncertainty**. Tossing the coin many times could help you estimate that probability more accurately.

This contrasts with **aleatoric uncertainty**, which comes from inherent randomness. Even if you know with certainty that the coin is fair, you still cannot know whether the *next* toss will be heads or tails.

So, roughly:

* **Epistemic uncertainty:** *We don't know enough.*
* **Aleatoric uncertainty:** *The outcome itself is variable or random.*

### Where It Appears

Epistemic uncertainty commonly comes from:

* **Parameter uncertainty:** not knowing the exact values used in a model.
* **Model uncertainty:** not knowing whether the model itself is correct.
* **Measurement uncertainty:** imperfect observations or instruments.
* **Missing knowledge:** not knowing all relevant variables or mechanisms.

For example, a machine-learning model may have high epistemic uncertainty when it encounters data very different from anything in its training set.

### Why It Matters

Recognizing epistemic uncertainty helps prevent **false confidence**. A precise prediction is not necessarily a well-supported prediction.

It also tells us when gathering more information is valuable. If uncertainty is epistemic, additional research or evidence may substantially improve a decision.

In short:
> **Epistemic uncertainty describes what we do not know—and, importantly, what we may be able to learn.**
";

fn answer_events(request: &ProviderRequest) -> Vec<Output> {
    let image = match &request.target {
        Some(Target::Image(image)) => Some(image),
        _ => None,
    };
    let answer = match (request.prompt.as_deref(), image) {
        (Some(prompt), Some(_)) => {
            format!("You asked: {prompt}. Image target captured. Here is a mocked streamed reply.")
        }
        (None, Some(_)) => "Image target captured. Here is a mocked streamed reply.".to_owned(),
        (Some(prompt), _) => format!("You asked: {prompt}. Here is a mocked streamed reply."),
        (None, _) => DEFAULT_ANSWER.to_owned(),
    };
    let mut events = answer
        .split_inclusive(' ')
        .map(|token| Output::AnswerChunk(token.to_owned()))
        .collect::<Vec<_>>();
    events.push(Output::AnswerCompleted);
    events
}

#[cfg(test)]
mod tests {
    use super::*;
    use owl_types::{ContextCaptureMethod, Provenance, TextCapture, TextCaptureMethod};

    fn image() -> ImageCapture {
        ImageCapture {
            png: b"png".to_vec(),
            region: None,
        }
    }

    #[test]
    fn image_reaches_mock_reply() {
        let request = ProviderRequest {
            prompt: Some("describe this".into()),
            target: Some(Target::Image(image())),
            context: None,
        };
        let reply = answer_events(&request)
            .into_iter()
            .filter_map(|event| match event {
                Output::AnswerChunk(chunk) => Some(chunk),
                _ => None,
            })
            .collect::<String>();

        assert!(reply.contains("Image target captured"));
        assert!(reply.contains("describe this"));
    }

    #[test]
    fn provider_request_uses_prompt_target_and_context() {
        let target = Target::Text(TextCapture {
            text: "selected text".into(),
            method: TextCaptureMethod::Accessibility,
        });
        let context = ContextCapture::Text {
            text: "surrounding context".into(),
            method: ContextCaptureMethod::Accessibility,
        };
        let mut state = CoreState::default();
        let capture_id = state.begin_capture();
        state.finish_capture(
            capture_id,
            Capture {
                target: Some(target),
                context: Some(context),
                provenance: Some(Provenance {
                    app_name: "Example".into(),
                    window_title: "Document".into(),
                    path: Some("/tmp/example".into()),
                }),
                elapsed_ms: 12,
            },
        );

        let request = state.provider_request(Some("explain this".into()));

        assert_eq!(request.prompt.as_deref(), Some("explain this"));
        assert!(matches!(
            request.target,
            Some(Target::Text(TextCapture { ref text, .. })) if text == "selected text"
        ));
        assert!(matches!(
            request.context,
            Some(ContextCapture::Text { ref text, .. }) if text == "surrounding context"
        ));
    }

    #[test]
    fn beginning_a_capture_discards_the_previous_capture() {
        let mut state = CoreState::default();
        let first_id = state.begin_capture();
        state.finish_capture(
            first_id,
            Capture {
                target: Some(Target::Image(image())),
                context: None,
                provenance: None,
                elapsed_ms: 0,
            },
        );

        state.begin_capture();

        assert!(state.current_capture.is_none());
    }

    #[test]
    fn stale_region_capture_does_not_replace_the_current_target() {
        let mut state = CoreState::default();
        let stale_id = state.begin_capture();
        let current_id = state.begin_capture();
        state.finish_capture(
            current_id,
            Capture {
                target: Some(Target::Text(TextCapture {
                    text: "current target".into(),
                    method: TextCaptureMethod::Accessibility,
                })),
                context: None,
                provenance: None,
                elapsed_ms: 0,
            },
        );

        assert!(!state.apply_region(stale_id, image()));
        assert!(matches!(
            state.current_capture.and_then(|capture| capture.target),
            Some(Target::Text(TextCapture { ref text, .. })) if text == "current target"
        ));
    }

    #[test]
    fn region_capture_replaces_text_target_and_preserves_context() {
        let context = ContextCapture::Text {
            text: "surrounding context".into(),
            method: ContextCaptureMethod::Accessibility,
        };
        let mut state = CoreState::default();
        let capture_id = state.begin_capture();
        state.finish_capture(
            capture_id,
            Capture {
                target: Some(Target::Text(TextCapture {
                    text: "selected text".into(),
                    method: TextCaptureMethod::Accessibility,
                })),
                context: Some(context),
                provenance: None,
                elapsed_ms: 0,
            },
        );

        assert!(state.apply_region(capture_id, image()));
        let request = state.provider_request(None);

        assert!(matches!(request.target, Some(Target::Image(_))));
        assert!(matches!(
            request.context,
            Some(ContextCapture::Text { ref text, .. }) if text == "surrounding context"
        ));
    }
}
