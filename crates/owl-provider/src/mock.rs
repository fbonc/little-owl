use std::time::Duration;

use futures_util::{StreamExt, stream};
use owl_types::Target;

use crate::{Provider, ProviderCapabilities, ProviderOutput, ProviderRequest, ProviderStream};

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

#[derive(Debug, Clone)]
pub struct MockProvider {
    chunk_delay: Duration,
}

impl MockProvider {
    pub fn new(chunk_delay: Duration) -> Self {
        Self { chunk_delay }
    }

    fn answer(request: &ProviderRequest) -> String {
        let has_image = matches!(request.target, Some(Target::Image(_)));

        match (request.prompt.as_deref(), has_image) {
            (Some(prompt), true) => format!(
                "You asked: {prompt}. Image target captured. Here is a mocked streamed reply."
            ),
            (None, true) => "Image target captured. Here is a mocked streamed reply.".to_owned(),
            (Some(prompt), false) => {
                format!("You asked: {prompt}. Here is a mocked streamed reply.")
            }
            (None, false) => DEFAULT_ANSWER.to_owned(),
        }
    }
}

impl Default for MockProvider {
    fn default() -> Self {
        Self::new(Duration::from_millis(55))
    }
}

impl Provider for MockProvider {
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities { image_input: true }
    }

    fn stream(&self, request: ProviderRequest) -> ProviderStream {
        let delay = self.chunk_delay;
        let chunks = Self::answer(&request)
            .split_inclusive(' ')
            .map(|chunk| Ok(ProviderOutput::TextDelta(chunk.to_owned())))
            .collect::<Vec<_>>();

        stream::iter(chunks)
            .enumerate()
            .then(move |(index, output)| async move {
                if index > 0 && !delay.is_zero() {
                    tokio::time::sleep(delay).await;
                }
                output
            })
            .boxed()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use futures_executor::block_on;
    use futures_util::StreamExt;
    use owl_types::{ImageCapture, Target};

    use super::*;

    #[test]
    fn image_and_prompt_reach_mock_reply() {
        let request = ProviderRequest {
            prompt: Some("describe this".into()),
            target: Some(Target::Image(ImageCapture {
                png: b"png".to_vec(),
                region: None,
            })),
            context: None,
        };
        let reply = block_on(
            MockProvider::new(Duration::ZERO)
                .stream(request)
                .map(Result::unwrap)
                .map(|output| match output {
                    ProviderOutput::TextDelta(chunk) => chunk,
                })
                .collect::<String>(),
        );

        assert!(reply.contains("Image target captured"));
        assert!(reply.contains("describe this"));
    }
}
