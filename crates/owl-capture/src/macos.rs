use crate::{
    Capture, CaptureMethod, Capturer, ContextCaptureMethod, Error, Permission, Provenance, Result,
    SpanCaptureMethod,
};

use axuielement::prelude::*;

pub struct MacosCapturer {}

impl MacosCapturer {
    pub fn new() -> Self {
        Self {}
    }

    // --- span helpers --------------------------------------------------------

    /// Accessibility fast path (5-30ms): focused element -> `kAXSelectedText`.
    /// Returns `(text, implementation_detail)`, or `None` if absent/empty.
    fn ax_selected_text(&self) -> Option<(String, Option<String>)> {
        // TODO(axuielement): system_wide -> kAXFocusedUIElement -> kAXSelectedText.
        // Return None on empty string so the cascade falls through to clipboard.
        todo!("read kAXSelectedText via axuielement")
    }

    /// Clipboard fallback (~40-120ms): save pasteboard, synthesize Cmd+C, poll
    /// until it changes, read it, then RESTORE the prior contents (mandatory -
    /// this mutates global state). Needs `arboard` + a key-synth path
    /// (core-graphics `CGEvent`), neither a dependency yet -> deferred.
    fn clipboard_span(&self) -> Option<(String, Option<String>)> {
        None
    }

    /// OCR of the focused-window region via the Vision framework. Opt-in only,
    /// and the only path needing Screen Recording permission. If you implement
    /// it and the grant is missing, surface
    /// `Error::PermissionDenied { permission: Permission::ScreenRecording }`.
    fn ocr_span(&self) -> Option<(String, Option<String>)> {
        None
    }

    // --- context helpers -----------------------------------------------------

    /// Accessibility: focused element -> `kAXValue` (its full text = the
    /// surrounding context). Frequently `None` (many apps don't expose it);
    /// that is expected and `capture()` degrades context to `None`.
    fn ax_value_text(&self) -> Option<(String, Option<String>)> {
        // TODO(axuielement): kAXFocusedUIElement -> kAXValue.
        todo!("read kAXValue via axuielement")
    }

    /// OCR of the window region. Deferred alongside `ocr_span`.
    fn ocr_region(&self) -> Option<(String, Option<String>)> {
        None
    }

    // --- provenance helpers --------------------------------------------------

    /// Frontmost application name. Almost always resolves.
    fn focused_app_name(&self) -> Option<String> {
        // TODO: NSWorkspace.frontmostApplication.localizedName (objc2-app-kit),
        // or system_wide -> kAXFocusedApplication -> kAXTitle.
        todo!("frontmost app name")
    }

    /// Focused window title via `kAXTitle`. Almost always resolves.
    fn focused_window_title(&self) -> Option<String> {
        // TODO(axuielement): kAXFocusedApplication -> kAXFocusedWindow -> kAXTitle.
        None
    }

    /// Browser URL: from the AX tree where exposed, AppleScript otherwise.
    fn browser_url(&self) -> Option<String> {
        None
    }

    /// The high-value field: focused window -> `kAXDocument` -> `file://` URL ->
    /// path string. Yields a real path for Preview / Skim / PDFKit apps; `None`
    /// elsewhere (title-parsing fallback lives higher up, in core).
    fn document_path(&self) -> Option<String> {
        // TODO(axuielement): kAXFocusedWindow -> kAXDocument -> parse file URL.
        None
    }
}

/// Whether this process holds macOS Accessibility permission (`AXIsProcessTrusted`).
/// Stubbed `true` so local dev proceeds; wire to the real check before shipping.
fn ax_is_trusted() -> bool {
    // TODO: AXIsProcessTrusted() - via axuielement or `accessibility-sys`.
    true
}

impl Capturer for MacosCapturer {
    fn capture(&self) -> Result<Capture> {
        if !ax_is_trusted() {
            return Err(Error::PermissionDenied {
                permission: Permission::Accessibility,
            });
        }
        let start = std::time::Instant::now();

        // Required. `prefer` lists are hardcoded here; core will build them from
        // config (append `Ocr` only when the user enables it).
        let (span, span_method) = self.capture_span(&[
            SpanCaptureMethod::Accessibility,
            SpanCaptureMethod::Clipboard,
        ])?;

        // Best-effort: on failure, record the attempted method with no text.
        let (context, context_method) =
            match self.capture_context(&[ContextCaptureMethod::Accessibility]) {
                Ok((text, method)) => (Some(text), method),
                Err(_) => (
                    None,
                    CaptureMethod {
                        method: ContextCaptureMethod::Accessibility,
                        implementation: None,
                    },
                ),
            };

        // Best-effort: a missing provenance is not a failed capture.
        let provenance = self.capture_provenance().ok();

        Ok(Capture {
            span: Some(span),
            context,
            provenance,
            elapsed_ms: start.elapsed().as_millis() as u32,
            span_method,
            context_method,
        })
    }

    /// Walk `prefer` in order; return the first path that yields text, tagging
    /// it with the method that won and its implementation detail.
    fn capture_span(
        &self,
        prefer: &[SpanCaptureMethod],
    ) -> Result<(String, CaptureMethod<SpanCaptureMethod>)> {
        for &method in prefer {
            let got = match method {
                SpanCaptureMethod::Accessibility => self.ax_selected_text(),
                SpanCaptureMethod::Clipboard => self.clipboard_span(),
                SpanCaptureMethod::Ocr => self.ocr_span(),
            };
            if let Some((text, implementation)) = got {
                return Ok((text, CaptureMethod { method, implementation }));
            }
        }
        Err(Error::AllMethodsFailed)
    }

    /// Same cascade shape as span, minus the clipboard path (the clipboard
    /// carries only the selection, never surrounding context - which is why
    /// `ContextCaptureMethod` has no `Clipboard` variant).
    fn capture_context(
        &self,
        prefer: &[ContextCaptureMethod],
    ) -> Result<(String, CaptureMethod<ContextCaptureMethod>)> {
        for &method in prefer {
            let got = match method {
                ContextCaptureMethod::Accessibility => self.ax_value_text(),
                ContextCaptureMethod::Ocr => self.ocr_region(),
            };
            if let Some((text, implementation)) = got {
                return Ok((text, CaptureMethod { method, implementation }));
            }
        }
        Err(Error::AllMethodsFailed)
    }

    /// Not a cascade: gather each field best-effort and independently. Only
    /// errors if there is genuinely no frontmost context to describe; empty /
    /// `None` sub-fields are the honest "weak provenance" case.
    fn capture_provenance(&self) -> Result<Provenance> {
        Ok(Provenance {
            app_name: self.focused_app_name().unwrap_or_default(),
            window_title: self.focused_window_title().unwrap_or_default(),
            url: self.browser_url(),
            path: self.document_path(),
        })
    }
}
