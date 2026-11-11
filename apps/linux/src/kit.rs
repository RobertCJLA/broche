// SPDX-License-Identifier: GPL-3.0-or-later
//! Material 3 building blocks drawn with egui: cards, buttons, switches, chips,
//! segmented buttons and list rows. All of them report their role and state to
//! screen readers (AccessKit) and keep a visible outline on every surface.

use eframe::egui::{
    self, Color32, CornerRadius, Margin, Response, RichText, Sense, Stroke, StrokeKind, WidgetInfo,
    WidgetType,
};

use crate::theme::{RADIUS_CARD, RADIUS_FULL, RADIUS_SMALL, pal};

/// A glass pane: translucent over the backdrop, with a light rim and a soft shadow that lifts it
/// off the backdrop. Add the inner margin; after `show`, [`glass_edge`] adds the light on top.
pub fn glass(ui: &egui::Ui) -> egui::Frame {
    let p = pal(ui);
    let dark = ui.visuals().dark_mode;
    egui::Frame::new()
        .fill(p.glass(dark))
        .stroke(Stroke::new(1.0_f32, p.rim(dark)))
        .corner_radius(CornerRadius::same(RADIUS_CARD))
        .shadow(egui::Shadow {
            offset: [0, 10],
            blur: 30,
            spread: 0,
            color: Color32::from_black_alpha(if dark { 70 } else { 22 }),
        })
}

/// The brighter line along a glass pane's top edge (light from above).
pub fn glass_edge(ui: &egui::Ui, pane: egui::Rect) {
    let p = pal(ui);
    let dark = ui.visuals().dark_mode;
    let inset = f32::from(RADIUS_CARD);
    let y = pane.top() + 1.0;
    ui.painter().line_segment(
        [egui::pos2(pane.left() + inset, y), egui::pos2(pane.right() - inset, y)],
        Stroke::new(1.0_f32, p.rim(dark).gamma_multiply(if dark { 2.4 } else { 1.0 })),
    );
}

/// A glass card: one idea per card.
pub fn card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let shown = glass(ui).inner_margin(Margin::same(18)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        add(ui)
    });
    glass_edge(ui, shown.response.rect);
    shown.inner
}

/// A card tinted with a colour (for status and warnings): glass too, but nearly opaque so the
/// text on it keeps the contrast measured for that colour.
pub fn tinted_card<R>(ui: &mut egui::Ui, fill: Color32, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let rim = pal(ui).rim(ui.visuals().dark_mode);
    egui::Frame::new()
        .fill(fill.gamma_multiply(0.85))
        .stroke(Stroke::new(1.0_f32, rim))
        .corner_radius(CornerRadius::same(RADIUS_CARD))
        .inner_margin(Margin::same(16))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

/// The card title the settings search jumped to; `title` scrolls there once.
pub const SCROLL_TO: &str = "settings-scroll-to";

/// The title of a card (Material "title medium").
pub fn title(ui: &mut egui::Ui, text: &str) {
    let r = ui.label(RichText::new(text).size(17.0).strong().color(pal(ui).on_surface));
    let id = egui::Id::new(SCROLL_TO);
    if ui.ctx().data(|d| d.get_temp::<String>(id)).as_deref() == Some(text) {
        r.scroll_to_me(Some(egui::Align::TOP));
        ui.ctx().data_mut(|d| d.remove::<String>(id));
    }
}

/// The headline of a page.
pub fn headline(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).size(26.0).color(pal(ui).on_surface));
}

/// Secondary text that explains something; it wraps.
pub fn supporting(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.add(egui::Label::new(RichText::new(text.into()).size(13.5).color(pal(ui).on_surface_variant)).wrap());
}

