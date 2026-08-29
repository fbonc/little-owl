use crate::{
    Capturer, ContextCapture, ContextCaptureMethod, Error, OcrCapture, Permission, Provenance,
    Result, ScreenRect, TextCapture, TextCaptureMethod,
};

use axuielement as ax;

pub struct MacosCapturer {}

impl MacosCapturer {
    pub fn new() -> Self {
        Self {}
    }

    fn ax_selected_text(&self) -> Option<String> {
        // TODO(axuielement): system_wide -> kAXFocusedUIElement -> kAXSelectedText
        todo!("read kAXSelectedText via axuielement")
    }

    fn clipboard_text(&self) -> Option<String> {
        None
    }

    fn ax_value_text(&self) -> Option<String> {
        let focused = ax::system_wide()?.focused_ui_element().ok()??;
        focused
            .string_attribute(ax::ax_attribute::AX_VALUE_ATTRIBUTE)
            .ok()?
    }

    fn ocr_window_context(&self) -> Option<String> {
        None
    }

    fn ocr_capture(&self, region: ScreenRect) -> Result<String> {
        // TODO: screenshot region (Err PermissionDenied{ScreenRecording} on denial);
        // AppKit points -> CG pixels (flip Y, * backingScaleFactor, + display origin);
        // Vision VNRecognizeTextRequest -> observations joined in reading order.
        let _ = region;
        todo!("screenshot region + Vision OCR")
    }

    fn focused_app_name(&self) -> Option<String> {
        // TODO: NSWorkspace.frontmostApplication, or kAXFocusedApplication -> kAXTitle
        todo!("frontmost app name")
    }

    fn focused_window_title(&self) -> Option<String> {
        // TODO(axuielement): kAXFocusedApplication -> kAXFocusedWindow -> kAXTitle
        None
    }

    fn browser_url(&self) -> Option<String> {
        None
    }

    fn document_path(&self) -> Option<String> {
        // TODO(axuielement): kAXFocusedWindow -> kAXDocument -> parse file URL
        None
    }
}

impl Capturer for MacosCapturer {
    fn capture_text(&self) -> Result<TextCapture> {
        if !ax::is_process_trusted_with_prompt() {
            return Err(Error::PermissionDenied {
                permission: Permission::Accessibility,
            });
        }
        if let Some(text) = self.ax_selected_text() {
            return Ok(TextCapture {
                text,
                method: TextCaptureMethod::Accessibility,
            });
        }
        if let Some(text) = self.clipboard_text() {
            return Ok(TextCapture {
                text,
                method: TextCaptureMethod::Clipboard,
            });
        }
        Err(Error::AllMethodsFailed)
    }

    fn capture_context(&self) -> Result<ContextCapture> {
        if let Some(text) = self.ax_value_text() {
            return Ok(ContextCapture {
                text,
                method: ContextCaptureMethod::Accessibility,
            });
        }
        if let Some(text) = self.ocr_window_context() {
            return Ok(ContextCapture {
                text,
                method: ContextCaptureMethod::Ocr,
            });
        }
        Err(Error::AllMethodsFailed)
    }

    fn capture_region(&self, region: ScreenRect) -> Result<OcrCapture> {
        let text = self.ocr_capture(region)?;
        Ok(OcrCapture { text, region })
    }

    fn capture_provenance(&self) -> Result<Provenance> {
        Ok(Provenance {
            app_name: self.focused_app_name().unwrap_or_default(),
            window_title: self.focused_window_title().unwrap_or_default(),
            url: self.browser_url(),
            path: self.document_path(),
        })
    }
}
