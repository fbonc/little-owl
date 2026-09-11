use iced::Element;

pub mod overlay;

pub use overlay::Output as OverlayOutput;
pub use overlay::{Overlay, Phase, Submit};
pub use owl_types::{Target, UiEvent};

pub fn view(overlay: &Overlay) -> Element<'_, overlay::Message> {
    overlay.view()
}
