use iced::Element;

pub mod overlay;

pub use overlay::{Event, Overlay, Phase, Submit};
pub use owl_types::{Target, UiEvent};

pub fn view(overlay: &Overlay) -> Element<'_, overlay::Message> {
    overlay.view()
}
