use bevy::{
	pbr::Material,
	prelude::*,
	render::{
		alpha::AlphaMode,
		render_resource::{AsBindGroup, ShaderType},
	},
	shader::ShaderRef,
};

/// Standalone material that adds contour lines to terrain based on height
#[derive(AsBindGroup, Asset, TypePath, Debug, Clone)]
pub struct ContourMaterial {
	/// Placeholder texture to satisfy PBR pipeline layout requirements
	/// Using high binding numbers to avoid conflicts with PBR reserved bindings
	#[texture(100, dimension = "2d")]
	#[sampler(101)]
	pub placeholder_texture: Handle<Image>,

	/// All material properties and settings combined into a single uniform
	#[uniform(102)]
	pub settings: ContourSettings,
}

#[derive(ShaderType, Clone, Debug)]
pub struct ContourSettings {
	/// Metallic factor (0.0 = non-metallic, 1.0 = metallic)
	pub metallic: f32,
	/// Perceptual roughness (0.0 = smooth, 1.0 = rough)
	pub perceptual_roughness: f32,
	/// Height interval between contour lines in world units
	pub interval: f32,
	/// Color of the contour lines (RGB)
	pub line_color: Vec3,
	/// Thickness of the contour lines (affects smoothstep falloff)
	pub line_thickness: f32,
	/// Whether contour lines are enabled (1.0 = enabled, 0.0 = disabled)
	pub enabled: u32,
}

impl Default for ContourMaterial {
	fn default() -> Self {
		Self {
			placeholder_texture: Handle::default(),
			settings: ContourSettings {
				line_color: Vec3::ONE,
				metallic: 0.0,
				perceptual_roughness: 0.5,
				interval: 20.0,
				line_thickness: 0.02,
				enabled: false as u32,
			},
		}
	}
}

impl Material for ContourMaterial {
	fn fragment_shader() -> ShaderRef {
		"shaders/contour_lines.wgsl".into()
	}

	fn alpha_mode(&self) -> AlphaMode {
		AlphaMode::Blend
	}
}

/// Resource to store contour line state
#[derive(Resource)]
pub struct ContourState {
	pub enabled: bool,
	pub interval: f32,
	pub line_color: [f32; 3],
	pub thickness: f32,
	pub needs_update: bool,
}

impl Default for ContourState {
	fn default() -> Self {
		Self {
			enabled: false,
			interval: 20.0,
			line_color: [1.0, 1.0, 1.0],
			thickness: 0.02,
			needs_update: false,
		}
	}
}
