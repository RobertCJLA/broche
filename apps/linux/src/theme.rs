// SPDX-License-Identifier: GPL-3.0-or-later
//! Colors and shapes. The widgets follow Material 3 (roles such as `primary`,
//! `surface_container` or `outline`), coloured with a Catppuccin or Tokyo Night
//! palette. Every scheme has a dark and a light flavour; egui switches between
//! them when the mode follows the system.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use eframe::egui::{self, Color32, CornerRadius, Shadow, Stroke, Theme, ThemePreference};

/// The raw colours of one flavour (Catppuccin names; Tokyo Night is mapped onto them).
#[derive(Debug, Clone, Copy)]
struct Flavor {
    accent: Color32,
    red: Color32,
    green: Color32,
    peach: Color32,
    blue: Color32,
    text: Color32,
    subtext: Color32,
    overlay1: Color32,
    overlay0: Color32,
    surface2: Color32,
    surface1: Color32,
    surface0: Color32,
    base: Color32,
    mantle: Color32,
    crust: Color32,
}

const fn hex(rgb: u32) -> Color32 {
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

#[allow(clippy::too_many_arguments)]
const fn flavor(
    accent: u32,
    red: u32,
    green: u32,
    peach: u32,
    blue: u32,
    text: u32,
    subtext: u32,
    overlay1: u32,
    overlay0: u32,
    surface2: u32,
    surface1: u32,
    surface0: u32,
    base: u32,
    mantle: u32,
    crust: u32,
) -> Flavor {
    Flavor {
        accent: hex(accent),
        red: hex(red),
        green: hex(green),
        peach: hex(peach),
        blue: hex(blue),
        text: hex(text),
        subtext: hex(subtext),
        overlay1: hex(overlay1),
        overlay0: hex(overlay0),
        surface2: hex(surface2),
        surface1: hex(surface1),
        surface0: hex(surface0),
        base: hex(base),
        mantle: hex(mantle),
        crust: hex(crust),
    }
}

// https://catppuccin.com/palette
const MOCHA: Flavor = flavor(
    0xcba6f7, 0xf38ba8, 0xa6e3a1, 0xfab387, 0x89b4fa, 0xcdd6f4, 0xa6adc8, 0x7f849c, 0x6c7086, 0x585b70,
    0x45475a, 0x313244, 0x1e1e2e, 0x181825, 0x11111b,
);
const MACCHIATO: Flavor = flavor(
    0xc6a0f6, 0xed8796, 0xa6da95, 0xf5a97f, 0x8aadf4, 0xcad3f5, 0xa5adcb, 0x8087a2, 0x6e738d, 0x5b6078,
    0x494d64, 0x363a4f, 0x24273a, 0x1e2030, 0x181926,
);
const FRAPPE: Flavor = flavor(
    0xca9ee6, 0xe78284, 0xa6d189, 0xef9f76, 0x8caaee, 0xc6d0f5, 0xa5adce, 0x838ba7, 0x737994, 0x626880,
    0x51576d, 0x414559, 0x303446, 0x292c3c, 0x232634,
);
const LATTE: Flavor = flavor(
    0x8839ef, 0xd20f39, 0x40a02b, 0xfe640b, 0x1e66f5, 0x4c4f69, 0x6c6f85, 0x8c8fa1, 0x9ca0b0, 0xacb0be,
    0xbcc0cc, 0xccd0da, 0xeff1f5, 0xe6e9ef, 0xdce0e8,
);
// https://github.com/folke/tokyonight.nvim (extras/lua)
const TOKYO_NIGHT: Flavor = flavor(
    0x7aa2f7, 0xf7768e, 0x9ece6a, 0xff9e64, 0x7dcfff, 0xc0caf5, 0xa9b1d6, 0x737aa2, 0x565f89, 0x545c7e,
    0x414868, 0x292e42, 0x1a1b26, 0x16161e, 0x0f0f14,
);
const TOKYO_STORM: Flavor = flavor(
    0x7aa2f7, 0xf7768e, 0x9ece6a, 0xff9e64, 0x7dcfff, 0xc0caf5, 0xa9b1d6, 0x737aa2, 0x565f89, 0x545c7e,
    0x414868, 0x292e42, 0x24283b, 0x1f2335, 0x1b1e2d,
);
const TOKYO_MOON: Flavor = flavor(
    0x82aaff, 0xff757f, 0xc3e88d, 0xff966c, 0x86e1fc, 0xc8d3f5, 0xa9b8e8, 0x737aa2, 0x636da6, 0x545c7e,
    0x444a73, 0x2f334d, 0x222436, 0x1e2030, 0x191a2a,
);
const TOKYO_DAY: Flavor = flavor(
    0x2e7de9, 0xc64343, 0x587539, 0xb15c00, 0x007197, 0x3760bf, 0x4f5e9c, 0x68709a, 0x848cb5, 0xa1a6c5,
    0xb7c1e3, 0xc4c8da, 0xe1e2e7, 0xd0d5e3, 0xc7cbdb,
);

/// A colour scheme the user can pick. Each has a dark and a light flavour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scheme {
    Mocha,
    Macchiato,
    Frappe,
    TokyoNight,
    TokyoStorm,
    TokyoMoon,
}

