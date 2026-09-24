use std::sync::Arc;
use std::thread;

use futures_channel::mpsc;
use futures_util::Stream;

use crate::Output;
use crate::core::Core;

const HOTKEY_ACCELERATOR: &str = "Ctrl+Shift+KeyA";

pub(crate) fn outputs(core: Arc<Core>) -> impl Stream<Item = Output> {
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
