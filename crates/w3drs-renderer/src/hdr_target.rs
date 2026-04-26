use wgpu::TextureFormat;

pub const HDR_FORMAT: TextureFormat = TextureFormat::Rgba16Float;

/// Choisit un facteur MSAA (4, 2 ou 1) pour le rendu HDR principal, selon l’adaptateur.
pub fn pick_hdr_main_pass_msaa(adapter: &wgpu::Adapter) -> u32 {
    let hdr_flags = adapter.get_texture_format_features(HDR_FORMAT).flags;

    if !hdr_flags.contains(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_RESOLVE) {
        return 1;
    }
    // The depth attachment (Depth32Float) must share the same sample_count as the colour
    // attachment. Without TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES the WebGPU spec only
    // guarantees [1, 4] for Depth32Float, so we cap here rather than at runtime.
    for &c in &[4u32, 2u32] {
        if hdr_flags.sample_count_supported(c) {
            return c;
        }
    }
    1
}

/// Cible HDR résolue (échantillonnée par le post-process) + option MSAA pour le pass PBR.
pub struct HdrTarget {
    /// Texture **non** multisamplée : resolve + binding tonemap / bloom.
    pub color: wgpu::Texture,
    pub view: wgpu::TextureView,
    /// Couleur HDR multisamplée (≥2) ; `None` si `samples == 1`.
    pub msaa_color: Option<wgpu::Texture>,
    pub msaa_view: Option<wgpu::TextureView>,
    pub samples: u32,
    pub width: u32,
    pub height: u32,
}

impl HdrTarget {
    pub fn new(device: &wgpu::Device, width: u32, height: u32, samples: u32) -> Self {
        let width = width.max(1);
        let height = height.max(1);
        let samples = samples.max(1);

        let color = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("hdr_resolve"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: HDR_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = color.create_view(&wgpu::TextureViewDescriptor::default());

        let (msaa_color, msaa_view) = if samples > 1 {
            let t = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("hdr_msaa"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format: HDR_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let v = t.create_view(&wgpu::TextureViewDescriptor::default());
            (Some(t), Some(v))
        } else {
            (None, None)
        };

        Self {
            color,
            view,
            msaa_color,
            msaa_view,
            samples,
            width,
            height,
        }
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if self.width == width && self.height == height {
            return;
        }
        *self = Self::new(device, width, height, self.samples);
    }

    /// Vue attachée au pass PBR (MSAA ou resolve).
    #[inline]
    pub fn main_color_attachment(&self) -> &wgpu::TextureView {
        self.msaa_view.as_ref().unwrap_or(&self.view)
    }

    /// Cible de resolve vers la texture HDR 1×MS (tonemap / bloom).
    #[inline]
    pub fn resolve_target(&self) -> Option<&wgpu::TextureView> {
        if self.samples > 1 {
            Some(&self.view)
        } else {
            None
        }
    }

    /// Attachement couleur du pass PBR : texture MSAA + resolve optionnel vers [`Self::view`].
    pub fn main_pass_color_attachment(
        &self,
        clear: wgpu::Color,
    ) -> wgpu::RenderPassColorAttachment<'_> {
        wgpu::RenderPassColorAttachment {
            view: self.main_color_attachment(),
            depth_slice: None,
            resolve_target: self.resolve_target(),
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(clear),
                store: if self.samples > 1 {
                    wgpu::StoreOp::Discard
                } else {
                    wgpu::StoreOp::Store
                },
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{pick_hdr_main_pass_msaa, HdrTarget};

    /// `pick_hdr_main_pass_msaa` must return 1, 2, 4, or 8 (never 0 or other).
    #[test]
    fn pick_msaa_returns_valid_count() {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: None,
            ..Default::default()
        }))
        .expect("adapter");
        let count = pick_hdr_main_pass_msaa(&adapter);
        assert!(
            matches!(count, 1 | 2 | 4 | 8),
            "unexpected MSAA count {count}"
        );
    }

    #[test]
    fn hdr_target_msaa1_has_no_msaa_views() {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: None,
            ..Default::default()
        }))
        .expect("adapter");
        let (device, _queue) =
            pollster::block_on(adapter.request_device(&Default::default())).expect("device");
        let t = HdrTarget::new(&device, 16, 16, 1);
        assert_eq!(t.samples, 1);
        assert!(t.msaa_view.is_none());
        assert!(std::ptr::eq(t.main_color_attachment(), &t.view));
        assert!(t.resolve_target().is_none());
    }
}
