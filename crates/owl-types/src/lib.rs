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

// `display` is the platform display id (e.g., a CGDirectDisplayID on macOS).
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

#[derive(Debug, Clone)]
pub struct ImageCapture {
    pub png: Vec<u8>,
    pub region: ScreenRect,
}

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

#[derive(Debug, Clone)]
pub enum CoreMessage {
    Show,
    Target(Target),
    Token(String),
    Done,
    Error(String),
}
