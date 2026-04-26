//! Viewport 3D : PBR (lib `khronos_pbr`) + texture enregistrée auprès d’`egui-wgpu`.
//!
//! Cible PBR : `Rgba8Unorm` (format attendu par `Renderer::register_native_texture`).

use eframe::egui;
use w3drs_input::{InputFrame, PointerDelta};
use w3drs_renderer::pick_hdr_main_pass_msaa;

const OUT_FMT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// Saisie orbit pour la zone viewport (même frame que le rendu).
fn input_frame_viewport(ctx: &egui::Context, vrect: egui::Rect) -> InputFrame {
    ctx.input(|i| {
        let inside = i
            .pointer
            .interact_pos()
            .map_or(false, |p| vrect.contains(p));
        if !inside {
            return InputFrame::default();
        }
        let d = i.pointer.delta();
        let scroll = i.smooth_scroll_delta;
        let wheel = scroll.y / 12.0;
        let primary = if i.pointer.button_down(egui::PointerButton::Primary) {
            PointerDelta::new(d.x, d.y)
        } else {
            PointerDelta::default()
        };
        let sec = if i.pointer.button_down(egui::PointerButton::Secondary) {
            PointerDelta::new(d.x, d.y)
        } else {
            PointerDelta::default()
        };
        let mid = if i.pointer.button_down(egui::PointerButton::Middle) {
            PointerDelta::new(d.x, d.y)
        } else {
            PointerDelta::default()
        };
        InputFrame {
            primary_drag: primary,
            secondary_drag: sec,
            middle_drag: mid,
            wheel_lines: wheel,
            ..Default::default()
        }
    })
}

/// Exposé pour tests : construit le même enregistrement de champs.
pub fn input_frame_viewport_test_rect(
    interact_pos: Option<egui::Pos2>,
    vrect: egui::Rect,
    delta: egui::Vec2,
    primary_down: bool,
    smooth_scroll_y: f32,
) -> InputFrame {
    let inside = interact_pos.map_or(false, |p| vrect.contains(p));
    if !inside {
        return InputFrame::default();
    }
    let wheel = smooth_scroll_y / 12.0;
    let primary = if primary_down {
        PointerDelta::new(delta.x, delta.y)
    } else {
        PointerDelta::default()
    };
    InputFrame {
        primary_drag: primary,
        wheel_lines: wheel,
        ..Default::default()
    }
}

/// État 3D.
pub struct Viewport3dPbr {
    state: Option<khronos_pbr::State>,
    color_tex: Option<wgpu::Texture>,
    egui_tex: Option<egui::TextureId>,
    wh_px: (u32, u32),
}

impl Viewport3dPbr {
    pub fn new() -> Self {
        Self {
            state: None,
            color_tex: None,
            egui_tex: None,
            wh_px: (0, 0),
        }
    }

    fn init_if_needed(&mut self, rs: &egui_wgpu::RenderState) {
        if self.state.is_some() {
            return;
        }
        let dev = &rs.device;
        let q = &rs.queue;
        let fmt = OUT_FMT;
        let ms = pick_hdr_main_pass_msaa(&rs.adapter);
        self.state = Some(khronos_pbr::State::new_egui_host(
            dev.clone(),
            q.clone(),
            2,
            2,
            fmt,
            ms,
        ));
    }

    fn ensure_color(
        &mut self,
        rs: &egui_wgpu::RenderState,
        w: u32,
        h: u32,
    ) {
        if self.state.is_none() {
            return;
        }
        if w == 0 || h == 0 {
            return;
        }
        if self.color_tex.is_some() && self.wh_px == (w, h) {
            return;
        }
        self.wh_px = (w, h);
        {
            let s = self.state.as_mut().expect("set when color alloc");
            s.resize(w, h);
        }
        self.color_tex = Some(rs.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("editor pbr view"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: OUT_FMT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        }));
        let v = self
            .color_tex
            .as_ref()
            .expect("just created")
            .create_view(&Default::default());
        let dev = &rs.device;
        let mut gr = rs.renderer.write();
        if let Some(id) = self.egui_tex {
            gr.update_egui_texture_from_wgpu_texture(dev, &v, wgpu::FilterMode::Linear, id);
        } else {
            let id = gr.register_native_texture(dev, &v, wgpu::FilterMode::Linear);
            self.egui_tex = Some(id);
        }
    }

    /// Rendu PBR dans le rectangle. Retourne `false` si wgpu n’est pas l’hôte.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        eframe: &eframe::Frame,
        vrect: egui::Rect,
    ) -> bool {
        let Some(rs) = eframe.wgpu_render_state() else {
            return false;
        };
        self.init_if_needed(rs);
        let ppp = ctx.pixels_per_point();
        let w_px = ((vrect.width() * ppp) as u32).max(1);
        let h_px = ((vrect.height() * ppp) as u32).max(1);
        self.ensure_color(rs, w_px, h_px);
        if let (Some(s), Some(tex), Some(tid)) = (
            self.state.as_mut(),
            self.color_tex.as_ref(),
            self.egui_tex,
        ) {
            let in_fr = input_frame_viewport(ctx, vrect);
            s.orbit.apply_input(&in_fr);
            let v = tex.create_view(&Default::default());
            s.tick_egui(&v);
            {
                let mut gr = rs.renderer.write();
                gr.update_egui_texture_from_wgpu_texture(
                    &rs.device,
                    &v,
                    wgpu::FilterMode::Linear,
                    tid,
                );
            }
            let _ = ui.scope_builder(egui::UiBuilder::new().max_rect(vrect), |ui| {
                let r = ui.max_rect();
                let sz = r.size();
                let _r = ui.add(
                    egui::Image::new(egui::load::SizedTexture::new(tid, sz))
                        .corner_radius(6.0)
                        .fit_to_original_size(1.0),
                );
                ui.painter().rect_stroke(
                    r,
                    6.0,
                    egui::epaint::Stroke::new(
                        1.0,
                        egui::Color32::from_rgba_premultiplied(80, 80, 90, 200),
                    ),
                    egui::StrokeKind::Inside,
                );
            });
            return true;
        }
        false
    }
}

impl Default for Viewport3dPbr {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use eframe::egui::{pos2, Rect, Vec2};

    use super::input_frame_viewport_test_rect;

    #[test]
    fn test_rect_empty_when_outside() {
        let r = Rect::from_min_size(pos2(10.0, 10.0), Vec2::splat(100.0));
        let f = input_frame_viewport_test_rect(Some(pos2(0.0, 0.0)), r, Vec2::new(1.0, 2.0), true, 0.0);
        assert_eq!(f.primary_drag.dx, 0.0);
        assert_eq!(f.wheel_lines, 0.0);
    }

    #[test]
    fn test_rect_drag_inside() {
        let r = Rect::from_min_size(pos2(0.0, 0.0), Vec2::splat(200.0));
        let f = input_frame_viewport_test_rect(Some(pos2(10.0, 10.0)), r, Vec2::new(2.0, 3.0), true, 12.0);
        assert!((f.primary_drag.dx - 2.0).abs() < 0.01);
        assert!((f.primary_drag.dy - 3.0).abs() < 0.01);
        assert!((f.wheel_lines - 1.0).abs() < 0.01);
    }
}
