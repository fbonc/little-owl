use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use futures_channel::mpsc;
use futures_util::future::{AbortHandle, AbortRegistration, Abortable, ready};
use futures_util::stream::BoxStream;
use futures_util::{Stream, StreamExt, stream};
use owl_capture::{Capturer, new_capturer};
use owl_provider::{MockProvider, Provider, ProviderOutput, ProviderRequest, ProviderStream};
use owl_types::{ContextCapture, ImageCapture, Provenance, Target};

const HOTKEY_ACCELERATOR: &str = "Ctrl+Shift+KeyA";

pub type Sender = mpsc::Sender<Input>;

#[derive(Debug, Clone)]
pub enum Input {
    Submit { prompt: Option<String> },
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

#[derive(Debug, Clone)]
pub struct Capture {
    pub target: Option<Target>,
    pub context: Option<ContextCapture>,
    pub provenance: Option<Provenance>,
    pub elapsed_ms: u32,
}

#[derive(Debug, Default)]
struct State {
    capture_id: u64,
    current_capture: Option<Capture>,
    provider_request_id: u64,
    provider_abort: Option<AbortHandle>,
}

impl State {
    fn start_capture(&mut self) -> u64 {
        self.cancel_provider_request();
        self.capture_id = self.capture_id.wrapping_add(1);
        self.current_capture = None;
        self.capture_id
    }

    fn commit_capture(&mut self, capture_id: u64, capture: Capture) {
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

    fn start_provider_request(
        &mut self,
        prompt: Option<String>,
    ) -> (ProviderRequest, u64, AbortRegistration) {
        self.cancel_provider_request();
        let request = ProviderRequest {
            prompt,
            target: self
                .current_capture
                .as_ref()
                .and_then(|capture| capture.target.clone()),
            context: self
                .current_capture
                .as_ref()
                .and_then(|capture| capture.context.clone()),
        };
        let (abort, registration) = AbortHandle::new_pair();
        self.provider_abort = Some(abort);
        (request, self.provider_request_id, registration)
    }

    fn cancel_provider_request(&mut self) {
        self.provider_request_id = self.provider_request_id.wrapping_add(1);
        if let Some(abort) = self.provider_abort.take() {
            abort.abort();
        }
    }

    fn accept_provider_output(&mut self, request_id: u64, output: &Output) -> bool {
        if self.provider_request_id != request_id || self.provider_abort.is_none() {
            return false;
        }

        if matches!(output, Output::AnswerCompleted | Output::RequestFailed(_)) {
            self.provider_abort = None;
        }

        true
    }
}

struct Core {
    state: Mutex<State>,
    provider: Arc<dyn Provider>,
    capturer: Arc<dyn Capturer>,
}

impl Core {
    fn new(provider: Arc<dyn Provider>, capturer: Arc<dyn Capturer>) -> Self {
        Self {
            state: Mutex::new(State::default()),
            provider,
            capturer,
        }
    }

    fn capture(&self) -> CaptureOutcome {
        let capture_id = self
            .state
            .lock()
            .expect("core state mutex poisoned")
            .start_capture();
        let started = Instant::now();
        let provenance = self.capturer.capture_provenance().ok();
        let (target, failure) = match self.capturer.capture_text() {
            Ok(capture) => (Some(Target::Text(capture)), None),
            Err(error) => (None, Some(format!("failed to capture text: {error:?}"))),
        };
        let context = self.capturer.capture_context().ok();
        let elapsed_ms = u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX);
        let capture = Capture {
            target: target.clone(),
            context,
            provenance,
            elapsed_ms,
        };

        self.state
            .lock()
            .expect("core state mutex poisoned")
            .commit_capture(capture_id, capture);

        CaptureOutcome { target, failure }
    }

    fn select_region(&self) -> Result<Option<ImageCapture>, String> {
        let capture_id = self
            .state
            .lock()
            .expect("core state mutex poisoned")
            .capture_id;
        let result = self
            .capturer
            .select_region()
            .map_err(|error| error.to_string())?;

        let Some(image) = result else {
            return Ok(None);
        };
        let applied = self
            .state
            .lock()
            .expect("core state mutex poisoned")
            .apply_region(capture_id, image.clone());

        Ok(applied.then_some(image))
    }

