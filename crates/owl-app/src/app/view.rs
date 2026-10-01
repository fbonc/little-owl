use iced::widget::mouse_area;
use iced::{Element, Subscription, window as iced_window};
use owl_ui::overlay;

use super::{App, Input};

pub(super) fn view(state: &App) -> Element<'_, Input> {
    let available_models = state.providers.available_models();

    mouse_area(owl_ui::view(&state.overlay, available_models).map(Input::Overlay))
        .on_press(Input::DragWindow)
        .into()
}

pub(super) fn subscription(state: &App) -> Subscription<Input> {
    Subscription::batch([
        iced::event::listen_with(|event, _status, _window| match event {
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(_))
            | iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { .. }) => {
                Some(Input::CheckPromptInputFocus)
            }
            iced::Event::Window(iced_window::Event::CloseRequested) => {
                Some(Input::Overlay(overlay::Input::DismissRequested))
            }
            _ => None,
        }),
        state.overlay.subscription().map(Input::Overlay),
    ])
}
