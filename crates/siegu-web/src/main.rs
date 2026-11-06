use ely_gpui_component::{Button, Heading, Paragraph};
use gpui::*;

fn main() {
    // GPUI wasm app: start a headless app and render a minimal UI.
    let cx = App::new().unwrap();
    cx.new(|cx| {
        let root = div()
            .bg(cx.theme().colors.bg)
            .text_color(cx.theme().colors.fg)
            .child(Heading::h1("Siegu web"))
            .child(Paragraph::new("GPUI + Ely web spike"))
            .child(
                Button::new("press", "Press me")
                    .primary()
                    .on_click(|_, _, cx| cx.notify()),
            );
        root
    });
    // The wasm runtime keeps the event loop alive.
}
