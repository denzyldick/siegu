use std::rc::Rc;

use crate::i18n;
use ely_gpui_component::{
    buttons::Button,
    media::ImageViewer,
    motion::ProgressBar,
    overlays::Dialog,
    primitives::{Icon, IconName, Image},
    theme::{ActiveTheme, IconSize, TextSize},
    typography::{Caption, Heading, Overline, Paragraph},
};
use gpui::{
    div, px, AnyElement, App, ElementId, ImageSource, InteractiveElement, IntoElement,
    KeyDownEvent, ObjectFit, ParentElement, RenderOnce, Resource, SharedString, Styled, Window,
};

/// One labeled camera/EXIF value in the viewer's metadata panel.
#[derive(Clone, Debug)]
pub struct ViewerField {
    pub label: SharedString,
    pub value: String,
}

/// A detected person in the viewed photo.
#[derive(Clone)]
pub struct ViewerPerson {
    pub name: String,
    pub thumbnail: Option<ImageSource>,
}

/// An AI object label and its confidence, as a whole percentage.
#[derive(Clone, Debug)]
pub struct ViewerInsight {
    pub name: String,
    pub percent: u32,
}

/// Everything the metadata side panel needs for one photo. The desktop shell
/// fills this from the photo row plus the small set of linked SQLite tables, so
/// the shared UI crate stays independent of the database schema.
#[derive(Clone, Default)]
pub struct ViewerMetadata {
    pub file_path: String,
    pub caption: Option<String>,
    pub aesthetics_score: Option<f64>,
    pub recognized_text: String,
    pub transcript: String,
    pub is_video: bool,
    pub camera: Vec<ViewerField>,
    pub people: Vec<ViewerPerson>,
    pub insights: Vec<ViewerInsight>,
}

/// A fullscreen modal viewer for a single library photo, with zoom, pan, and
/// rotation via `ely::media::ImageViewer`, or a thumbnail fallback when the
/// full-size file is not present locally.
#[derive(IntoElement)]
pub struct MediaViewer {
    id: ElementId,
    title: SharedString,
    detail: Option<SharedString>,
    source: Option<Resource>,
    fallback: Option<ImageSource>,
    favorite: bool,
    metadata: Option<ViewerMetadata>,
    on_toggle_favorite: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    on_add_to_album: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    on_prev: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    on_next: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    on_show_in_explorer: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    on_close: Rc<dyn Fn(&mut Window, &mut App)>,
}

