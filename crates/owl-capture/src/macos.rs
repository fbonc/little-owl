use std::io::Cursor;
use std::thread;
use std::time::{Duration, Instant};

use arboard::Clipboard;
use axuielement as ax;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use objc2_app_kit::NSWorkspace;
use xcap::Monitor;
use xcap::image::{ImageFormat, RgbaImage};

use crate::{
    Capturer, ContextCapture, ContextCaptureMethod, Error, ImageCapture, Permission, Provenance,
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

    // Screenshot a region as PNG bytes.
    fn capture_image(&self, region: ScreenRect) -> Result<Vec<u8>> {
        screenshot_png(region)
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

    // fallback: cmd+a, copy to grab the whole focused document as context, then restore the clipboard.
    fn clipboard_context(&self) -> Option<String> {
        let mut clipboard = Clipboard::new().ok()?;
        let original = clipboard.get_text().ok();

        let captured = if send_cmd_chord('a') && send_copy_shortcut() {
            poll_for_copied_text(&mut clipboard, original.as_deref())
        } else {
            None
        };

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

        captured.and_then(non_empty)
    }

    // fallback: screenshot the focused window as context.
    fn image_window_context(&self) -> Option<ImageCapture> {
        let window = focused_window()?;
        let pos = window
            .point_attribute(ax::ax_attribute::AX_POSITION_ATTRIBUTE)
            .ok()??;
        let size = window
            .size_attribute(ax::ax_attribute::AX_SIZE_ATTRIBUTE)
            .ok()??;
        let region = window_screen_rect(pos, size)?;
        let png = self.capture_image(region).ok()?;
        Some(ImageCapture { png, region })
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
            return Ok(ContextCapture::Text {
                text,
                method: ContextCaptureMethod::Accessibility,
            });
        }
        if let Some(text) = self.clipboard_context() {
            return Ok(ContextCapture::Text {
                text,
                method: ContextCaptureMethod::Clipboard,
            });
        }
        if let Some(image) = self.image_window_context() {
            return Ok(ContextCapture::Image(image));
        }
        Err(Error::AllMethodsFailed)
    }

    fn capture_region(&self, region: ScreenRect) -> Result<ImageCapture> {
        let png = self.capture_image(region)?;
        Ok(ImageCapture { png, region })
    }

    fn capture_provenance(&self) -> Result<Provenance> {
        Ok(Provenance {
            app_name: self.focused_app_name().unwrap_or_default(),
            window_title: self.focused_window_title().unwrap_or_default(),
            path: self.document_path(),
        })
    }
}

fn non_empty(s: String) -> Option<String> {
    if s.trim().is_empty() { None } else { Some(s) }
}

fn frontmost_app_element() -> Option<ax::AXUIElement> {
    let pid = NSWorkspace::sharedWorkspace()
        .frontmostApplication()?
        .processIdentifier();
    let app = ax::AXUIElement::from_pid(pid)?;
    // Ask Chromium/Electron apps to build their AX tree (off by default).
    let _ = app.set_bool_attribute("AXManualAccessibility", true);
    Some(app)
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

fn screenshot_png(region: ScreenRect) -> Result<Vec<u8>> {
    let monitor = monitor_for_display(region.display)?;
    let (mon_w, mon_h) = (
        monitor.width().map_err(xcap_err)?,
        monitor.height().map_err(xcap_err)?,
    );
    let (x, y, w, h) = clamp_region(region, mon_w, mon_h)
        .ok_or_else(|| Error::Platform("capture region is empty or off-screen".to_string()))?;
    let image = monitor.capture_region(x, y, w, h).map_err(xcap_err)?;
    encode_png(&image)
}

fn monitor_for_display(display: u32) -> Result<Monitor> {
    Monitor::all()
        .map_err(xcap_err)?
        .into_iter()
        .find(|m| m.id().map(|id| id == display).unwrap_or(false))
        .ok_or_else(|| Error::Platform(format!("no monitor with display id {display}")))
}

// Clip a display-relative region to the monitor and snap to whole pixels.
// Returns None if nothing on-screen remains.
fn clamp_region(region: ScreenRect, mon_w: u32, mon_h: u32) -> Option<(u32, u32, u32, u32)> {
    let (mon_w, mon_h) = (mon_w as f64, mon_h as f64);
    let left = region.x.max(0.0);
    let top = region.y.max(0.0);
    let right = (region.x + region.w).min(mon_w);
    let bottom = (region.y + region.h).min(mon_h);
    let w = right - left;
    let h = bottom - top;
    if w < 1.0 || h < 1.0 {
        return None;
    }
    Some((left as u32, top as u32, w as u32, h as u32))
}

// Map an AX window rect (global top-left points) to a display-relative ScreenRect.
fn window_screen_rect(pos: ax::AXPoint, size: ax::AXSize) -> Option<ScreenRect> {
    let center_x = (pos.x + size.width / 2.0) as i32;
    let center_y = (pos.y + size.height / 2.0) as i32;
    let monitor = Monitor::from_point(center_x, center_y).ok()?;
    Some(ScreenRect {
        x: pos.x - monitor.x().ok()? as f64,
        y: pos.y - monitor.y().ok()? as f64,
        w: size.width,
        h: size.height,
        display: monitor.id().ok()?,
    })
}

fn encode_png(image: &RgbaImage) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
        .map_err(|e| Error::Platform(format!("PNG encode failed: {e}")))?;
    Ok(buf)
}