/// Material state layer: a tint while hovered or pressed, a short flash after a click and a
/// focus ring, so every button shows that it was touched (buttons with their own colours
/// don't get egui's hover look).
pub fn feedback(ui: &egui::Ui, r: &Response, on: Color32, radius: CornerRadius) {
    let now = ui.input(|i| i.time);
    let id = r.id.with("flash");
    if r.clicked() {
        ui.data_mut(|d| d.insert_temp(id, now));
    }
    // Hover fades in and out instead of switching (instant with reduced motion).
    let hover = ui.ctx().animate_bool_with_time(r.id.with("hover"), r.hovered(), crate::theme::anim(0.12));
    let mut alpha: f32 = if r.is_pointer_button_down_on() { 0.18 } else { 0.08 * hover };
    if let Some(t) = ui.data(|d| d.get_temp::<f64>(id)) {
        let since = (now - t) as f32;
        if since < 0.4 {
            alpha = alpha.max(0.28 * (1.0 - since / 0.4));
            ui.ctx().request_repaint();
        }
    }
    if !ui.is_rect_visible(r.rect) {
        return;
    }
    if alpha > 0.0 {
        ui.painter().rect_filled(r.rect, radius, on.gamma_multiply(alpha));
    }
    if r.has_focus() {
        ui.painter().rect_stroke(
            r.rect.expand(2.0),
            radius,
            Stroke::new(2.0_f32, pal(ui).primary),
            StrokeKind::Outside,
        );
    }
}

/// What a screen reader should say for a label that starts with an icon: "🔔 Ring" → "Ring".
/// Text without letters or digits (a lone icon) is kept as it is.
pub fn spoken(text: &str) -> &str {
    let rest = text.trim_start_matches(|c: char| !c.is_alphanumeric());
    if rest.is_empty() { text } else { rest }
}

/// An icon that only decorates: painted, so screen readers skip it (a label would make them
/// read the emoji's name, e.g. "mobile phone").
pub fn glyph(ui: &mut egui::Ui, icon: &str, size: f32, color: Color32) -> Response {
    let galley = ui.painter().layout_no_wrap(icon.to_owned(), egui::FontId::proportional(size), color);
    let (rect, r) = ui.allocate_exact_size(galley.size(), Sense::hover());
    ui.painter().galley(rect.min, galley, color);
    r
}

fn button(ui: &mut egui::Ui, text: &str, fill: Color32, fg: Color32, stroke: Stroke) -> Response {
    let r = ui.add(
        egui::Button::new(RichText::new(text).color(fg).strong())
            .fill(fill)
            .stroke(stroke)
            .corner_radius(CornerRadius::same(RADIUS_FULL))
            .min_size(egui::vec2(48.0, 40.0)),
    );
    feedback(ui, &r, fg, CornerRadius::same(RADIUS_FULL));
    named(r, spoken(text)).on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// The main thing to do on a card.
pub fn filled_button(ui: &mut egui::Ui, text: &str) -> Response {
    let p = pal(ui);
    button(ui, text, p.primary, p.on_primary, Stroke::NONE)
}

/// An important action that is not the main one.
pub fn tonal_button(ui: &mut egui::Ui, text: &str) -> Response {
    let p = pal(ui);
    button(ui, text, p.secondary_container, p.on_secondary_container, Stroke::new(1.0_f32, p.outline_variant))
}

/// A dangerous action (unpair, delete).
pub fn danger_button(ui: &mut egui::Ui, text: &str) -> Response {
    let p = pal(ui);
    button(ui, text, Color32::TRANSPARENT, p.error, Stroke::new(1.0_f32, p.error))
}

/// A Material switch (52×32). Returns the response; `changed()` tells if it flipped.
pub fn switch(ui: &mut egui::Ui, on: &mut bool, label: &str) -> Response {
    let size = egui::vec2(52.0, 32.0);
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    let (enabled, value, name) = (ui.is_enabled(), *on, label.to_string());
    response.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, enabled, value, &name));
    if ui.is_rect_visible(rect) {
        let p = pal(ui);
        let t = ui.ctx().animate_bool_responsive(response.id, *on);
        let track = if *on { p.primary } else { p.surface_container_highest };
        let painter = ui.painter();
        painter.rect(
            rect,
            CornerRadius::same(16),
            track,
            Stroke::new(2.0_f32, if *on { p.primary } else { p.outline }),
            StrokeKind::Inside,
        );
        let radius = egui::lerp(8.0..=12.0, t) + if response.is_pointer_button_down_on() { 2.0 } else { 0.0 };
        let x = egui::lerp((rect.left() + 16.0)..=(rect.right() - 16.0), t);
        let thumb = if *on { p.on_primary } else { p.outline };
        if response.hovered() || response.has_focus() {
            painter.circle_filled(egui::pos2(x, rect.center().y), 18.0, p.on_surface.gamma_multiply(0.10));
        }
        if response.has_focus() {
            painter.rect_stroke(
                rect.expand(3.0),
                CornerRadius::same(19),
                Stroke::new(2.0_f32, p.primary),
                StrokeKind::Outside,
            );
        }
        painter.circle_filled(egui::pos2(x, rect.center().y), radius, thumb);
    }
    response
}

