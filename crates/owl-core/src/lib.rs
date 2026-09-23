use std::thread;
use std::time::Duration;

use futures_channel::mpsc;
use futures_util::{Stream, StreamExt, stream};
use owl_capture::new_capturer;
use owl_types::{ImageCapture, Target};
use tokio::time::sleep;

const HOTKEY_ACCELERATOR: &str = "Ctrl+Shift+KeyA";

pub type Sender = mpsc::Sender<Input>;

#[derive(Debug, Clone)]
pub enum Input {
    Submit {
        prompt: Option<String>,
        image: Option<ImageCapture>,
    },
    SelectRegion,
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

pub fn start() -> (Sender, impl Stream<Item = Output>) {
    let (sender, inputs) = mpsc::channel(1);
    let hotkey_outputs = hotkey_outputs();
    let responses = inputs.flat_map(|input| match input {
        Input::Submit { prompt, image } => {
            stream::iter(answer_events(prompt.as_deref(), image.as_ref()))
                .enumerate()
                .then(|(index, output)| async move {
                    if index > 0 {
                        sleep(Duration::from_millis(55)).await;
                    }
                    output
                })
                .boxed()
        }
        Input::SelectRegion => stream::once(async {
            let result = tokio::task::spawn_blocking(|| {
                new_capturer()
                    .select_region()
                    .map_err(|error| error.to_string())
            })
            .await
            .map_err(|error| format!("region selection task failed: {error}"))
            .and_then(|result| result);
            Output::RegionSelectionFinished(result)
        })
        .boxed(),
    });

    (sender, stream::select(hotkey_outputs, responses))
}

fn hotkey_outputs() -> impl Stream<Item = Output> {
    let (outputs, receiver) = mpsc::unbounded();

    match owl_hotkey::new_hotkey(HOTKEY_ACCELERATOR) {
        Ok(hotkey) => {
            thread::spawn(move || {
                loop {
                    match hotkey.recv() {
                        Ok(()) => {
                            let capturer = new_capturer();
                            let capture = match capturer.capture_text() {
                                Ok(c) => Some(Target::Text(c)),
                                Err(error) => {
                                    let _ = outputs.unbounded_send(Output::RequestFailed(format!(
                                        "failed to capture text: {error:?}"
                                    )));
                                    None
                                }
                            };

                            if outputs.unbounded_send(Output::ShowRequested).is_err() {
                                break;
                            }

                            if let Some(capture) = capture
                                && outputs
                                    .unbounded_send(Output::TargetCaptured(capture))
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

fn answer_events(prompt: Option<&str>, image: Option<&ImageCapture>) -> Vec<Output> {
    let answer = match (prompt, image) {
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
    #[test]
    fn image_reaches_mock_reply() {
        let image = ImageCapture {
            png: b"png".to_vec(),
            region: None,
        };
        let reply = answer_events(Some("describe this"), Some(&image))
            .into_iter()
            .filter_map(|event| match event {
                Output::AnswerChunk(chunk) => Some(chunk),
                _ => None,
            })
            .collect::<String>();

        assert!(reply.contains("Image target captured"));
        assert!(reply.contains("describe this"));
    }
}