impl Scheme {
    pub const ALL: [Scheme; 6] = [
        Scheme::Mocha,
        Scheme::Macchiato,
        Scheme::Frappe,
        Scheme::TokyoNight,
        Scheme::TokyoStorm,
        Scheme::TokyoMoon,
    ];

    /// The name saved in the settings file (the same ids as the Android app where they exist).
    pub fn id(self) -> &'static str {
        match self {
            Scheme::Mocha => "catppuccin",
            Scheme::Macchiato => "macchiato",
            Scheme::Frappe => "frappe",
            Scheme::TokyoNight => "tokyo",
            Scheme::TokyoStorm => "tokyo-storm",
            Scheme::TokyoMoon => "tokyo-moon",
        }
    }

    pub fn from_id(id: &str) -> Scheme {
        Scheme::ALL.into_iter().find(|s| s.id() == id).unwrap_or(Scheme::Mocha)
    }

    pub fn label(self) -> &'static str {
        match self {
            Scheme::Mocha => "Catppuccin Mocha / Latte",
            Scheme::Macchiato => "Catppuccin Macchiato / Latte",
            Scheme::Frappe => "Catppuccin Frappé / Latte",
            Scheme::TokyoNight => "Tokyo Night / Day",
            Scheme::TokyoStorm => "Tokyo Night Storm / Day",
            Scheme::TokyoMoon => "Tokyo Night Moon / Day",
        }
    }

    fn flavors(self) -> (Flavor, Flavor) {
        match self {
            Scheme::Mocha => (MOCHA, LATTE),
            Scheme::Macchiato => (MACCHIATO, LATTE),
            Scheme::Frappe => (FRAPPE, LATTE),
            Scheme::TokyoNight => (TOKYO_NIGHT, TOKYO_DAY),
            Scheme::TokyoStorm => (TOKYO_STORM, TOKYO_DAY),
            Scheme::TokyoMoon => (TOKYO_MOON, TOKYO_DAY),
        }
    }
}

static SCHEME: AtomicU8 = AtomicU8::new(0);

pub(crate) fn current_scheme() -> Scheme {
    Scheme::ALL.get(SCHEME.load(Ordering::Relaxed) as usize).copied().unwrap_or(Scheme::Mocha)
}

/// Material 3 colour roles.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub primary: Color32,
    pub on_primary: Color32,
    pub primary_container: Color32,
    pub on_primary_container: Color32,
    pub secondary_container: Color32,
    pub on_secondary_container: Color32,
    pub surface: Color32,
    pub surface_container_lowest: Color32,
    pub surface_container_low: Color32,
    pub surface_container: Color32,
    pub surface_container_high: Color32,
    pub surface_container_highest: Color32,
    pub on_surface: Color32,
    pub on_surface_variant: Color32,
    pub outline: Color32,
    pub outline_variant: Color32,
    pub error: Color32,
    pub error_container: Color32,
    pub on_error_container: Color32,
    pub success: Color32,
    pub warning: Color32,
    pub info: Color32,
}

