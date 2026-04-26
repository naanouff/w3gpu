//! Constantes de couleur alignées [docs/design/v3-hifi.css] (sujet *html.dark* / *:root* light).
#![allow(clippy::too_many_arguments)]

use eframe::egui;

// --- Dark (Zed warm gray) — mêmes codes que la maquette v3-hifi. ---
pub const D_BG_APP: [u8; 3] = [0x16, 0x15, 0x13];
pub const D_BG_CHROME: [u8; 3] = [0x1c, 0x1b, 0x18];
pub const D_BG_PANEL: [u8; 3] = [0x1f, 0x1d, 0x1a];
pub const D_BG_VIEWPORT: [u8; 3] = [0x10, 0x0f, 0x0d];
pub const D_LINE: [u8; 3] = [0x2e, 0x2b, 0x27];
pub const D_FG: [u8; 3] = [0xe8, 0xe2, 0xd5];
pub const D_FG_STRONG: [u8; 3] = [0xf5, 0xef, 0xde];
pub const D_FG_SOFT: [u8; 3] = [0x8b, 0x83, 0x77];
pub const D_FG_2: [u8; 3] = [0xc5, 0xbf, 0xb1];
pub const D_FG_FAINT: [u8; 3] = [0x59, 0x53, 0x48];
pub const D_BRAND: [u8; 3] = [0xe0, 0x85, 0x62];
pub const D_BRAND_2: [u8; 3] = [0xc9, 0x6a, 0x45];
pub const D_BRAND_FG: [u8; 3] = [0x1a, 0x0e, 0x08];
pub const D_BRAND_SOFT: [u8; 3] = [0x3a, 0x24, 0x18];
pub const D_SEL: [u8; 3] = [0x3a, 0x2a, 0x1c];
pub const D_HOVER: [u8; 3] = [0x2a, 0x27, 0x24];
// Play (v3-hifi *v5* / rail)
pub const D_PLAY_RAIL: [u8; 3] = [0x0d, 0x0c, 0x0a];
pub const D_PLAY_LINE: [u8; 3] = [0x1a, 0x17, 0x14];
pub const D_MODE_ON_RAIL: [u8; 3] = [0x6b, 0x63, 0x59];
pub const D_MODE_ON_BRAND: [u8; 3] = [0xe0, 0x85, 0x62];

// --- Light (warm) ---
pub const L_BG_APP: [u8; 3] = [0xfa, 0xf8, 0xf5];
pub const L_BG_CHROME: [u8; 3] = [0xf3, 0xef, 0xe9];
pub const L_LINE: [u8; 3] = [0xdd, 0xd5, 0xc6];
pub const L_FG: [u8; 3] = [0x1f, 0x1b, 0x16];
pub const L_BRAND: [u8; 3] = [0xd9, 0x77, 0x57];
pub const L_BRAND_2: [u8; 3] = [0xc4, 0x5a, 0x36];
pub const L_BRAND_SOFT: [u8; 3] = [0xf6, 0xe3, 0xd8];
pub const L_FG_SOFT: [u8; 3] = [0x7d, 0x75, 0x69];
pub const L_SEL: [u8; 3] = [0xf6, 0xdc, 0xc3];
pub const L_HOVER: [u8; 3] = [0xef, 0xea, 0xe1];

/// RGB triplet (maquette) → [Color32] pour peinture egui.
pub const fn c(rgb: [u8; 3]) -> egui::Color32 {
    egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}

/// Hi-fi *dark* (html.dark) — stage / rail / outliner / inspector.
pub fn color_dark() -> HifiDark {
    HifiDark {
        bg_app: c(D_BG_APP),
        bg_chrome: c(D_BG_CHROME),
        bg_panel: c(D_BG_PANEL),
        bg_viewport: c(D_BG_VIEWPORT),
        line: c(D_LINE),
        fg: c(D_FG),
        fg_strong: c(D_FG_STRONG),
        fg_2: c(D_FG_2),
        fg_soft: c(D_FG_SOFT),
        fg_faint: c(D_FG_FAINT),
        brand: c(D_BRAND),
        brand_2: c(D_BRAND_2),
        brand_fg: c(D_BRAND_FG),
        brand_soft: c(D_BRAND_SOFT),
        selection: c(D_SEL),
        hover: c(D_HOVER),
    }
}