/// A settings row: title and explanation on the left, a switch on the right.
/// Returns true when the value changed.
pub fn switch_row(ui: &mut egui::Ui, on: &mut bool, title_text: &str, explanation: Option<&str>) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        let switch_w = 52.0 + ui.spacing().item_spacing.x;
        ui.allocate_ui_with_layout(
            egui::vec2((ui.available_width() - switch_w).max(80.0), 0.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.add(egui::Label::new(RichText::new(title_text).color(pal(ui).on_surface)).wrap());
                if let Some(e) = explanation {
                    supporting(ui, e);
                }
            },
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let r = switch(ui, on, title_text);
            // The explanation is read with the switch, not only as a separate line.
            if let Some(e) = explanation {
                r.ctx.accesskit_node_builder(r.id, |n| n.set_description(e));
            }
            changed = r.changed();
        });
    });
    changed
}

/// A filter chip (32 px high, small corners). Selected chips show a check mark.
pub fn chip(ui: &mut egui::Ui, selected: bool, text: &str) -> Response {
    let p = pal(ui);
    let label = if selected { format!("✔ {text}") } else { text.to_string() };
    let r = ui.add(
        egui::Button::new(RichText::new(label).color(if selected {
            p.on_primary_container
        } else {
            p.on_surface_variant
        }))
        .fill(if selected { p.primary_container } else { Color32::TRANSPARENT })
        .stroke(if selected { Stroke::NONE } else { Stroke::new(1.0_f32, p.outline) })
        .corner_radius(CornerRadius::same(RADIUS_SMALL))
        .min_size(egui::vec2(0.0, 32.0)),
    );
    feedback(ui, &r, p.on_surface, CornerRadius::same(RADIUS_SMALL));
    let name = spoken(text).to_string();
    r.widget_info(|| WidgetInfo::selected(WidgetType::SelectableLabel, true, selected, &name));
    r
}

/// Segmented buttons: pick one of a few options. Returns true when it changed.
pub fn segmented<T: PartialEq + Copy>(ui: &mut egui::Ui, value: &mut T, options: &[(T, String)]) -> bool {
    let p = pal(ui);
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let n = options.len();
        for (i, (v, label)) in options.iter().enumerate() {
            let selected = *value == *v;
            let r = |first: bool, last: bool| CornerRadius {
                nw: if first { RADIUS_FULL } else { 0 },
                sw: if first { RADIUS_FULL } else { 0 },
                ne: if last { RADIUS_FULL } else { 0 },
                se: if last { RADIUS_FULL } else { 0 },
            };
            let text = if selected { format!("✔ {label}") } else { label.clone() };
            let resp = ui.add(
                egui::Button::new(RichText::new(text).color(if selected {
                    p.on_secondary_container
                } else {
                    p.on_surface
                }))
                .fill(if selected { p.primary_container } else { Color32::TRANSPARENT })
                .stroke(Stroke::new(1.0_f32, p.outline))
                .corner_radius(r(i == 0, i + 1 == n))
                .min_size(egui::vec2(64.0, 40.0)),
            );
            feedback(ui, &resp, p.on_surface, r(i == 0, i + 1 == n));
            let name = spoken(label).to_string();
            resp.widget_info(|| WidgetInfo::selected(WidgetType::RadioButton, true, selected, &name));
            if resp.clicked() && !selected {
                *value = *v;
                changed = true;
            }
        }
    });
    changed
}

