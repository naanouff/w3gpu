//! Marque rail : [docs/design/w3d_logo.svg] — raster resvg (alignement maquette v3-hifi, pas de « w3 » seul quand le rendu est dispo).
//!
//! Le SVG est embarqué en `include_str!` pour rester reproductible sans chemins disque.

use eframe::egui;

const W3D_LOGO_SVG: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../docs/design/w3d_logo.svg"));

/// Rasterise le logo en carré `px×px` (mise à l’échelle *contain* via `usvg` / resvg).
pub fn rasterize_w3d_brand_image(px: u32) -> Option<egui::ColorImage> {
    let opt = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_str(W3D_LOGO_SVG, &opt).ok()?;
    let src = tree.size().to_int_size();
    let target = resvg::tiny_skia::IntSize::from_wh(px, px)?;
    let out = src.scale_to(target);
    let mut pixmap = resvg::tiny_skia::Pixmap::new(out.width(), out.height())?;
    let s1 = src.to_size();
    let s2 = out.to_size();
    let t = resvg::tiny_skia::Transform::from_scale(
        s2.width() / s1.width(),
        s2.height() / s1.height(),
    );
    resvg::render(&tree, t, &mut pixmap.as_mut());
    Some(egui::ColorImage::from_rgba_premultiplied(
        [out.width() as usize, out.height() as usize],
        pixmap.data(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn w3d_logo_fixture_renders_non_empty_square() {
        let img = rasterize_w3d_brand_image(32).expect("w3d_logo.svg should parse and render");
        assert_eq!(img.size, [32, 32]);
        let active = img
            .pixels
            .iter()
            .any(|p| p.r() | p.g() | p.b() | p.a() > 0);
        assert!(active, "raster is fully transparent/empty");
    }
}
