use ely_gpui_component::layout::SimpleGrid;
use gpui::{px, AnyElement, App, IntoElement, ParentElement, RenderOnce, Window};

/// A responsive grid of photos: as many columns as fit, each at least `min`
/// wide, with `gap` between them.
#[derive(IntoElement)]
pub struct PhotoGrid {
    children: Vec<AnyElement>,
}

impl PhotoGrid {
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
        }
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }
}

impl Default for PhotoGrid {
    fn default() -> Self {
        Self::new()
    }
}

impl ParentElement for PhotoGrid {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for PhotoGrid {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut grid = SimpleGrid::new("photo-grid", px(220.0), px(16.0));
        grid.extend(self.children.into_iter());
        grid
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::div;

    #[test]
    fn grid_collects_children() {
        let mut grid = PhotoGrid::new();
        grid = grid.child(div());
        grid = grid.child(div());
        assert_eq!(grid.children.len(), 2);
    }
}