pub fn color_light() -> HifiLight {
    HifiLight {
        bg_app: c(L_BG_APP),
        bg_chrome: c(L_BG_CHROME),
        line: c(L_LINE),
        fg: c(L_FG),
        brand: c(L_BRAND),
        brand_2: c(L_BRAND_2),
        brand_soft: c(L_BRAND_SOFT),
        fg_soft: c(L_FG_SOFT),
        selection: c(L_SEL),
        hover: c(L_HOVER),
    }
}

#[derive(Clone, Copy)]
pub struct HifiDark {
    pub bg_app: egui::Color32,
    pub bg_chrome: egui::Color32,
    pub bg_panel: egui::Color32,
    pub bg_viewport: egui::Color32,
    pub line: egui::Color32,
    pub fg: egui::Color32,
    pub fg_strong: egui::Color32,
    pub fg_2: egui::Color32,
    pub fg_soft: egui::Color32,
    pub fg_faint: egui::Color32,
    pub brand: egui::Color32,
    pub brand_2: egui::Color32,
    pub brand_fg: egui::Color32,
    pub brand_soft: egui::Color32,
    pub selection: egui::Color32,
    pub hover: egui::Color32,
}

#[derive(Clone, Copy)]
pub struct HifiLight {
    pub bg_app: egui::Color32,
    pub bg_chrome: egui::Color32,
    pub line: egui::Color32,
    pub fg: egui::Color32,
    pub brand: egui::Color32,
    pub brand_2: egui::Color32,
    pub brand_soft: egui::Color32,
    pub fg_soft: egui::Color32,
    pub selection: egui::Color32,
    pub hover: egui::Color32,
}

/// Applique *Visuals* egui (widgets + panneaux) d’après la fiche v3-hifi.
pub fn apply_egui_visuals(ctx: &egui::Context, appearance: crate::editor_config::Appearance) {
    use crate::editor_config::Appearance;
    let mut v = match appearance {
        Appearance::Dark => egui::Visuals::dark(),
        Appearance::Light => egui::Visuals::light(),
    };
    match appearance {
        Appearance::Dark => {
            let d = color_dark();
            v.window_fill = d.bg_app;
            v.panel_fill = d.bg_chrome;
            v.extreme_bg_color = c(D_PLAY_RAIL);
            v.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, d.fg_2);
            v.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, d.fg);
            v.widgets.inactive.weak_bg_fill = d.bg_panel;
            v.widgets.hovered.weak_bg_fill = d.hover;
            v.widgets.active.weak_bg_fill = d.brand_soft;
            v.widgets.active.fg_stroke = egui::Stroke::new(1.0, d.brand_2);
            v.selection.bg_fill = d.selection;
            v.selection.stroke = egui::Stroke::new(1.0, d.brand);
            v.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, d.fg_2);
            v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, d.line);
            v.hyperlink_color = d.brand_2;
        }
        Appearance::Light => {
            let l = color_light();
            v.window_fill = l.bg_app;
            v.panel_fill = l.bg_chrome;
            v.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, l.fg_soft);
            v.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, l.fg);
            v.selection.bg_fill = l.selection;
            v.selection.stroke = egui::Stroke::new(1.0, l.brand_2);
            v.widgets.hovered.weak_bg_fill = l.hover;
        }
    }
    ctx.set_visuals(v);
}

/// Couleurs rail (mode *normal* : hi-fi) vs *play* (skin v5 assombri).
pub fn rail_skin(play: bool) -> (egui::Color32, egui::Color32, bool) {
    if play {
        (c(D_PLAY_RAIL), c(D_PLAY_LINE), true)
    } else {
        let d = color_dark();
        (d.bg_chrome, d.line, false)
    }
}

/// Couleurs tête d’espace (Play : fond rail + titre brand).
pub fn head_skin(play: bool) -> (egui::Color32, egui::Color32) {
    if play {
        (c(D_PLAY_RAIL), c(D_MODE_ON_BRAND))
    } else {
        let d = color_dark();
        (d.bg_chrome, d.fg_strong)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_brand_matches_css_hex() {
        let d = D_BRAND;
        assert_eq!(d, [0xe0, 0x85, 0x62]);
        let t = c(D_BRAND);
        assert_eq!([t.r(), t.g(), t.b()], d);
    }
}
