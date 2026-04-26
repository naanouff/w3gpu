pub mod asset_registry;
pub mod cull_pass;
pub mod error;
pub mod frame_uniforms;
pub mod gpu_context;
pub mod hdr_target;
pub mod hiz_pass;
pub mod ibl;
pub mod ibl_spec;
pub mod light_uniforms;
pub mod material_uniforms;
pub mod plugin;
pub mod post_process;
pub mod render_command;
pub mod render_state;
pub mod shadow_pass;
pub mod systems;
pub mod vertex_layout;
pub mod viewer_light_rig;

pub mod render_graph_exec;

#[cfg(not(target_arch = "wasm32"))]
pub use render_graph_exec::{
    encode_render_graph_passes_v0, run_graph_v0_checksum, run_graph_v0_checksum_with_registry,
    run_graph_v0_checksum_with_registry_pre_writes,
};
pub use render_graph_exec::{
    encode_render_graph_passes_v0_with_wgsl, encode_render_graph_passes_v0_with_wgsl_host,
    run_graph_v0_checksum_from_wgsl, run_graph_v0_checksum_with_registry_wgsl,
    run_graph_v0_checksum_with_registry_wgsl_host, validate_render_graph_exec_v0,
    NoopRenderGraphV0Host, RenderGraphExecError, RenderGraphGpuRegistry, RenderGraphV0Host,
    Texture2dGpu,
};
pub use w3drs_render_graph::{
    parse_render_graph_json, pass_ids_in_order_v0, validate_exec_v0, RenderGraphValidateError,
};

pub use asset_registry::{AssetRegistry, GpuMaterial, GpuMesh, GpuTexture, MaterialTextures};
pub use cull_pass::{CullPass, CullUniforms, CULL_STATS_SIZE, MAX_CULL_ENTITIES};
pub use error::EngineError;
pub use frame_uniforms::{FrameUniforms, IBL_FLAG_DISABLE_IRRADIANCE_DIFFUSE, SHADOW_CASCADE_COUNT};
pub use gpu_context::{GpuContext, DEPTH_FORMAT};
pub use hdr_target::{pick_hdr_main_pass_msaa, HdrTarget, HDR_FORMAT};
pub use hiz_pass::HizPass;
pub use ibl::{IblContext, PreparedIbl};
pub use ibl_spec::{from_tier_name_silent, prefiltered_mip_level_count, IblGenerationSpec};
pub use light_uniforms::LightUniforms;
pub use material_uniforms::MaterialUniforms;
pub use plugin::{App, Plugin};
pub use post_process::{BloomParams, PostProcessPass, TonemapParams};
pub use render_command::{
    build_batches, build_entity_list, derive_shadow_batches, DrawBatch, DrawEntity,
    DrawIndexedIndirectArgs, EntityCullData, RenderCommand, ShadowBatch,
};
pub use render_state::{RenderState, MAX_INSTANCES};
pub use shadow_pass::{ShadowPass, SHADOW_SIZE};
pub use systems::{camera_system, frustum_culling_system, transform_system};
pub use vertex_layout::VERTEX_BUFFER_LAYOUT;
pub use viewer_light_rig::{
    active_camera_vpc, build_frame_uniforms_for_viewer, build_frame_uniforms_for_world,
    light_uniforms_for_cascades, light_uniforms_from_viewer,
};
