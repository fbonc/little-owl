use std::task::Context;

use thiserror::Error;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanCaptureMethod {
    Accessibility,
    Clipboard,
    Ocr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextCaptureMethod {
    Accessibility,
    Ocr
}

pub struct CaptureMethod<T> {
    pub method: T,
    pub implementation: Option<String>
}

pub struct Provenance {
    pub app_name: String,
    pub window_title: String,
    pub url: Option<String>,
    pub path: Option<String>,
}

pub struct Capture {
    pub span: Option<String>,
    pub context: Option<String>,
    pub provenance: Option<Provenance>,
    pub elapsed_ms: u32,
    pub span_method: CaptureMethod<SpanCaptureMethod>,
    pub context_method: CaptureMethod<ContextCaptureMethod>
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    Accessibility,
    ScreenRecording
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("no capture method produced a span")]
    AllMethodsFailed,

    #[error("{permission:?} permission not granted")]
    PermissionDenied { permission: Permission },

    #[error("{0}")]
    Platform(String),
}

pub type Result<T> = std::result::Result<T, Error>;

pub trait Capturer: Send + Sync {
    fn capture(&self) -> Result<Capture>;

    fn capture_span(&self, prefer: &[SpanCaptureMethod]) -> Result<(String, CaptureMethod<SpanCaptureMethod>)>;

    fn capture_context(&self, prefer: &[ContextCaptureMethod]) -> Result<(String, CaptureMethod<ContextCaptureMethod>)>;

    fn capture_provenance(&self) -> Result<Provenance>;
}

pub fn new_capturer() -> Box<dyn Capturer> {
    #[cfg(target_os = "macos")]
    return Box::new(macos::MacosCapturer::new());

    #[cfg(target_os = "windows")]
    return Box::new(windows::WindowsCapturer::new());

    #[cfg(target_os = "linux")]
    return Box::new(linux::LinuxCapturer::new());
}