    fn start_provider_stream(
        self: &Arc<Self>,
        prompt: Option<String>,
    ) -> BoxStream<'static, Output> {
        let (request, request_id, abort_registration) = self
            .state
            .lock()
            .expect("core state mutex poisoned")
            .start_provider_request(prompt);
        let outputs = stream::unfold(
            ProviderResponseState::Streaming(self.provider.stream(request)),
            |response| async move {
                match response {
                    ProviderResponseState::Streaming(mut stream) => match stream.next().await {
                        Some(Ok(ProviderOutput::TextDelta(chunk))) => Some((
                            Output::AnswerChunk(chunk),
                            ProviderResponseState::Streaming(stream),
                        )),
                        Some(Err(error)) => Some((
                            Output::RequestFailed(error.to_string()),
                            ProviderResponseState::Finished,
                        )),
                        None => Some((Output::AnswerCompleted, ProviderResponseState::Finished)),
                    },
                    ProviderResponseState::Finished => None,
                }
            },
        );
        let core = Arc::clone(self);

        Abortable::new(outputs, abort_registration)
            .filter_map(move |output| {
                let accepted = core.accept_provider_output(request_id, &output);
                ready(accepted.then_some(output))
            })
            .boxed()
    }

    fn accept_provider_output(&self, request_id: u64, output: &Output) -> bool {
        self.state
            .lock()
            .expect("core state mutex poisoned")
            .accept_provider_output(request_id, output)
    }
}

struct CaptureOutcome {
    target: Option<Target>,
    failure: Option<String>,
}

pub fn start() -> (Sender, impl Stream<Item = Output>) {
    start_with_provider(Arc::new(MockProvider::default()))
}

pub fn start_with_provider(provider: Arc<dyn Provider>) -> (Sender, impl Stream<Item = Output>) {
    let (sender, inputs) = mpsc::channel(1);
    let core = Arc::new(Core::new(provider, Arc::from(new_capturer())));
    let hotkey_outputs = hotkey_outputs(Arc::clone(&core));
    let responses = inputs.flat_map(move |input| {
        let core = Arc::clone(&core);

        match input {
            Input::Submit { prompt } => core.start_provider_stream(prompt),
            Input::SelectRegion => stream::once(async move {
                let result = tokio::task::spawn_blocking(move || core.select_region())
                    .await
                    .map_err(|error| format!("region selection task failed: {error}"))
                    .and_then(|result| result);

                Output::RegionSelectionFinished(result)
            })
            .boxed(),
        }
    });

    (sender, stream::select(hotkey_outputs, responses))
}

enum ProviderResponseState {
    Streaming(ProviderStream),
    Finished,
}