/// `a` moved towards `b` by `t` (0..1), in sRGB (good enough for UI tints).
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

/// `fg` pushed towards `towards` just enough to reach `min` contrast on every background.
fn ensure(fg: Color32, backgrounds: &[Color32], min: f32, towards: Color32) -> Color32 {
    (0..=40)
        .map(|i| mix(fg, towards, i as f32 / 40.0))
        .find(|c| backgrounds.iter().all(|bg| contrast(*c, *bg) >= min))
        .unwrap_or(towards)
}

/// Raises text and borders to WCAG AA: 4.5:1 for text, 3:1 for outlines and the accent.
fn accessible(mut p: Palette, dark: bool) -> Palette {
    let (light_end, dark_end) = (Color32::WHITE, Color32::BLACK);
    let towards = if dark { light_end } else { dark_end };
    let mut surfaces =
        vec![p.surface, p.surface_container_low, p.surface_container, p.surface_container_high];
    surfaces.extend(p.backgrounds(dark));
    p.on_surface = ensure(p.on_surface, &surfaces, 4.5, towards);
    p.on_surface_variant = ensure(p.on_surface_variant, &surfaces, 4.5, towards);
    p.outline = ensure(p.outline, &surfaces, 3.0, towards);
    // Text on the accent: keep the accent, darken it in light mode if white text is too faint.
    if !dark {
        p.primary = ensure(p.primary, &[p.on_primary], 4.5, dark_end);
    }
    p.primary = ensure(p.primary, &surfaces, 3.0, towards);
    p.on_primary = ensure(p.on_primary, &[p.primary], 4.5, if dark { dark_end } else { light_end });
    p.on_primary_container = ensure(p.on_primary_container, &[p.primary_container], 4.5, towards);
    p.on_secondary_container = ensure(p.on_secondary_container, &[p.secondary_container], 4.5, towards);
    p.on_error_container = ensure(p.on_error_container, &[p.error_container], 4.5, towards);
    p.error = ensure(p.error, &surfaces, 3.0, towards);
    // The backdrop is tinted with the accent, which may have just moved: check text once more.
    let last = p.backgrounds(dark);
    p.on_surface = ensure(p.on_surface, &last, 4.5, towards);
    p.on_surface_variant = ensure(p.on_surface_variant, &last, 4.5, towards);
    p
}

fn roles(f: &Flavor, dark: bool) -> Palette {
    accessible(raw_roles(f, dark), dark)
}

fn raw_roles(f: &Flavor, dark: bool) -> Palette {
    if dark {
        Palette {
            primary: f.accent,
            on_primary: f.crust,
            primary_container: mix(f.base, f.accent, 0.32),
            on_primary_container: mix(f.accent, Color32::WHITE, 0.55),
            secondary_container: mix(f.surface0, f.accent, 0.12),
            on_secondary_container: f.text,
            surface: f.base,
            surface_container_lowest: f.crust,
            surface_container_low: f.mantle,
            surface_container: mix(f.base, f.surface0, 0.45),
            surface_container_high: f.surface0,
            surface_container_highest: f.surface1,
            on_surface: f.text,
            on_surface_variant: f.subtext,
            // At least 3:1 against every surface: borders must never disappear.
            outline: f.overlay1,
            outline_variant: f.surface2,
            error: f.red,
            error_container: mix(f.base, f.red, 0.28),
            on_error_container: mix(f.red, Color32::WHITE, 0.5),
            success: f.green,
            warning: f.peach,
            info: f.blue,
        }
    } else {
        Palette {
            primary: f.accent,
            on_primary: Color32::WHITE,
            primary_container: mix(f.base, f.accent, 0.18),
            on_primary_container: mix(f.accent, Color32::BLACK, 0.45),
            secondary_container: mix(f.mantle, f.accent, 0.10),
            on_secondary_container: f.text,
            surface: f.base,
            surface_container_lowest: Color32::WHITE,
            surface_container_low: mix(f.base, Color32::WHITE, 0.45),
            surface_container: f.mantle,
            surface_container_high: f.crust,
            surface_container_highest: f.surface0,
            on_surface: f.text,
            on_surface_variant: f.subtext,
            outline: mix(f.overlay0, f.text, 0.35),
            outline_variant: f.surface1,
            error: f.red,
            error_container: mix(f.base, f.red, 0.16),
            on_error_container: mix(f.red, Color32::BLACK, 0.4),
            success: f.green,
            warning: f.peach,
            info: f.blue,
        }
    }
}

