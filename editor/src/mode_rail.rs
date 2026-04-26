//! Icônes 18px (sémantique [www/src/editor/modeIcons]) + pastille *w3* rail (v3-hifi, équivalent marque texte + texture logo).

use eframe::egui;
use eframe::egui::{epaint, vec2, Pos2};

use crate::v3_hifi::{c, D_BRAND, D_BRAND_2, D_BRAND_FG, D_FG_2, D_MODE_ON_RAIL};

const R2: f32 = 6.0;
const RAIL_BTN: f32 = 36.0;

/// Pastille *w3d* : carré **brand** (32×) + *w3* (maquette v3-hifi *rail-brand* en fallback sans texture).
pub fn paint_rail_brand(painter: &egui::Painter, rect: egui::Rect, _on_play_skin: bool) {
    let fill = c(D_BRAND);
    painter.rect_filled(rect, R2, fill);
    let galley = painter.layout(
        "w3".to_string(),
        epaint::FontId::proportional(11.0),
        c(D_BRAND_FG),
        rect.width(),
    );
    let t = rect.center() - galley.size() * 0.5;
    painter.galley(Pos2::new(t.x, t.y + 0.5), galley, c(D_BRAND_FG));
}

/// Dessine l’icône du mode (ident `build`, `play`, …).
pub fn paint_mode_icon(painter: &egui::Painter, center: Pos2, id: &str, stroke: epaint::Stroke) {
    let u = RAIL_BTN / 24.0; // ancre viewBox 24
    let p = |x: f32, y: f32| center + vec2((x - 12.0) * u, (y - 12.0) * u);

    match id {
        "build" => {
            let d = 8.0 * u;
            let r = egui::Rect::from_center_size(p(12.0, 12.0), 2.0 * d * vec2(1.0, 1.0));
            painter.rect_stroke(r, 2.0 * u, stroke, egui::StrokeKind::Inside);
        }
        "play" => {
            painter.add(epaint::PathShape::closed_line(
                vec![p(8.0, 5.0), p(18.0, 12.0), p(8.0, 19.0)],
                stroke,
            ));
        }
        "ship" => {
            painter.line_segment([p(4.0, 16.0), p(12.0, 8.0)], stroke);
            painter.line_segment([p(12.0, 8.0), p(20.0, 16.0)], stroke);
            painter.line_segment([p(12.0, 8.0), p(12.0, 20.0)], stroke);
        }
        "paint" => {
            painter.add(epaint::PathShape::closed_line(
                vec![p(4.0, 20.0), p(14.0, 10.0), p(19.0, 15.0)],
                stroke,
            ));
            painter.circle_stroke(p(17.0, 7.0), 1.5 * u, stroke);
        }
        "sculpt" => {
            for i in 0..8 {
                let t1 = (i as f32) * 0.2;
                let t2 = t1 * std::f32::consts::PI;
                let x1 = 4.0 + t1 * 12.0;
                let y1 = 4.0 + 8.0 * t2.sin() + 2.0;
                if i < 7 {
                    let t3 = ((i + 1) as f32) * 0.2;
                    let t4 = t3 * std::f32::consts::PI;
                    let x2 = 4.0 + t3 * 12.0;
                    let y2 = 4.0 + 8.0 * t4.sin() + 2.0;
                    painter.line_segment([p(x1, y1), p(x2, y2)], stroke);
                }
            }
        }
        "logic" => {
            painter.circle_stroke(p(6.0, 6.0), 1.5 * u, stroke);
            painter.circle_stroke(p(18.0, 18.0), 1.5 * u, stroke);
            painter.line_segment([p(7.0, 6.0), p(17.0, 18.0)], stroke);
        }
        "light" => {
            painter.line_segment([p(12.0, 3.0), p(12.0, 6.0)], stroke);
            painter.line_segment([p(12.0, 18.0), p(12.0, 21.0)], stroke);
            painter.line_segment([p(3.0, 12.0), p(6.0, 12.0)], stroke);
            painter.line_segment([p(18.0, 12.0), p(21.0, 12.0)], stroke);
            painter.circle_stroke(p(12.0, 12.0), 4.0 * u, stroke);
        }
        "animate" => {
            for i in 0..7 {
                let t1 = (i as f32) * 0.18;
                let t2 = (i as f32 + 1.0) * 0.18;
                let x1 = 4.0 + t1 * 14.0;
                let x2 = 4.0 + t2 * 14.0;
                let y1 = 18.0 - 4.0 * t1;
                let y2 = 18.0 - 4.0 * t2;
                painter.line_segment([p(x1, y1), p(x2, y2)], stroke);
            }
        }
        _ => {
            painter.circle_filled(p(12.0, 12.0), 3.0 * u, stroke.color);
        }
    }
}

/// Stroke 1.6, couleurs *hi-fi* (actif = brand-2, play rail = #6b6359).
pub fn mode_stroke_sel(selected: bool, play_rail: bool) -> epaint::Stroke {
    let col = if play_rail {
        if selected {
            c(D_BRAND_2)
        } else {
            c(D_MODE_ON_RAIL)
        }
    } else if selected {
        c(D_BRAND_2)
    } else {
        c(D_FG_2)
    };
    epaint::Stroke::new(1.6, col)
}

/// Même style que *web* : actif = fond *brand_soft* + anneau *brand* (inset).
pub fn draw_mode_button(
    ui: &mut egui::Ui,
    i: usize,
    m: &crate::editor_config::ModeEntry,
    active: bool,
    play: bool,
) -> egui::Response {
    let size = RAIL_BTN;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click());
    let id = m.id.as_str();
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        if active {
            if play {
                painter.rect_filled(
                    rect,
                    4.0,
                    c([0xf1, 0xed, 0xe3]).linear_multiply(0.1),
                );
            } else {
                painter.rect_filled(rect, 4.0, c([0x3a, 0x24, 0x18])); // brand_soft
            }
            painter.rect_stroke(
                rect,
                4.0,
                epaint::Stroke::new(1.0, c(D_BRAND)),
                egui::StrokeKind::Inside,
            );
        } else if response.hovered() {
            painter.rect_filled(rect, 4.0, c([0x2a, 0x27, 0x24])); // hover
        }
        if !m.key_hint.is_empty() {
            let kh = m.key_hint.as_str();
            let t = if kh == "␣" { "␣" } else { kh };
            let gal = painter.layout(
                t.to_string(),
                epaint::FontId::proportional(6.0),
                if play && !active {
                    c(D_MODE_ON_RAIL)
                } else {
                    c([0x59, 0x53, 0x48])
                },
                32.0,
            );
            let gpos = rect.right_bottom() - vec2(2.0, 1.0) - gal.size();
            painter.galley(
                gpos,
                gal,
                if play && !active {
                    c(D_MODE_ON_RAIL)
                } else {
                    c([0x59, 0x53, 0x48])
                },
            );
        }
        let center = rect.center();
        let st = mode_stroke_sel(active, play);
        paint_mode_icon(painter, center, id, st);
    }
    let _ = i;
    response
}

/// Constante *taille* bouton rail (points).
pub fn rail_button_size() -> f32 {
    RAIL_BTN
}
