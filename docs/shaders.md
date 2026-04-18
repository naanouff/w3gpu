# Shaders & Bind Groups

## Bind group layout — PBR main pass

```
group(0) binding(0)  FrameUniforms          uniform  VERTEX | FRAGMENT
group(1) binding(0)  ObjectUniforms         uniform  VERTEX   (dynamic offset, 256-byte aligned)
group(2) binding(0)  MaterialUniforms       uniform  FRAGMENT
group(2) binding(1)  albedo_tex             texture_2d<f32>  FRAGMENT  (sRGB)
group(2) binding(2)  normal_tex             texture_2d<f32>  FRAGMENT  (linear, tangent-space)
group(2) binding(3)  mr_tex                 texture_2d<f32>  FRAGMENT  (linear, G=rough B=metal)
group(2) binding(4)  emissive_tex           texture_2d<f32>  FRAGMENT  (sRGB)
group(2) binding(5)  mat_sampler            sampler(Filtering)
group(3) binding(0)  irradiance_map         texture_cube<f32>  FRAGMENT
group(3) binding(1)  prefiltered_map        texture_cube<f32>  FRAGMENT  (5 mip levels)
group(3) binding(2)  brdf_lut               texture_2d<f32>  FRAGMENT  (rg16float, NdotV × roughness)
group(3) binding(3)  ibl_sampler            sampler(Filtering)
group(4) binding(0)  LightUniforms          uniform  FRAGMENT           ← Phase 3a
group(4) binding(1)  shadow_map             texture_depth_2d   FRAGMENT ← Phase 3a
group(4) binding(2)  shadow_sampler         sampler(Comparison)        ← Phase 3a
```

## Bind group layout — Shadow depth pass

```
group(0) binding(0)  LightUniforms          uniform  VERTEX
group(1) binding(0)  ObjectUniforms         uniform  VERTEX  (dynamic offset)
```

---

## Structs GPU (Rust ↔ WGSL alignment)

### Règle critique WGSL std140

`vec3<f32>` en WGSL a une alignement de **16 bytes** (pas 12). Un struct contenant un `vec3<f32>` subit un padding implicite en fin. Toujours utiliser des champs `f32` individuels pour le padding plutôt que `vec3<f32>`.

### FrameUniforms (272 bytes)

```rust
// Rust
pub struct FrameUniforms {
    pub projection:          [[f32; 4]; 4],   // offset   0, size 64
    pub view:                [[f32; 4]; 4],   // offset  64, size 64
    pub inv_view_projection: [[f32; 4]; 4],   // offset 128, size 64
    pub camera_position:     [f32; 3],        // offset 192, size 12
    pub _pad0:               f32,             // offset 204, size  4
    pub light_direction:     [f32; 3],        // offset 208, size 12
    pub _pad1:               f32,             // offset 220, size  4
    pub light_color:         [f32; 3],        // offset 224, size 12
    pub ambient_intensity:   f32,             // offset 236, size  4
    pub total_time:          f32,             // offset 240, size  4
    pub _pad2:               [f32; 3],        // offset 244, size 12
}                                             // total: 256 bytes
```

```wgsl
// WGSL — champs de padding nommés individuellement (pas vec3<f32>)
struct FrameUniforms {
    projection:          mat4x4<f32>,
    view:                mat4x4<f32>,
    inv_view_projection: mat4x4<f32>,
    camera_position:     vec3<f32>,   _pad0:  f32,
    light_direction:     vec3<f32>,   _pad1:  f32,
    light_color:         vec3<f32>,   ambient_intensity: f32,
    total_time:          f32,
    _pad2a: f32,  _pad2b: f32,  _pad2c: f32,
}
```

### ObjectUniforms (64 bytes, dynamic offset = 256)

```rust
pub struct ObjectUniforms { pub world: [[f32; 4]; 4] }
pub const OBJECT_ALIGN: u64 = 256;
pub const MAX_OBJECTS:  u64 = 1024;
```

### MaterialUniforms (48 bytes)

```rust
pub struct MaterialUniforms {
    pub albedo:    [f32; 4],   // offset  0
    pub emissive:  [f32; 4],   // offset 16
    pub metallic:  f32,        // offset 32
    pub roughness: f32,        // offset 36
    pub _pad:      [f32; 2],   // offset 40
}                              // total: 48
```

### LightUniforms (80 bytes)

```rust
pub struct LightUniforms {
    pub view_proj:    [[f32; 4]; 4],  // offset  0, size 64
    pub shadow_bias:  f32,            // offset 64
    pub _pad:         [f32; 3],       // offset 68
}                                     // total: 80
```

---

## Vertex layout (80 bytes = 20 × f32)

| Location | Attribut | Type | Offset |
|---|---|---|---|
| 0 | position | vec3<f32> | 0 |
| 1 | uv0 | vec2<f32> | 12 |
| 2 | uv1 | vec2<f32> | 20 |
| 3 | normal | vec3<f32> | 28 |
| 4 | tangent | vec3<f32> | 40 |
| 5 | bitangent | vec3<f32> | 52 |
| 6 | color | vec4<f32> | 64 |

---

## Textures — conventions de format

| Slot | Format upload | sRGB flag | Raison |
|---|---|---|---|
| albedo | RGBA8 | **true** | données sRGB → correction gamma par wgpu |
| normal | RGBA8 | **false** | données linéaires (tangent-space normals) |
| metallic-roughness | RGBA8 | **false** | données linéaires (G=rough, B=metal) |
| emissive | RGBA8 | **true** | données sRGB |
| shadow_map | Depth32Float | — | depth-only, comparaison PCF |
| irradiance | Rgba16Float | — | HDR, filterable en WebGPU natif |
| prefiltered | Rgba16Float | — | 5 mip levels |
| BRDF LUT | Rg16Float | — | scale + bias pour split-sum |

---

## IBL — Split-sum (Karis 2013)

```
L_ambient = L_diffuse + L_specular

L_diffuse  = kD * irradiance(N) * albedo
L_specular = prefiltered(R, roughness) * (F * brdf.x + brdf.y)

irradiance(N)         = ∫ L(ωi) cos(θ) dωi  / π   (convolution hémisphère)
prefiltered(R, rough) = ∫ L(ωi) * D(ωh, rough) * G dωi  (importance sampling GGX)
brdf(NdotV, rough)    = ∫ (f(ωi, ωo) / F0) dωi  → [scale_f0, bias_f0]
```

Géométrie IBL : `k = roughness² / 2` (differ de la version directe où `k = (rough+1)²/8`)
