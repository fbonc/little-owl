use std::thread;
use std::time::{Duration, Instant};

use arboard::Clipboard;
use axuielement as ax;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use objc2_app_kit::NSWorkspace;

use crate::{
    Capturer, ContextCapture, ContextCaptureMethod, Error, OcrCapture, Permission, Provenance,
    Result, ScreenRect, TextCapture, TextCaptureMethod,
};

const CLIPBOARD_TIMEOUT: Duration = Duration::from_millis(400);
const CLIPBOARD_POLL_INTERVAL: Duration = Duration::from_millis(15);

pub struct MacosCapturer {}

impl MacosCapturer {
    pub fn new() -> Self {
        Self {}
    }
}

// Target capture
impl MacosCapturer {
    fn ax_selected_text(&self) -> Option<String> {
        let text = focused_element()?
            .string_attribute(ax::ax_attribute::AX_SELECTED_TEXT_ATTRIBUTE)
            .ok()??;
        non_empty(text)
    }

    // text fallback
    fn clipboard_text(&self) -> Option<String> {
        let mut clipboard = Clipboard::new().ok()?;
        let original = clipboard.get_text().ok();

        if !send_copy_shortcut() {
            return None;
        }

        let captured = poll_for_copied_text(&mut clipboard, original.as_deref());

        if captured.is_some() {
            match &original {
                Some(text) => {
                    let _ = clipboard.set_text(text.clone());
                }
                None => {
                    let _ = clipboard.clear();
                }
            }
        }

        captured
    }

    fn ocr_capture(&self, region: ScreenRect) -> Result<String> {
        // Not yet implemented: needs a ScreenCaptureKit/CGDisplay screenshot of
        // the region, AppKit-points -> CG-pixels conversion (flip Y, scale by
        // backingScaleFactor, add display origin), then Vision text recognition.
        let _ = region;
        Err(Error::Platform("OCR is not implemented".to_string()))
    }
}

// Context capture
impl MacosCapturer {
    fn ax_value_text(&self) -> Option<String> {
        let text = focused_element()?
            .string_attribute(ax::ax_attribute::AX_VALUE_ATTRIBUTE)
            .ok()??;
        non_empty(text)
    }

    // fallback
    fn ocr_window_context(&self) -> Option<String> {
        // Window-region OCR context is not implemented (see ocr_capture).
        None
    }
}

// Provenance capture
impl MacosCapturer {
    fn focused_app_name(&self) -> Option<String> {
        Some(
            NSWorkspace::sharedWorkspace()
                .frontmostApplication()?
                .localizedName()?
                .to_string(),
        )
    }

    fn focused_window_title(&self) -> Option<String> {
        focused_window()?
            .string_attribute(ax::ax_attribute::AX_TITLE_ATTRIBUTE)
            .ok()?
    }

    fn browser_url(&self) -> Option<String> {
        // Browsers do not expose the address-bar URL through a standard AX attribute.
        // Recovering it reliably requires per-browser AppleScript automation,
        // which is a separate permission and out of scope here.
        None
    }

