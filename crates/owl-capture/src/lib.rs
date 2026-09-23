use thiserror::Error;

// Shared data vocabulary lives in owl-types (a dependency-light leaf crate) so
// owl-core and owl-ui can name these without depending on owl-capture. Re-exported
// here so `owl_capture::Target` etc. and the trait signatures below keep working.
pub use owl_types::{
    Capture, ContextCapture, ContextCaptureMethod, ImageCapture, Provenance, ScreenRect, Target,
    TextCapture, TextCaptureMethod,
};

#[cfg(target_os = "macos")]
mod macos;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    Accessibility,
    ScreenRecording,
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("no capture method succeeded")]
    AllMethodsFailed,

    #[error("{permission:?} permission not granted")]
    PermissionDenied { permission: Permission },

    #[error("{0}")]
    Platform(String),
}

pub type Result<T> = std::result::Result<T, Error>;

pub trait Capturer: Send + Sync {
    fn capture_text(&self) -> Result<TextCapture>;

    fn capture_context(&self) -> Result<ContextCapture>;

    fn select_region(&self) -> Result<Option<ImageCapture>>;

    fn capture_provenance(&self) -> Result<Provenance>;
}

pub fn new_capturer() -> Box<dyn Capturer> {
    #[cfg(target_os = "macos")]
    return Box::new(macos::MacosCapturer::new());
}
