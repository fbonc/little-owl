use iced::Element;
use owl_provider::ModelSelection;

pub mod overlay;

pub use overlay::Output as OverlayOutput;
pub use overlay::{Overlay, Phase};
pub use owl_types::Target;

pub fn view(
    overlay: &Overlay,
    available_models: Vec<ModelSelection>,
) -> Element<'_, overlay::Input> {
    overlay.view(available_models)
}