impl MediaViewer {
    /// Creates a viewer dialog keyed by `id` for the photo titled `title`.
    pub fn new(
        id: impl Into<ElementId>,
        title: impl Into<SharedString>,
        on_close: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            detail: None,
            source: None,
            fallback: None,
            favorite: false,
            metadata: None,
            on_toggle_favorite: None,
            on_add_to_album: None,
            on_prev: None,
            on_next: None,
            on_show_in_explorer: None,
            on_close: Rc::new(on_close),
        }
    }

    /// Supporting subtitle shown under the title (e.g. capture date).
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Full-resolution image resource (typically a local `PathBuf`).
    pub fn source(mut self, source: impl Into<Resource>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Cached thumbnail shown when the full-resolution file is unavailable locally.
    pub fn fallback(mut self, fallback: impl Into<ImageSource>) -> Self {
        self.fallback = Some(fallback.into());
        self
    }

    /// Whether the photo is currently marked as a favorite.
    pub fn favorite(mut self, favorite: bool) -> Self {
        self.favorite = favorite;
        self
    }

    /// Enables the Info action and the metadata side panel for this photo.
    pub fn metadata(mut self, metadata: ViewerMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Invoked when the user clicks the Favorite toggle action.
    pub fn on_toggle_favorite(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_toggle_favorite = Some(Rc::new(handler));
        self
    }

    /// Enables the "Add to Collection" action for this photo.
    pub fn on_add_to_album(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_add_to_album = Some(Rc::new(handler));
        self
    }

    /// Invoked when the user clicks Previous.
    pub fn on_prev(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_prev = Some(Rc::new(handler));
        self
    }

    /// Invoked when the user clicks Next.
    pub fn on_next(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_next = Some(Rc::new(handler));
        self
    }

    /// Invoked when the user asks to reveal the original file in the OS file
    /// manager. Omit it when the file is not available locally.
    pub fn on_show_in_explorer(
        mut self,
        handler: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_show_in_explorer = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for MediaViewer {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let metadata_open =
            window.use_keyed_state(("media-viewer-metadata", 0usize), cx, |_, _| false);
        let theme = cx.theme();
        let on_close = self.on_close;
        let on_prev = self.on_prev.clone();
        let on_next = self.on_next.clone();
        let metadata = self.metadata.clone();
        let show_metadata = metadata.is_some() && *metadata_open.read(cx);
        let mut dialog = Dialog::new(self.id.clone(), self.title, move |window, cx| {
            on_close(window, cx);
        })
        .fullscreen();

        if let Some(detail) = self.detail {
            dialog = dialog.detail(detail);
        }

        if let Some(on_toggle_favorite) = self.on_toggle_favorite {
            let fav = self.favorite;
            dialog = dialog.action(move |_| {
                let label = if fav { "Favorited" } else { "Favorite" };
                let btn = Button::new("mv-favorite", label)
                    .icon(IconName::Heart)
                    .on_click(move |_, window, cx| on_toggle_favorite(window, cx));
                if fav {
                    btn.primary()
                } else {
                    btn
                }
            });
        }

        if let Some(on_add_to_album) = self.on_add_to_album.clone() {
            let label = i18n::text(cx, "media_viewer.add_to_album", &[]);
            dialog = dialog.action(move |_| {
                let handler = on_add_to_album.clone();
                Button::new("mv-add-to-album", label)
                    .icon(IconName::FolderPlus)
                    .on_click(move |_, window, cx| handler(window, cx))
            });
        }

        if let Some(on_show_in_explorer) = self.on_show_in_explorer.clone() {
            let label = i18n::text(cx, "media_viewer.show_in_explorer", &[]);
            dialog = dialog.action(move |_| {
                let handler = on_show_in_explorer.clone();
                Button::new("mv-show-in-explorer", label)
                    .icon(IconName::FolderOpen)
                    .on_click(move |_, window, cx| handler(window, cx))
            });
        }

        if metadata.is_some() {
            let label = i18n::text(cx, "media_viewer.metadata", &[]);
            let toggle = metadata_open.clone();
            dialog = dialog.action(move |_| {
                let state = toggle.clone();
                Button::new("mv-metadata-toggle", label)
                    .icon(IconName::Info)
                    .on_click(move |_, _, cx| {
                        state.update(cx, |open, cx| {
                            *open = !*open;
                            cx.notify();
                        });
                    })
            });
        }

        if let Some(on_prev) = on_prev.clone() {
            dialog = dialog.action(move |_| {
                Button::new("mv-prev", "Previous")
                    .icon(IconName::ArrowLeft)
                    .on_click(move |_, window, cx| on_prev(window, cx))
            });
        }

        if let Some(on_next) = on_next.clone() {
            dialog = dialog.action(move |_| {
                Button::new("mv-next", "Next")
                    .icon(IconName::ArrowRight)
                    .on_click(move |_, window, cx| on_next(window, cx))
            });
        }

        let stage = match (self.source, self.fallback) {
            (Some(resource), _) => div()
                .w_full()
                .h_full()
                .min_h(px(420.0))
                .child(ImageViewer::new((self.id, "viewer"), resource))
                .into_any_element(),
            (None, Some(fallback)) => div()
                .w_full()
                .h_full()
                .min_h(px(420.0))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_3()
                .child(
                    Image::new((self.id, "fallback"), fallback)
                        .fit(ObjectFit::Contain)
                        .w_full()
                        .h(px(360.0)),
                )
                .child(
                    Paragraph::new("Showing cached preview — original file is not stored locally.")
                        .text_size(theme.text_size(TextSize::Sm))
                        .text_color(theme.colors.fg_muted),
                )
                .into_any_element(),
            (None, None) => div()
                .w_full()
                .min_h(px(360.0))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_2()
                .child(
                    Icon::new(IconName::ImageOff)
                        .size(IconSize::Lg)
                        .color(theme.colors.fg_subtle),
                )
                .child(
                    Paragraph::new("Original file is not available on this device.")
                        .text_size(theme.text_size(TextSize::Sm))
                        .text_color(theme.colors.fg_muted),
                )
                .into_any_element(),
        };

        let body = if show_metadata {
            div()
                .flex()
                .flex_row()
                .gap_5()
                .child(div().flex_1().min_w_0().child(stage))
                .child(metadata_panel(metadata.as_ref().unwrap(), cx))
                .into_any_element()
        } else {
            stage
        };

        let dialog = dialog.child(body);
        let (keys_prev, keys_next) = (on_prev, on_next);
        div()
            .size_full()
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                let callback = match event.keystroke.key.as_str() {
                    "left" => keys_prev.as_ref(),
                    "right" => keys_next.as_ref(),
                    _ => return,
                };
                if let Some(callback) = callback {
                    cx.stop_propagation();
                    callback(window, cx);
                }
            })
            .child(dialog)
    }
}

