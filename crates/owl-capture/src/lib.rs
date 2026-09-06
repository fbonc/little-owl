use thiserror::Error;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextCaptureMethod {
    Accessibility,
    Clipboard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextCaptureMethod {
    Accessibility,
    Clipboard,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub display: u32,
}

pub struct TextCapture {
    pub text: String,
    pub method: TextCaptureMethod,
}

// A raw screen image handed to a multimodal model, rather than OCR'd to text.
pub struct ImageCapture {
    pub png: Vec<u8>,
    pub region: ScreenRect,
}

pub enum Target {
    Text(TextCapture),
    Image(ImageCapture),
}

pub enum ContextCapture {
    Text {
        text: String,
        method: ContextCaptureMethod,
    },
    Image(ImageCapture),
}

pub struct Provenance {
    pub app_name: String,
    pub window_title: String,
    pub path: Option<String>,
}

pub struct Capture {
    pub target: Option<Target>,
    pub context: Option<ContextCapture>,
    pub provenance: Option<Provenance>,
    pub elapsed_ms: u32,
}

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

    fn capture_region(&self, region: ScreenRect) -> Result<ImageCapture>;

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
