// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

//! Split Button Component
//!
//! A button with two parts: a main action button and a dropdown trigger.
//! Commonly used for buttons with multiple related actions.

use gpui::*;
use gpui_component::{
    Disableable, Icon, IconName, Selectable,
    button::{Button, ButtonVariants},
    h_flex,
    menu::{DropdownMenu, PopupMenu},
};

use crate::assets::{AppIcon, ButtonAppIconExt};

/// Creates a split button with an icon on the left and a labeled dropdown on the right.
///
/// Layout: `[icon] [label ▼]`
///
/// # Arguments
/// * `id` - Unique identifier for the button
/// * `icon` - Icon to display on the main button (left side)
/// * `label` - Text label for the dropdown button (right side)
/// * `selected` - Whether the button appears selected/active
/// * `disabled` - Whether the button is disabled
/// * `on_click` - Handler for main icon button clicks
/// * `dropdown_builder` - Builder for the dropdown menu
///
/// # Example
///
/// ```rust,ignore
/// split_button_labeled(
///     "reasoning",
///     AppIcon::Brain,
///     "Medium",
///     true,
///     false,
///     cx.listener(|this, _, _, cx| this.toggle_reasoning(cx)),
///     |menu, _, _| {
///         menu.item(PopupMenuItem::new("Off"))
///             .item(PopupMenuItem::new("Low"))
///             .item(PopupMenuItem::new("Medium"))
///             .item(PopupMenuItem::new("High"))
///     },
/// )
/// ```
pub fn split_button_labeled<M>(
    id: impl Into<SharedString>,
    icon: AppIcon,
    label: impl Into<SharedString>,
    selected: bool,
    disabled: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    dropdown_builder: M,
) -> impl IntoElement
where
    M: Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static,
{
    let id = id.into();
    let label = label.into();
    let main_id = SharedString::from(format!("{}-main", id));
    let dropdown_id = SharedString::from(format!("{}-dropdown", id));

    div()
        .id(id)
        .flex()
        .items_center()
        // Icon button (left side)
        .child(
            Button::new(main_id)
                .app_icon(icon)
                .ghost()
                .selected(selected)
                .disabled(disabled)
                .rounded_r_none()
                .border_r_0()
                .on_click(on_click),
        )
        // Label + dropdown trigger (right side)
        .child(
            Button::new(dropdown_id)
                .child(
                    h_flex()
                        .gap_1()
                        .items_center()
                        .child(label)
                        .child(Icon::new(IconName::ChevronDown).size_4()),
                )
                .ghost()
                .compact()
                .selected(selected)
                .disabled(disabled)
                .rounded_l_none()
                .dropdown_menu(dropdown_builder),
        )
}