/// The palette of what is on screen now.
pub fn pal(ui: &egui::Ui) -> Palette {
    palette_for(current_scheme(), ui.visuals().dark_mode)
}

pub fn palette_for(scheme: Scheme, dark: bool) -> Palette {
    let (d, l) = scheme.flavors();
    if dark { roles(&d, true) } else { roles(&l, false) }
}

/// Glass: translucent panes over a backdrop of soft colour, the way the original NectarLink drew
/// its glass theme over its own backdrop. egui can't blur what is behind the window, so Broche
/// paints the backdrop itself ([`paint_backdrop`]) and the panes ([`Palette::glass`]) let it through.
impl Palette {
    /// The backdrop's colour patches: a colour and how strongly it tints the base at its centre.
    fn blobs(&self, dark: bool) -> [(Color32, f32); 3] {
        let k = if dark { 0.32 } else { 0.24 };
        [(self.primary, k), (self.info, k * 0.8), (self.success, k * 0.55)]
    }

    fn glass_alpha(dark: bool) -> f32 {
        if dark { 0.68 } else { 0.6 }
    }

    /// The fill of a glass pane (cards, the navigation).
    pub fn glass(&self, dark: bool) -> Color32 {
        let c = self.surface_container_low;
        Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (Self::glass_alpha(dark) * 255.0) as u8)
    }

    /// The light along a pane's edge.
    pub fn rim(&self, dark: bool) -> Color32 {
        if dark { Color32::from_white_alpha(34) } else { Color32::from_white_alpha(190) }
    }

    /// Every colour text can end up on: the backdrop (page titles sit right on it) and a glass
    /// pane over each part of it. The contrast checks use these, not the pane alone.
    fn backgrounds(&self, dark: bool) -> Vec<Color32> {
        let base = self.surface;
        let mut under = vec![base];
        under.extend(self.blobs(dark).iter().map(|(c, k)| mix(base, *c, *k)));
        let glass = self.surface_container_low;
        let a = Self::glass_alpha(dark);
        let mut all = under.clone();
        all.extend(under.iter().map(|bg| mix(*bg, glass, a)));
        all
    }
}

/// Paints the backdrop over `rect`: the base colour with three soft patches of the scheme's
/// accent colours, as a vertex-coloured mesh (one draw call, no textures).
pub fn paint_backdrop(painter: &egui::Painter, rect: egui::Rect, p: &Palette, dark: bool) {
    let (nx, ny) = (24_u32, 16_u32);
    let centres = [(0.1, 0.05), (0.95, 0.35), (0.4, 1.05)];
    let reach = rect.width().max(rect.height()) * 0.6;
    let mut mesh = egui::Mesh::default();
    for j in 0..=ny {
        for i in 0..=nx {
            let pos = rect.min
                + egui::vec2(rect.width() * i as f32 / nx as f32, rect.height() * j as f32 / ny as f32);
            let mut c = p.surface;
            for ((blob, k), (cx, cy)) in p.blobs(dark).iter().zip(centres) {
                let centre = rect.min + egui::vec2(rect.width() * cx, rect.height() * cy);
                let d = (pos - centre).length() / reach;
                c = mix(c, *blob, k * (-2.5 * d * d).exp());
            }
            mesh.colored_vertex(pos, c);
        }
    }
    for j in 0..ny {
        for i in 0..nx {
            let a = j * (nx + 1) + i;
            let below = a + nx + 1;
            mesh.add_triangle(a, a + 1, below + 1);
            mesh.add_triangle(a, below + 1, below);
        }
    }
    painter.add(mesh);
}

