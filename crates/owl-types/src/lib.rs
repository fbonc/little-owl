//! Shared data vocabulary for little-owl: the capture result types passed between
//! owl-capture (producer), owl-core (orchestrator), and owl-ui (renderer). A leaf
//! crate with no dependencies, so any crate can name these without coupling to
//! another crate or its platform stack.

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

// `display` is the platform display id (a CGDirectDisplayID on macOS).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub display: u32,
}

#[derive(Debug, Clone)]
pub struct TextCapture {
    pub text: String,
    pub method: TextCaptureMethod,
}

// A raw screen image handed to a multimodal model, rather than OCR'd to text.
#[derive(Debug, Clone)]
pub struct ImageCapture {
    pub png: Vec<u8>,
    pub region: ScreenRect,
}

/// The captured target being looked up: the highlighted text, or an image region
/// when text capture failed and we fell back to a screenshot.
#[derive(Debug, Clone)]
pub enum Target {
    Text(TextCapture),
    Image(ImageCapture),
}

#[derive(Debug, Clone)]
pub enum ContextCapture {
    Text {
        text: String,
        method: ContextCaptureMethod,
    },
    Image(ImageCapture),
}

#[derive(Debug, Clone)]
pub struct Provenance {
    pub app_name: String,
    pub window_title: String,
    pub path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Capture {
    pub target: Option<Target>,
    pub context: Option<ContextCapture>,
    pub provenance: Option<Provenance>,
    pub elapsed_ms: u32,
}