/// Turn a model score into a rounded whole percentage. Scores are stored as
/// 0..=1 fractions, but older rows can exceed that range.
pub fn insight_percent(score: f64) -> u32 {
    (score.clamp(0.0, 1.0) * 100.0).round() as u32
}

fn eyebrow(cx: &App, key: &str) -> Overline {
    Overline::new(i18n::text(cx, key, &[]))
}

fn value_line(text: impl Into<SharedString>, cx: &App) -> AnyElement {
    Paragraph::new(text)
        .text_size(cx.theme().text_size(TextSize::Sm))
        .text_color(cx.theme().colors.fg_muted)
        .into_any_element()
}

fn metadata_section(cx: &App, title_key: &str, body: AnyElement) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(eyebrow(cx, title_key))
        .child(body)
        .into_any_element()
}

fn metadata_panel(metadata: &ViewerMetadata, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let colors = &theme.colors;
    let mut panel = div()
        .w(px(340.0))
        .flex_none()
        .flex()
        .flex_col()
        .gap_5()
        .p_5()
        .border_l_1()
        .border_color(colors.border)
        .bg(colors.surface)
        .child(Heading::h4(i18n::text(cx, "media_viewer.metadata", &[])));

    if !metadata.file_path.is_empty() {
        let path = metadata.file_path.clone();
        panel = panel.child(metadata_section(
            cx,
            "media_viewer.file_details",
            div()
                .flex()
                .items_start()
                .gap_2()
                .child(
                    Icon::new(IconName::FolderOpen)
                        .size(IconSize::Sm)
                        .color(colors.fg_subtle),
                )
                .child(value_line(path, cx))
                .into_any_element(),
        ));
    }

    if let Some(caption) = &metadata.caption {
        let caption = format!("“{caption}”");
        panel = panel.child(metadata_section(
            cx,
            "media_viewer.ai_caption",
            value_line(caption, cx),
        ));
    }

    if let Some(score) = metadata.aesthetics_score.filter(|score| score.is_finite()) {
        panel = panel.child(metadata_section(
            cx,
            "media_card.aesthetics_score",
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    Icon::new(IconName::Sparkles)
                        .size(IconSize::Sm)
                        .color(colors.fg_subtle),
                )
                .child(value_line(format!("{score:.1}"), cx))
                .into_any_element(),
        ));
    }

    if !metadata.recognized_text.is_empty() {
        panel = panel.child(metadata_section(
            cx,
            "media_viewer.recognized_text",
            value_line(metadata.recognized_text.clone(), cx),
        ));
    }

    if !metadata.transcript.is_empty() {
        panel = panel.child(metadata_section(
            cx,
            "media_viewer.transcript",
            value_line(metadata.transcript.clone(), cx),
        ));
    } else if metadata.is_video {
        let empty = i18n::text(cx, "media_viewer.transcript_empty", &[]);
        panel = panel.child(metadata_section(
            cx,
            "media_viewer.transcript",
            value_line(empty, cx),
        ));
    }

    if !metadata.camera.is_empty() {
        let fields = metadata
            .camera
            .iter()
            .map(|field| {
                div()
                    .w(px(140.0))
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .child(Caption::new(field.label.clone()))
                    .child(value_line(field.value.clone(), cx))
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        panel = panel.child(metadata_section(
            cx,
            "media_viewer.camera_settings",
            div()
                .flex()
                .flex_wrap()
                .gap_x_4()
                .gap_y_2()
                .children(fields)
                .into_any_element(),
        ));
    }

    let people_body = if metadata.people.is_empty() {
        value_line(i18n::text(cx, "media_viewer.no_faces", &[]), cx)
    } else {
        let cards = metadata
            .people
            .iter()
            .enumerate()
            .map(|(idx, person)| {
                let avatar = if let Some(thumbnail) = &person.thumbnail {
                    div()
                        .w(px(56.0))
                        .h(px(56.0))
                        .rounded_full()
                        .overflow_hidden()
                        .child(
                            Image::new(("mv-person", idx), thumbnail.clone())
                                .fit(ObjectFit::Cover)
                                .w_full()
                                .h_full(),
                        )
                } else {
                    div()
                        .w(px(56.0))
                        .h(px(56.0))
                        .rounded_full()
                        .bg(colors.border)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            Icon::new(IconName::Users)
                                .size(IconSize::Lg)
                                .color(colors.fg_subtle),
                        )
                };
                div()
                    .w(px(72.0))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_1()
                    .child(avatar)
                    .child(
                        Caption::new(person.name.clone())
                            .text_color(colors.fg)
                            .text_align(gpui::TextAlign::Center),
                    )
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        div()
            .flex()
            .flex_wrap()
            .gap_3()
            .children(cards)
            .into_any_element()
    };
    panel = panel.child(metadata_section(
        cx,
        "media_viewer.people_in_photo",
        people_body,
    ));

    let insights_body = if metadata.insights.is_empty() {
        value_line(i18n::text(cx, "media_viewer.no_insights", &[]), cx)
    } else {
        let rows = metadata
            .insights
            .iter()
            .enumerate()
            .map(|(idx, insight)| {
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .child(value_line(insight.name.clone(), cx))
                            .child(Caption::new(format!("{}%", insight.percent))),
                    )
                    .child(ProgressBar::new(
                        ("mv-insight", idx),
                        insight.percent as f32 / 100.0,
                    ))
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        div()
            .flex()
            .flex_col()
            .gap_2()
            .children(rows)
            .into_any_element()
    };
    panel = panel.child(metadata_section(
        cx,
        "media_viewer.ai_insights",
        insights_body,
    ));

    panel.into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn media_viewer_collects_fields() {
        let metadata = ViewerMetadata {
            file_path: "/tmp/lake.jpg".to_string(),
            caption: Some("A quiet lake".to_string()),
            insights: vec![ViewerInsight {
                name: "lake".to_string(),
                percent: insight_percent(0.87),
            }],
            ..Default::default()
        };
        let viewer = MediaViewer::new("mv", "Lake", |_, _| {})
            .detail("2026-08-01")
            .source(PathBuf::from("/tmp/lake.jpg"))
            .on_prev(|_, _| {})
            .on_next(|_, _| {})
            .on_show_in_explorer(|_, _| {})
            .metadata(metadata.clone());
        assert_eq!(viewer.title.as_ref(), "Lake");
        assert_eq!(viewer.detail.as_deref(), Some("2026-08-01"));
        assert!(viewer.source.is_some());
        assert!(viewer.on_prev.is_some());
        assert!(viewer.on_next.is_some());
        assert!(viewer.on_show_in_explorer.is_some());
        assert_eq!(viewer.metadata.as_ref().unwrap().file_path, "/tmp/lake.jpg");
        assert_eq!(viewer.metadata.unwrap().insights[0].percent, 87);
    }

    #[test]
    fn model_scores_clamp_to_a_whole_percentage() {
        assert_eq!(insight_percent(0.875), 88);
        assert_eq!(insight_percent(1.4), 100);
        assert_eq!(insight_percent(-0.2), 0);
    }
}
