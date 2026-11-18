#![cfg_attr(target_family = "wasm", no_main)]

use ely_gpui_component::{
    buttons::Button,
    theme::{Mode, Theme},
    typography::{Heading, Paragraph},
};
use gpui::{
    div, px, size, App, AppContext, Bounds, Context, IntoElement, ParentElement, Render, Styled,
    Window, WindowBounds, WindowOptions,
};

/// The web spike: a minimal page proving GPUI + Ely render in the browser.
struct SieguWeb;

impl Render for SieguWeb {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_3()
            .p_8()
            .size_full()
            .child(Heading::h1("Siegu web"))
            .child(Paragraph::new("GPUI + Ely web spike"))
            .child(Button::new("press", "Press me").primary())
    }
}

fn run() {
    gpui_platform::application()
        .with_assets(ely_gpui_component::Assets)
        .run(|cx: &mut App| {
            ely_gpui_component::init(cx).expect("Ely failed to start");
            Theme::set_mode_now(Mode::Dark, cx);

            let bounds = Bounds::centered(None, size(px(900.0), px(560.0)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            };

            cx.open_window(options, |_, cx| cx.new(|_| SieguWeb))
                .expect("failed to open window");
            cx.activate(true);
        });
}

#[cfg(not(target_family = "wasm"))]
fn main() {
    run();
}

#[cfg(target_family = "wasm")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    gpui_platform::web_init();
    run();
}