    fn document_path(&self) -> Option<String> {
        let doc = focused_window()?
            .string_attribute(ax::ax_attribute::AX_DOCUMENT_ATTRIBUTE)
            .ok()??;
        doc_url_to_path(&doc)
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

fn non_empty(s: String) -> Option<String> {
    if s.trim().is_empty() { None } else { Some(s) }
}

// Reads go through the frontmost application element, not the system-wide
// element: the system-wide kAXFocusedUIElement/kAXFocusedApplication reads
// return kAXErrorCannotComplete on some setups.
fn frontmost_app_element() -> Option<ax::AXUIElement> {
    let pid = NSWorkspace::sharedWorkspace()
        .frontmostApplication()?
        .processIdentifier();
    ax::AXUIElement::from_pid(pid)
}

fn focused_element() -> Option<ax::AXUIElement> {
    frontmost_app_element()?
        .element_attribute(ax::ax_attribute::AX_FOCUSED_UI_ELEMENT_ATTRIBUTE)
        .ok()?
}

fn focused_window() -> Option<ax::AXUIElement> {
    frontmost_app_element()?
        .element_attribute(ax::ax_attribute::AX_FOCUSED_WINDOW_ATTRIBUTE)
        .ok()?
}

fn send_copy_shortcut() -> bool {
    let Ok(mut enigo) = Enigo::new(&Settings::default()) else {
        return false;
    };
    if enigo.key(Key::Meta, Direction::Press).is_err() {
        return false;
    }
    let pressed_c = enigo.key(Key::Unicode('c'), Direction::Click).is_ok();
    let released = enigo.key(Key::Meta, Direction::Release).is_ok();
    pressed_c && released
}

fn poll_for_copied_text(clipboard: &mut Clipboard, original: Option<&str>) -> Option<String> {
    let deadline = Instant::now() + CLIPBOARD_TIMEOUT;
    loop {
        if let Ok(current) = clipboard.get_text()
            && let Some(text) = clipboard_changed(original, &current)
        {
            return Some(text);
        }
        if Instant::now() >= deadline {
            return None;
        }
        thread::sleep(CLIPBOARD_POLL_INTERVAL);
    }
}

fn clipboard_changed(original: Option<&str>, current: &str) -> Option<String> {
    if current.trim().is_empty() {
        return None;
    }
    if Some(current) == original {
        return None;
    }
    Some(current.to_string())
}

// Convert a `kAXDocument` value into a filesystem path.
// normally a `file://` URL but some apps hand back a bare POSIX path.
fn doc_url_to_path(doc: &str) -> Option<String> {
    let trimmed = doc.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(rest) = trimmed.strip_prefix("file://") {

        let path = rest.strip_prefix("localhost").unwrap_or(rest);
        if !path.starts_with('/') {
            return None;
        }
        Some(percent_decode(path))
    } else if trimmed.starts_with('/') {
        Some(trimmed.to_string())
    } else{
        None
    }
}

/// Decode `%XX` escapes in a URL path back into raw bytes, interpreting the
/// result as UTF-8. Invalid escapes are left verbatim.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_digit(16);
            let lo = (bytes[i + 2] as char).to_digit(16);
            if let (Some(hi), Some(lo)) = (hi, lo) {
                out.push((hi * 16 + lo) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_empty_rejects_blank() {
        assert_eq!(non_empty("hello".to_string()), Some("hello".to_string()));
        assert_eq!(non_empty("  x ".to_string()), Some("  x ".to_string()));
        assert_eq!(non_empty(String::new()), None);
        assert_eq!(non_empty("   \n\t".to_string()), None);
    }

    #[test]
    fn clipboard_change_detection() {
        // Unchanged: same value that was already on the clipboard.
        assert_eq!(clipboard_changed(Some("old"), "old"), None);
        // Changed: a new value replaced the old one.
        assert_eq!(
            clipboard_changed(Some("old"), "new"),
            Some("new".to_string())
        );
        // Changed from an empty/non-text clipboard.
        assert_eq!(clipboard_changed(None, "new"), Some("new".to_string()));
        // Whitespace-only copies are treated as no capture.
        assert_eq!(clipboard_changed(Some("old"), "   "), None);
        assert_eq!(clipboard_changed(None, ""), None);
    }

    #[test]
    fn doc_url_plain_and_encoded() {
        assert_eq!(
            doc_url_to_path("file:///Users/a/notes.txt"),
            Some("/Users/a/notes.txt".to_string())
        );
        assert_eq!(
            doc_url_to_path("file:///Users/a/my%20file.pdf"),
            Some("/Users/a/my file.pdf".to_string())
        );
        // Optional localhost host component.
        assert_eq!(
            doc_url_to_path("file://localhost/Users/a/b.txt"),
            Some("/Users/a/b.txt".to_string())
        );
        // Non-ASCII via UTF-8 percent bytes: %C3%A9 -> é.
        assert_eq!(
            doc_url_to_path("file:///Users/caf%C3%A9.txt"),
            Some("/Users/café.txt".to_string())
        );
    }

    #[test]
    fn doc_url_bare_path_and_rejects() {
        // Some apps return a bare POSIX path.
        assert_eq!(
            doc_url_to_path("/Users/a/b.txt"),
            Some("/Users/a/b.txt".to_string())
        );
        // Not a file: URL and not a path.
        assert_eq!(doc_url_to_path("https://example.com"), None);
        assert_eq!(doc_url_to_path(""), None);
        assert_eq!(doc_url_to_path("   "), None);
    }

    #[test]
    fn percent_decode_edge_cases() {
        assert_eq!(percent_decode("abc"), "abc");
        assert_eq!(percent_decode("a%20b"), "a b");
        // Malformed escape (not two hex digits) is left verbatim.
        assert_eq!(percent_decode("a%2"), "a%2");
        assert_eq!(percent_decode("a%zz"), "a%zz");
        // Trailing percent with no digits.
        assert_eq!(percent_decode("a%"), "a%");
    }

    // Integration tests.
    // Need the Accessibility permissions granted to the test runner.
    //
    // Assert only that the calls return without panicking,
    // since the actual text depends on what is focused when they run.

    #[test]
    #[ignore = "requires Accessibility permission and a focused app"]
    fn live_capture_provenance() {
        let cap = MacosCapturer::new();
        let prov = cap.capture_provenance().expect("provenance is best-effort");
        println!(
            "app={:?} window={:?} url={:?} path={:?}",
            prov.app_name, prov.window_title, prov.url, prov.path
        );
    }

    #[test]
    #[ignore = "requires Accessibility permission; select text before running"]
    fn live_capture_text() {
        let cap = MacosCapturer::new();
        match cap.capture_text() {
            Ok(t) => println!("captured {:?} via {:?}", t.text, t.method),
            Err(e) => println!("no text captured: {e}"),
        }
    }

    #[test]
    #[ignore = "requires Accessibility permission and a focused text element"]
    fn live_capture_context() {
        let cap = MacosCapturer::new();
        match cap.capture_context() {
            Ok(c) => println!("context {} chars via {:?}", c.text.len(), c.method),
            Err(e) => println!("no context: {e}"),
        }
    }
}