/// Whether things move: off when the desktop asks for reduced motion (GNOME: Accessibility ›
/// Reduce animation), read from the Settings portal.
static MOTION: AtomicBool = AtomicBool::new(true);

pub fn motion() -> bool {
    MOTION.load(Ordering::Relaxed)
}

/// Turns movement on or off; off also stops egui's own animations (folding, scrolling).
pub fn set_motion(ctx: &egui::Context, on: bool) {
    MOTION.store(on, Ordering::Relaxed);
    ctx.all_styles_mut(|s| s.animation_time = if on { 1.0 / 12.0 } else { 0.0 });
}

/// A value that glides to `target` in `seconds`, fast at first and settling softly (cubic
/// ease-out), instead of jumping. Instant with reduced motion.
pub fn glide(ctx: &egui::Context, id: egui::Id, target: f32, seconds: f32) -> f32 {
    let now = ctx.input(|i| i.time);
    // (from, to, start time)
    let state = ctx.data(|d| d.get_temp::<(f32, f32, f64)>(id));
    let (from, to, start) = match state {
        Some(s) if s.1 == target => s,
        Some((from, to, start)) => {
            let shown = ease(from, to, start, now, seconds);
            (shown, target, now)
        }
        None => (target, target, now),
    };
    if !motion() {
        ctx.data_mut(|d| d.insert_temp(id, (target, target, now)));
        return target;
    }
    ctx.data_mut(|d| d.insert_temp(id, (from, to, start)));
    let value = ease(from, to, start, now, seconds);
    if value != to {
        ctx.request_repaint();
    }
    value
}

/// `seconds`, or 0 with reduced motion: for egui's own `animate_*_with_time`.
pub fn anim(seconds: f32) -> f32 {
    if motion() { seconds } else { 0.0 }
}

/// 0 → 1 over `seconds` each time `key` changes (a page appearing); always 1 with reduced motion.
pub fn appear(ctx: &egui::Context, id: egui::Id, key: impl std::hash::Hash, seconds: f32) -> f32 {
    let key = egui::Id::new(key);
    let now = ctx.input(|i| i.time);
    let start = match ctx.data(|d| d.get_temp::<(egui::Id, f64)>(id)) {
        Some((k, start)) if k == key => start,
        // First frame: already shown, nothing to fade in.
        None => now - f64::from(seconds),
        Some(_) => now,
    };
    ctx.data_mut(|d| d.insert_temp(id, (key, start)));
    if !motion() {
        return 1.0;
    }
    let value = ease(0.0, 1.0, start, now, seconds);
    if value < 1.0 {
        ctx.request_repaint();
    }
    value
}

fn ease(from: f32, to: f32, start: f64, now: f64, seconds: f32) -> f32 {
    let t = (((now - start) as f32) / seconds.max(0.001)).clamp(0.0, 1.0);
    let k = 1.0 - (1.0 - t).powi(3);
    if t >= 1.0 { to } else { from + (to - from) * k }
}

/// Material shape scale.
pub const RADIUS_FULL: u8 = 20; // buttons and chips (40 px high)
pub const RADIUS_CARD: u8 = 16;
pub const RADIUS_SMALL: u8 = 8;

