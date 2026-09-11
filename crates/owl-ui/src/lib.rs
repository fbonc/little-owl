use iced::Element;

pub mod overlay;

pub use overlay::Output as OverlayOutput;
pub use overlay::{Overlay, Phase, Submit};
pub use owl_types::{CoreMessage, Target};

pub fn view(overlay: &Overlay) -> Element<'_, overlay::Message> {
    overlay.view()
}
