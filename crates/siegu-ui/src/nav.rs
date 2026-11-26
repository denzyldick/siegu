use ely_gpui_component::{
    navigation::{NavGroup, NavItem},
    primitives::IconName,
};
use gpui::{App, ParentElement};
use std::rc::Rc;

/// One selectable place in the Siegu sidebar.
#[derive(Clone, Copy)]
pub struct NavItemDef {
    pub id: &'static str,
    pub icon: IconName,
    pub label: &'static str,
}

/// The built-in navigation sections, in display order.
pub const NAV_ITEMS: [NavItemDef; 6] = [
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

/// Builds the standard Siegu sidebar navigation group.
///
/// `on_nav` is invoked with the selected item's id whenever a user taps a nav
/// item, and `active` marks the item for the section currently being shown.
pub fn siegu_nav_items(on_nav: Rc<dyn Fn(&str, &mut App)>, active: Option<&str>) -> NavGroup {
    let mut group = NavGroup::new("siegu-nav", "Siegu", true);
    for item in NAV_ITEMS {
        let handler = Rc::clone(&on_nav);
        let id = item.id;
        group = group.child(
            NavItem::new(item.id, item.icon, item.label)
                .active(active == Some(item.id))
                .on_click(move |_, cx| handler(id, cx)),
        );
    }
    group
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
                "duplicates",
                "settings"
            ]
        );
    }
}