fn visuals(p: &Palette, dark: bool) -> egui::Visuals {
    let mut v = if dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    // The backdrop shows through; it is painted under everything (`paint_backdrop`).
    v.panel_fill = Color32::TRANSPARENT;
    v.window_fill = p.surface_container_high;
    v.faint_bg_color = p.surface_container_low;
    v.extreme_bg_color = p.surface_container_highest;
    v.text_edit_bg_color = Some(p.surface_container_lowest);
    v.code_bg_color = p.surface_container;
    v.override_text_color = None;
    v.weak_text_color = Some(p.on_surface_variant);
    v.hyperlink_color = p.info;
    v.warn_fg_color = p.warning;
    v.error_fg_color = p.error;
    v.selection.bg_fill = p.primary_container;
    // Also the focus ring of text fields: thick and in the accent colour.
    v.selection.stroke = Stroke::new(2.0_f32, p.on_primary_container);
    v.window_stroke = Stroke::new(1.0_f32, p.outline_variant);
    v.window_corner_radius = CornerRadius::same(28);
    v.menu_corner_radius = CornerRadius::same(RADIUS_SMALL);
    v.window_shadow = Shadow {
        offset: [0, 6],
        blur: 24,
        spread: 0,
        color: Color32::from_black_alpha(if dark { 110 } else { 40 }),
    };
    v.popup_shadow = Shadow {
        offset: [0, 3],
        blur: 12,
        spread: 0,
        color: Color32::from_black_alpha(if dark { 90 } else { 30 }),
    };
    v.slider_trailing_fill = true;
    v.handle_shape = egui::style::HandleShape::Circle;
    v.striped = false;
    v.indent_has_left_vline = false;
    v.collapsing_header_frame = true;
    v.interact_cursor = Some(egui::CursorIcon::PointingHand);
    v.disabled_alpha = 0.45;

    let full = CornerRadius::same(RADIUS_FULL);
    let w = &mut v.widgets;
    // Separators, group frames.
    w.noninteractive.bg_fill = p.surface_container_low;
    w.noninteractive.weak_bg_fill = p.surface_container_low;
    w.noninteractive.bg_stroke = Stroke::new(1.0_f32, p.outline_variant);
    w.noninteractive.fg_stroke = Stroke::new(1.0_f32, p.on_surface);
    w.noninteractive.corner_radius = CornerRadius::same(RADIUS_CARD);
    // Buttons, fields, check boxes: outlined, so they stand out on any surface.
    w.inactive.bg_fill = p.surface_container_highest;
    w.inactive.weak_bg_fill = p.secondary_container;
    w.inactive.bg_stroke = Stroke::new(1.0_f32, p.outline);
    w.inactive.fg_stroke = Stroke::new(1.5_f32, p.on_secondary_container);
    w.hovered.bg_fill = mix(p.surface_container_highest, p.primary, 0.12);
    w.hovered.weak_bg_fill = mix(p.secondary_container, p.primary, 0.14);
    w.hovered.bg_stroke = Stroke::new(1.5_f32, p.primary);
    w.hovered.fg_stroke = Stroke::new(1.5_f32, p.on_surface);
    // Pressed or focused with the keyboard: a clear 2 px ring.
    w.active.bg_fill = mix(p.surface_container_highest, p.primary, 0.22);
    w.active.weak_bg_fill = mix(p.secondary_container, p.primary, 0.24);
    w.active.bg_stroke = Stroke::new(2.0_f32, p.primary);
    w.active.fg_stroke = Stroke::new(2.0_f32, p.on_surface);
    w.open.bg_fill = p.surface_container_high;
    w.open.weak_bg_fill = p.secondary_container;
    w.open.bg_stroke = Stroke::new(1.5_f32, p.primary);
    w.open.fg_stroke = Stroke::new(1.5_f32, p.on_surface);
    for s in [&mut w.inactive, &mut w.hovered, &mut w.active, &mut w.open] {
        s.corner_radius = full;
        s.expansion = 0.0;
    }
    v
}

/// Sets both flavours of `scheme` (egui switches between them), the mode and the text size.
pub fn apply(ctx: &egui::Context, preference: ThemePreference, scheme: Scheme) {
    SCHEME.store(Scheme::ALL.iter().position(|s| *s == scheme).unwrap_or(0) as u8, Ordering::Relaxed);
    ctx.set_visuals_of(Theme::Dark, visuals(&palette_for(scheme, true), true));
    ctx.set_visuals_of(Theme::Light, visuals(&palette_for(scheme, false), false));
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 8.0);
        s.spacing.button_padding = egui::vec2(16.0, 8.0);
        s.spacing.interact_size = egui::vec2(40.0, 40.0);
        s.spacing.icon_width = 20.0;
        s.spacing.icon_width_inner = 10.0;
        s.spacing.icon_spacing = 8.0;
        s.spacing.slider_width = 200.0;
        s.spacing.combo_height = 320.0;
        s.spacing.menu_margin = egui::Margin::same(8);
        s.spacing.window_margin = egui::Margin::same(24);
        s.spacing.scroll = egui::style::ScrollStyle::floating();
        s.text_styles.insert(egui::TextStyle::Heading, egui::FontId::proportional(22.0));
        s.text_styles.insert(egui::TextStyle::Body, egui::FontId::proportional(15.0));
        s.text_styles.insert(egui::TextStyle::Button, egui::FontId::proportional(14.5));
        s.text_styles.insert(egui::TextStyle::Small, egui::FontId::proportional(12.5));
        s.text_styles.insert(egui::TextStyle::Monospace, egui::FontId::monospace(13.5));
    });
    ctx.set_theme(preference);
}