/// A round icon button with a tooltip (the tooltip is also its accessible name).
pub fn icon_button(ui: &mut egui::Ui, icon: &str, tip: &str) -> Response {
    let p = pal(ui);
    let r = ui
        .add(
            egui::Button::new(RichText::new(icon).size(18.0).color(p.on_surface_variant))
                .fill(Color32::TRANSPARENT)
                .stroke(Stroke::NONE)
                .corner_radius(CornerRadius::same(RADIUS_FULL))
                .min_size(egui::vec2(40.0, 40.0)),
        )
        .on_hover_text(tip);
    feedback(ui, &r, p.on_surface, CornerRadius::same(RADIUS_FULL));
    let r = r.on_hover_cursor(egui::CursorIcon::PointingHand);
    let name = tip.to_string();
    r.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, &name));
    r
}

/// A round icon button with a tonal container: for actions in lists (call, message).
pub fn tonal_icon_button(ui: &mut egui::Ui, icon: &str, tip: &str) -> Response {
    let p = pal(ui);
    let r = ui
        .add(
            egui::Button::new(RichText::new(icon).size(17.0).color(p.on_secondary_container))
                .fill(p.secondary_container)
                .stroke(Stroke::new(1.0_f32, p.outline_variant))
                .corner_radius(CornerRadius::same(RADIUS_FULL))
                .min_size(egui::vec2(40.0, 40.0)),
        )
        .on_hover_text(tip);
    feedback(ui, &r, p.on_surface, CornerRadius::same(RADIUS_FULL));
    let r = r.on_hover_cursor(egui::CursorIcon::PointingHand);
    let name = tip.to_string();
    r.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, &name));
    r
}

/// What a page shows when there is nothing to show yet.
pub fn empty_state(ui: &mut egui::Ui, icon: &str, title_text: &str, body: &str) {
    ui.add_space(24.0);
    ui.vertical_centered(|ui| {
        glyph(ui, icon, 40.0, pal(ui).on_surface_variant);
        ui.add_space(4.0);
        title(ui, title_text);
        supporting(ui, body);
    });
    ui.add_space(24.0);
}

/// Gives a control the name a screen reader says: egui only names buttons and labels, so
/// text fields, combo boxes and sliders are otherwise read as just "entry", "combo box"…
pub fn named(r: Response, name: &str) -> Response {
    r.ctx.accesskit_node_builder(r.id, |n| n.set_label(name));
    r
}

/// How wide a button with this text will be, so a row can be laid out left to right
/// (the order keyboard and screen readers follow) and still end at the right edge.
pub fn button_width(ui: &egui::Ui, text: RichText, min: f32) -> f32 {
    let galley = egui::WidgetText::from(text).into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        egui::TextStyle::Button,
    );
    (galley.size().x + 2.0 * ui.spacing().button_padding.x).max(min)
}

/// A single-line text field as wide as the space left minus `reserve` px.
pub fn field(ui: &mut egui::Ui, text: &mut String, hint: &str, reserve: f32) -> Response {
    let w = (ui.available_width() - reserve).max(120.0);
    let r = ui.add(
        egui::TextEdit::singleline(text).hint_text(hint).desired_width(w).margin(egui::vec2(12.0, 10.0)),
    );
    named(r, hint)
}

