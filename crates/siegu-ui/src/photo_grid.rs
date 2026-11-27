use ely_gpui_component::layout::SimpleGrid;
use gpui::{px, AnyElement, App, IntoElement, ParentElement, Pixels, RenderOnce, Window};

/// A responsive grid of photos: as many columns as fit, each at least `min`
/// wide, with `gap` between them.
#[derive(IntoElement)]
pub struct PhotoGrid {
    children: Vec<AnyElement>,
    gap: Pixels,
}

impl PhotoGrid {
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
            gap: px(16.0),
        }
    }

    /// Overrides the space between rows and columns for this grid only.
    pub fn gap(mut self, gap: Pixels) -> Self {
        self.gap = gap;
        self
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
        let mut grid = SimpleGrid::new("photo-grid", px(220.0), self.gap);
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

    #[test]
    fn gap_is_local_to_each_grid() {
        let default = PhotoGrid::new();
        let tight = PhotoGrid::new().gap(px(8.0));
        assert_eq!(default.gap, px(16.0));
        assert_eq!(tight.gap, px(8.0));
    }
}