/// WCAG contrast ratio between two opaque colours.
pub fn contrast(a: Color32, b: Color32) -> f32 {
    let lum = |c: Color32| {
        let ch = |v: u8| {
            let v = f32::from(v) / 255.0;
            if v <= 0.039_28 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * ch(c.r()) + 0.7152 * ch(c.g()) + 0.0722 * ch(c.b())
    };
    let (x, y) = (lum(a), lum(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catppuccin_values() {
        assert_eq!(MOCHA.base, Color32::from_rgb(0x1e, 0x1e, 0x2e));
        assert_eq!(MOCHA.accent, Color32::from_rgb(0xcb, 0xa6, 0xf7));
        assert_eq!(LATTE.base, Color32::from_rgb(0xef, 0xf1, 0xf5));
        assert_eq!(LATTE.text, Color32::from_rgb(0x4c, 0x4f, 0x69));
    }

    #[test]
    fn ease_starts_fast_and_lands_exactly() {
        assert_eq!(ease(0.0, 100.0, 0.0, 0.0, 0.2), 0.0);
        let half = ease(0.0, 100.0, 0.0, 0.1, 0.2);
        assert!(half > 80.0 && half < 100.0, "{half}");
        assert_eq!(ease(0.0, 100.0, 0.0, 0.2, 0.2), 100.0);
        assert_eq!(ease(0.0, 100.0, 0.0, 5.0, 0.2), 100.0);
    }

    #[test]
    fn ids_round_trip() {
        for s in Scheme::ALL {
            assert_eq!(Scheme::from_id(s.id()), s);
        }
        assert_eq!(Scheme::from_id("unknown"), Scheme::Mocha);
    }

    /// Text is readable (4.5:1) and borders visible (3:1) on every surface, in every scheme.
    #[test]
    fn accessible_contrast() {
        let mut bad = vec![];
        for s in Scheme::ALL {
            for dark in [true, false] {
                let p = palette_for(s, dark);
                let mut need = |what: &str, a: Color32, b: Color32, min: f32| {
                    let c = contrast(a, b);
                    if c < min {
                        bad.push(format!("{s:?} dark={dark} {what}: {c:.2} < {min}"));
                    }
                };
                for (n, bg) in [
                    ("surface", p.surface),
                    ("low", p.surface_container_low),
                    ("container", p.surface_container),
                    ("high", p.surface_container_high),
                ] {
                    need(&format!("text/{n}"), p.on_surface, bg, 4.5);
                    need(&format!("variant/{n}"), p.on_surface_variant, bg, 4.5);
                    need(&format!("outline/{n}"), p.outline, bg, 3.0);
                }
                need("on_primary", p.on_primary, p.primary, 4.5);
                need("primary_container", p.on_primary_container, p.primary_container, 4.5);
                need("secondary_container", p.on_secondary_container, p.secondary_container, 4.5);
                need("error_container", p.on_error_container, p.error_container, 4.5);
                need("primary/surface", p.primary, p.surface, 3.0);
                for (i, bg) in p.backgrounds(dark).into_iter().enumerate() {
                    need(&format!("text/backdrop{i}"), p.on_surface, bg, 4.5);
                    need(&format!("variant/backdrop{i}"), p.on_surface_variant, bg, 4.5);
                }
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }
}
