use ely_gpui_component::{
    buttons::Button,
    primitives::FocusScope,
    theme::{ActiveTheme, Mode, Theme},
    typography::{Heading, Paragraph},
    Assets,
};
use gpui::{
    div, px, size, App, AppContext, Bounds, Context, FocusHandle, IntoElement, ParentElement,
    Render, Styled, TitlebarOptions, Window, WindowBounds, WindowOptions,
};

struct Spike {
    focus: FocusHandle,
    presses: usize,
}

impl Render for Spike {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let next = if theme.is_dark() {
            Mode::Light
        } else {
            Mode::Dark
        };
        let this = cx.entity();

        FocusScope::new(&self.focus)
            .root()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_4()
            .bg(theme.colors.bg)
            .text_color(theme.colors.fg)
            .child(Heading::h1("Siegu"))
            .child(Paragraph::new("GPUI + Ely desktop spike"))
            .child(Paragraph::new(format!("Pressed {} time(s)", self.presses)))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Button::new("press", "Press me")
                            .primary()
                            .on_click(move |_, _, cx| {
                                this.update(cx, |spike, cx| {
                                    spike.presses += 1;
                                    cx.notify();
                                });
                            }),
                    )
                    .child(
                        Button::new("mode", format!("{next:?} mode"))
                            .on_click(move |_, _, cx| Theme::set_mode(next, cx)),
                    ),
            )
    }
}

fn main() {
    gpui_platform::application()
        .with_assets(Assets)
        .run(|cx: &mut App| {
            ely_gpui_component::init(cx).expect("Ely failed to start");
            Theme::set_mode_now(Mode::Dark, cx);

            let bounds = Bounds::centered(None, size(px(900.0), px(560.0)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Siegu — GPUI + Ely spike".into()),
                    ..Default::default()
                }),
                ..Default::default()
            };

            cx.open_window(options, |window, cx| {
                cx.new(|cx| {
                    let focus = cx.focus_handle();
                    window.focus(&focus, cx);
                    Spike { focus, presses: 0 }
                })
            })
            .expect("window failed to open");
            cx.activate(true);
        });
}