/// A Material search bar: a full-width pill with a magnifier, the field and, once there is
/// text, a button that clears it. Returns the text field's response.
pub fn search_field(ui: &mut egui::Ui, text: &mut String, hint: &str) -> Response {
    let p = pal(ui);
    egui::Frame::new()
        .fill(p.surface_container_high)
        .stroke(Stroke::new(1.0_f32, p.outline_variant))
        .corner_radius(CornerRadius::same(RADIUS_FULL))
        .inner_margin(Margin { left: 16, right: 6, top: 4, bottom: 4 })
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                glyph(ui, "🔍", 16.0, p.on_surface_variant);
                let clear = if text.is_empty() { 0.0 } else { 44.0 };
                let r = ui.add(
                    egui::TextEdit::singleline(text)
                        .hint_text(hint)
                        .frame(false)
                        .margin(egui::vec2(6.0, 10.0))
                        .desired_width((ui.available_width() - clear).max(80.0)),
                );
                let r = named(r, hint);
                if !text.is_empty() && icon_button(ui, "✖", &crate::i18n::t("Clear")).clicked() {
                    text.clear();
                    r.request_focus();
                }
                r
            })
            .inner
        })
        .inner
}

/// A text field and a button on one row: the button keeps its size and the field takes
/// the rest, so the row never gets wider than its card. Returns (field, button clicked, Enter pressed).
pub fn field_with_button(
    ui: &mut egui::Ui,
    text: &mut String,
    hint: &str,
    button_text: &str,
    filled: bool,
) -> (Response, bool, bool) {
    // Measure the button first so the field can come first: keyboard and screen readers
    // follow the order widgets are added, and the text belongs before its button.
    let button_w = button_width(ui, RichText::new(button_text).strong(), 48.0);
    let mut out = None;
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), 44.0),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            let margin = egui::vec2(12.0, 10.0);
            // desired_width is the text area; the margins are added around it.
            let w =
                (ui.available_width() - button_w - ui.spacing().item_spacing.x - 2.0 * margin.x).max(80.0);
            let r = ui.add(egui::TextEdit::singleline(text).hint_text(hint).desired_width(w).margin(margin));
            let r = named(r, hint);
            let clicked =
                if filled { filled_button(ui, button_text) } else { tonal_button(ui, button_text) }.clicked();
            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            out = Some((r, clicked, enter));
        },
    );
    out.expect("row drawn")
}

/// A small filled dot (status).
pub fn dot(ui: &mut egui::Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), Sense::hover());
    ui.painter().circle_filled(rect.center(), 5.0, color);
}

/// A thin divider inside a card.
pub fn divider(ui: &mut egui::Ui) {
    ui.add_space(4.0);
    let p = pal(ui);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, p.outline_variant);
    ui.add_space(4.0);
}

/// Lays `n` items out in as many equal columns as fit (`min` px each, at most `max_cols`).
pub fn columns_for(width: f32, min: f32, gap: f32, max_cols: usize) -> (usize, f32) {
    let cols = (((width + gap) / (min + gap)).floor() as usize).clamp(1, max_cols.max(1));
    let w = ((width - gap * (cols as f32 - 1.0)) / cols as f32).max(1.0);
    (cols, w)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spoken_drops_leading_icons() {
        assert_eq!(spoken("🔔 Ring"), "Ring");
        assert_eq!(spoken("⬆ Upload files…"), "Upload files…");
        assert_eq!(spoken("+ New folder"), "New folder");
        assert_eq!(spoken("100 %"), "100 %");
        assert_eq!(spoken("Send"), "Send");
        assert_eq!(spoken("⟳"), "⟳");
    }

    #[test]
    fn grid_columns() {
        assert_eq!(columns_for(100.0, 150.0, 8.0, 10).0, 1);
        let (c, w) = columns_for(800.0, 150.0, 8.0, 10);
        assert_eq!(c, 5);
        assert!((w * 5.0 + 8.0 * 4.0 - 800.0).abs() < 0.01);
        assert_eq!(columns_for(5000.0, 150.0, 8.0, 6).0, 6);
    }
}