fn hotkey_outputs(core: Arc<Core>) -> impl Stream<Item = Output> {
    let (outputs, receiver) = mpsc::unbounded();

    match owl_hotkey::new_hotkey(HOTKEY_ACCELERATOR) {
        Ok(hotkey) => {
            thread::spawn(move || {
                loop {
                    match hotkey.recv() {
                        Ok(()) => {
                            let result = core.capture();

                            if let Some(error) = result.failure {
                                let _ = outputs.unbounded_send(Output::RequestFailed(error));
                            }

                            if outputs.unbounded_send(Output::ShowRequested).is_err() {
                                break;
                            }

                            if let Some(target) = result.target
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

#[cfg(test)]
mod tests {
    use super::*;
    use owl_types::{
        ContextCapture, ContextCaptureMethod, Provenance, TextCapture, TextCaptureMethod,
    };

    struct StubCapturer;

    impl Capturer for StubCapturer {
        fn capture_text(&self) -> owl_capture::Result<TextCapture> {
            Ok(TextCapture {
                text: "selected text".into(),
                method: TextCaptureMethod::Accessibility,
            })
        }

        fn capture_context(&self) -> owl_capture::Result<ContextCapture> {
            Ok(ContextCapture::Text {
                text: "surrounding context".into(),
                method: ContextCaptureMethod::Accessibility,
            })
        }

        fn select_region(&self) -> owl_capture::Result<Option<ImageCapture>> {
            Ok(Some(image()))
        }

        fn capture_provenance(&self) -> owl_capture::Result<Provenance> {
            Ok(Provenance {
                app_name: "Example".into(),
                window_title: "Document".into(),
                path: Some("/tmp/example".into()),
            })
        }
    }

    fn image() -> ImageCapture {
        ImageCapture {
            png: b"png".to_vec(),
            region: None,
        }
    }

    #[test]
    fn core_capture_orchestrates_and_commits_capture_data() {
        let core = Core::new(Arc::new(MockProvider::default()), Arc::new(StubCapturer));

        let outcome = core.capture();

        assert!(outcome.failure.is_none());
        assert!(matches!(
            outcome.target,
            Some(Target::Text(TextCapture { ref text, .. })) if text == "selected text"
        ));

        let state = core.state.lock().expect("core state mutex poisoned");
        let capture = state.current_capture.as_ref().expect("capture committed");
        assert!(matches!(
            capture.context,
            Some(ContextCapture::Text { ref text, .. }) if text == "surrounding context"
        ));
        assert_eq!(
            capture
                .provenance
                .as_ref()
                .map(|value| value.app_name.as_str()),
            Some("Example")
        );
    }

    #[test]
    fn core_region_selection_replaces_the_committed_target() {
        let core = Core::new(Arc::new(MockProvider::default()), Arc::new(StubCapturer));
        core.capture();

        let selected = core.select_region().expect("region capture succeeds");

        assert!(selected.is_some());
        let state = core.state.lock().expect("core state mutex poisoned");
        assert!(matches!(
            state
                .current_capture
                .as_ref()
                .and_then(|capture| capture.target.as_ref()),
            Some(Target::Image(_))
        ));
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
        let mut state = State::default();
        let capture_id = state.start_capture();
        state.commit_capture(
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

        let (request, _, _) = state.start_provider_request(Some("explain this".into()));

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
        let mut state = State::default();
        let first_id = state.start_capture();
        state.commit_capture(
            first_id,
            Capture {
                target: Some(Target::Image(image())),
                context: None,
                provenance: None,
                elapsed_ms: 0,
            },
        );

        state.start_capture();

        assert!(state.current_capture.is_none());
    }

    #[test]
    fn stale_region_capture_does_not_replace_the_current_target() {
        let mut state = State::default();
        let stale_id = state.start_capture();
        let current_id = state.start_capture();
        state.commit_capture(
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
        let mut state = State::default();
        let capture_id = state.start_capture();
        state.commit_capture(
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
        let (request, _, _) = state.start_provider_request(None);

        assert!(matches!(request.target, Some(Target::Image(_))));
        assert!(matches!(
            request.context,
            Some(ContextCapture::Text { ref text, .. }) if text == "surrounding context"
        ));
    }

    #[test]
    fn newer_provider_request_aborts_the_previous_request() {
        let mut state = State::default();
        let (_, first_id, first_registration) = state.start_provider_request(None);
        let first_abort = first_registration.handle();

        let (_, second_id, _) = state.start_provider_request(None);

        assert!(first_abort.is_aborted());
        assert_ne!(first_id, second_id);
        assert!(!state.accept_provider_output(first_id, &Output::AnswerChunk("stale".into())));
        assert!(state.accept_provider_output(second_id, &Output::AnswerChunk("current".into())));
    }

    #[test]
    fn beginning_a_capture_aborts_the_active_provider_request() {
        let mut state = State::default();
        let (_, request_id, registration) = state.start_provider_request(None);
        let abort = registration.handle();

        state.start_capture();

        assert!(abort.is_aborted());
        assert!(!state.accept_provider_output(request_id, &Output::AnswerChunk("stale".into())));
    }

    #[test]
    fn terminal_provider_output_closes_the_active_request() {
        let mut state = State::default();
        let (_, request_id, _) = state.start_provider_request(None);

        assert!(state.accept_provider_output(request_id, &Output::AnswerCompleted));
        assert!(!state.accept_provider_output(request_id, &Output::AnswerChunk("late".into())));
    }
}
