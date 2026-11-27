use ely_gpui_component::{navigation::NavItem, primitives::IconName};
use gpui::{div, prelude::FluentBuilder, AnyElement, App, IntoElement, ParentElement, Styled};
use std::rc::Rc;

/// One selectable place in the Siegu sidebar.
#[derive(Clone, Copy)]
pub struct NavItemDef {
    pub id: &'static str,
    pub icon: IconName,
    pub label: &'static str,
}

/// The built-in navigation sections, in display order.
pub const NAV_ITEMS: [NavItemDef; 7] = [
    NavItemDef {
        id: "library",
        icon: IconName::LayoutGrid,
        label: "Library",
    },
    NavItemDef {
        id: "map",
        icon: IconName::MapPin,
        label: "Map",
    },
    NavItemDef {
        id: "people",
        icon: IconName::Users,
        label: "People",
    },
    NavItemDef {
        id: "collections",
        icon: IconName::Layers,
        label: "Collections",
    },
    NavItemDef {
        id: "devices",
        icon: IconName::Monitor,
        label: "Devices",
    },
    NavItemDef {
        id: "duplicates",
        icon: IconName::Copy,
        label: "Space saver",
    },
    NavItemDef {
        id: "settings",
        icon: IconName::Settings,
        label: "Settings",
    },
];

/// Builds the unbranded sidebar navigation list.
///
/// `on_nav` is invoked with the selected item's id whenever a user taps a nav
/// item, `active` marks the current section, and `folded` renders the icon rail
/// with tooltips instead of labels.
pub fn siegu_nav_items(
    on_nav: Rc<dyn Fn(&str, &mut App)>,
    active: Option<&str>,
    folded: bool,
) -> AnyElement {
    let mut items = Vec::with_capacity(NAV_ITEMS.len());
    for item in NAV_ITEMS {
        let handler = Rc::clone(&on_nav);
        let id = item.id;
        items.push(
            NavItem::new(item.id, item.icon, item.label)
                .active(active == Some(item.id))
                .folded(folded)
                .on_click(move |_, cx| handler(id, cx))
                .into_any_element(),
        );
    }
    div()
        .flex()
        .flex_col()
        .gap_0p5()
        .when(folded, |group| group.items_center().px_2())
        .children(items)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nav_items_are_in_order() {
        let ids: Vec<&str> = NAV_ITEMS.iter().map(|i| i.id).collect();
        assert_eq!(
            ids,
            [
                "library",
                "map",
                "people",
                "collections",
                "devices",
                "duplicates",
                "settings"
            ]
        );
    }
}