fn xcap_err(e: xcap::XCapError) -> Error {
    Error::Platform(format!("screen capture failed: {e}"))
}

fn send_copy_shortcut() -> bool {
    send_cmd_chord('c')
}

fn send_cmd_chord(c: char) -> bool {
    let Ok(mut enigo) = Enigo::new(&Settings::default()) else {
        return false;
    };
    if enigo.key(Key::Meta, Direction::Press).is_err() {
        return false;
    }
    let pressed = enigo.key(Key::Unicode(c), Direction::Click).is_ok();
    let released = enigo.key(Key::Meta, Direction::Release).is_ok();
    pressed && released
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
    } else {
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

    fn rect(x: f64, y: f64, w: f64, h: f64) -> ScreenRect {
        ScreenRect {
            x,
            y,
            w,
            h,
            display: 0,
        }
    }

    #[test]
    fn clamp_region_within_bounds() {
        assert_eq!(
            clamp_region(rect(10.0, 20.0, 100.0, 50.0), 1920, 1080),
            Some((10, 20, 100, 50))
        );
    }

    #[test]
    fn clamp_region_clips_to_monitor() {
        // Overhangs the right/bottom edges: width/height are trimmed.
        assert_eq!(
            clamp_region(rect(1900.0, 1060.0, 100.0, 100.0), 1920, 1080),
            Some((1900, 1060, 20, 20))
        );
        // Negative origin (off the top-left) is clipped back to zero.
        assert_eq!(
            clamp_region(rect(-30.0, -10.0, 100.0, 60.0), 1920, 1080),
            Some((0, 0, 70, 50))
        );
    }

    #[test]
    fn clamp_region_rejects_empty() {
        // Zero area.
        assert_eq!(clamp_region(rect(0.0, 0.0, 0.0, 100.0), 1920, 1080), None);
        // Fully off-screen to the right.
        assert_eq!(
            clamp_region(rect(3000.0, 0.0, 100.0, 100.0), 1920, 1080),
            None
        );
        // Sub-pixel sliver.
        assert_eq!(clamp_region(rect(0.0, 0.0, 0.4, 0.4), 1920, 1080), None);
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
            "app={:?} window={:?} path={:?}",
            prov.app_name, prov.window_title, prov.path
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
    #[ignore = "requires Screen Recording permission"]
    fn live_capture_region() {
        let display = Monitor::all()
            .expect("monitors")
            .into_iter()
            .find_map(|m| m.is_primary().ok().filter(|&p| p).and(m.id().ok()))
            .expect("a primary monitor");
        let cap = MacosCapturer::new();
        let region = ScreenRect {
            x: 0.0,
            y: 0.0,
            w: 200.0,
            h: 200.0,
            display,
        };
        let img = cap.capture_region(region).expect("capture");
        assert!(!img.png.is_empty());
        assert_eq!(&img.png[..8], b"\x89PNG\r\n\x1a\n");
    }

    #[test]
    #[ignore = "requires Accessibility permission and a focused text element"]
    fn live_capture_context() {
        let cap = MacosCapturer::new();
        match cap.capture_context() {
            Ok(ContextCapture::Text { text, method }) => {
                println!("context {} chars via {:?}", text.len(), method)
            }
            Ok(ContextCapture::Image(img)) => println!("context image {} bytes", img.png.len()),
            Err(e) => println!("no context: {e}"),
        }
    }
}
