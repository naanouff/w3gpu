use wgpu::TextureFormat;

pub const HDR_FORMAT: TextureFormat = TextureFormat::Rgba16Float;

pub struct HdrTarget {
    pub texture:     wgpu::Texture,
    pub view:        wgpu::TextureView,
    pub width:       u32,
    pub height:      u32,
}

impl HdrTarget {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label:           Some("hdr_target"),
            size:            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count:    1,
            dimension:       wgpu::TextureDimension::D2,
            format:          HDR_FORMAT,
            usage:           wgpu::TextureUsages::RENDER_ATTACHMENT
                           | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats:    &[],
        });
        let view = texture.create_view(&Default::default());
        Self { texture, view, width, height }
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if self.width != width || self.height != height {
            *self = Self::new(device, width, height);
        }
    }
}
