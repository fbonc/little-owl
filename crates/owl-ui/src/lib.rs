use iced::Element;

pub mod overlay;

pub use overlay::Output as OverlayOutput;
pub use overlay::{Overlay, Phase};
pub use owl_types::Target;

pub fn view(overlay: &Overlay) -> Element<'_, overlay::Input> {
    overlay.view()
}
