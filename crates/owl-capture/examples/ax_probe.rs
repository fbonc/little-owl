// Manual capture probe. Run with:
//   cargo run -p owl-capture --example ax_probe
// After "focus target", tab to an app, select some text, and wait.

use std::thread::sleep;
use std::time::Duration;

use owl_capture::{ContextCapture, new_capturer};

fn main() {
    eprintln!("\n--- focus target app + make a selection (4s) ---");
    sleep(Duration::from_secs(4));

    let cap = new_capturer();

    match cap.capture_text() {
        Ok(t) => eprintln!("text     : {:?} via {:?}", t.text, t.method),
        Err(e) => eprintln!("text     : {e}"),
    }
    match cap.capture_context() {
        Ok(ContextCapture::Text { text, method }) => {
            eprintln!("context  : {} chars via {:?}", text.len(), method)
        }
        Ok(ContextCapture::Image(img)) => eprintln!("context  : image {} bytes", img.png.len()),
        Err(e) => eprintln!("context  : {e}"),
    }
    match cap.capture_provenance() {
        Ok(p) => eprintln!(
            "provenance: app={:?} window={:?} url={:?} path={:?}",
            p.app_name, p.window_title, p.url, p.path
        ),
        Err(e) => eprintln!("provenance: {e}"),
    }

    eprintln!("--- END ---\n");
}
