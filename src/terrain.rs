use anyhow::Result;
use bevy::{
	asset::RenderAssetUsages,
	prelude::*,
	render::{
		mesh::{Indices, PrimitiveTopology},
		render_resource::{Extent3d, TextureDimension, TextureFormat},
	},
};
use bevy_egui::{EguiContexts, egui};
use log::{error, info};
use noise::{NoiseFn, OpenSimplex};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
	fn build(&self, app: &mut App) {
		app.insert_resource(load_settings())
			.add_systems(Startup, setup_terrain)
			.add_systems(Update, update_terrain)
			.add_systems(bevy_egui::EguiPrimaryContextPass, ui_system);
	}
}

#[derive(Resource, Serialize, Deserialize, Clone)]
struct Settings {
	terrain: TerrainConfig,
	noise: NoiseConfig,
}

impl Settings {
	fn save(&self) -> Result<()> {
		let settings_path = "terrain_settings.json";
		let json = serde_json::to_string_pretty(self)?;
		fs::write(settings_path, json)?;
		Ok(())
	}

	fn load() -> Result<Self> {
		let settings_path = "terrain_settings.json";
		if Path::new(settings_path).exists() {
			let json = fs::read_to_string(settings_path)?;
			let settings = serde_json::from_str(&json)?;
			Ok(settings)
		} else {
			Ok(Settings::default())
		}
	}
}

impl Default for Settings {
	fn default() -> Self {
		Self {
			terrain: TerrainConfig::default(),
			noise: NoiseConfig::default(),
		}
	}
}

fn load_settings() -> Settings {
	match Settings::load() {
		Ok(settings) => {
			info!("Loaded terrain settings from file");
			settings
		}
		Err(e) => {
			error!("Failed to load settings: {}. Using defaults.", e);
			Settings::default()
		}
	}
}

#[derive(Serialize, Deserialize, Clone)]
struct TerrainConfig {
	world_size: f32,
	resolution_multiplier: u32,
	height_multiplier: f32,
}

impl PartialEq for TerrainConfig {
	fn eq(&self, other: &Self) -> bool {
		self.world_size.to_bits() == other.world_size.to_bits()
			&& self.resolution_multiplier == other.resolution_multiplier
			&& self.height_multiplier.to_bits() == other.height_multiplier.to_bits()
	}
}

impl TerrainConfig {
	fn grid_size(&self) -> u32 {
		(16.0 * self.resolution_multiplier as f32) as u32
	}

	fn world_width(&self) -> f32 {
		self.world_size
	}

	fn world_length(&self) -> f32 {
		self.world_size
	}
}

impl Default for TerrainConfig {
	fn default() -> Self {
		Self {
			world_size: 15.0,
			resolution_multiplier: 8, // This gives us 64 grid cells (16 * 4)
			height_multiplier: 3.4,
		}
	}
}

#[derive(Serialize, Deserialize, Clone)]
struct NoiseConfig {
	seed: u32,
	offset_x: f32,
	offset_z: f32,
	scale: f32,
	octaves: u8,
	persistence: f32,
	lacunarity: f32,
	valley_exponent: f32,
	height_roughness: f32,
}

impl PartialEq for NoiseConfig {
	fn eq(&self, other: &Self) -> bool {
		self.seed == other.seed
			&& self.offset_x.to_bits() == other.offset_x.to_bits()
			&& self.offset_z.to_bits() == other.offset_z.to_bits()
			&& self.scale.to_bits() == other.scale.to_bits()
			&& self.octaves == other.octaves
			&& self.persistence.to_bits() == other.persistence.to_bits()
			&& self.lacunarity.to_bits() == other.lacunarity.to_bits()
			&& self.valley_exponent.to_bits() == other.valley_exponent.to_bits()
			&& self.height_roughness.to_bits() == other.height_roughness.to_bits()
	}
}

impl Default for NoiseConfig {
	fn default() -> Self {
		Self {
			seed: 0,
			offset_x: 0.0,
			offset_z: 0.0,
			scale: 3.8,
			octaves: 8,
			persistence: 0.18,
			lacunarity: 2.3,
			valley_exponent: 10.5,
			height_roughness: 1.9,
		}
	}
}

#[derive(Component)]
struct NoiseTexture;

#[derive(Component)]
struct TerrainMesh;

fn ui_system(mut contexts: EguiContexts, mut settings: ResMut<Settings>) {
	if let Ok(ctx) = contexts.ctx_mut() {
		egui::Window::new("Terrain Controls").show(ctx, |ui| {
			ui.label("Terrain Configuration:");

			ui.label("Resolution Multiplier:");
			ui.add(
				egui::Slider::new(&mut settings.terrain.resolution_multiplier, 1..=32).step_by(1.0),
			);

			ui.label(&format!(
				"Grid Size: {}x{}",
				settings.terrain.grid_size(),
				settings.terrain.grid_size()
			));

			ui.label("World Size:");
			ui.add(egui::Slider::new(&mut settings.terrain.world_size, 1.0..=20.0).step_by(0.5));

			ui.label("Height Multiplier:");
			ui.add(egui::Slider::new(
				&mut settings.terrain.height_multiplier,
				0.1..=5.0,
			));

			ui.separator();
			ui.label("Noise Parameters:");

			ui.label("Seed:");
			ui.add(egui::DragValue::new(&mut settings.noise.seed).speed(1.0));

			ui.label("Offset X:");
			ui.add(egui::Slider::new(&mut settings.noise.offset_x, -10.0..=10.0).step_by(0.1));

			ui.label("Offset Z:");
			ui.add(egui::Slider::new(&mut settings.noise.offset_z, -10.0..=10.0).step_by(0.1));

			ui.label("Scale:");
			ui.add(egui::Slider::new(&mut settings.noise.scale, 0.01..=10.0));

			ui.label("Octaves:");
			ui.add(egui::Slider::new(&mut settings.noise.octaves, 1..=8).step_by(1.0));

			ui.label("Persistence:");
			ui.add(egui::Slider::new(
				&mut settings.noise.persistence,
				0.0..=1.0,
			));

			ui.label("Lacunarity:");
			ui.add(egui::Slider::new(
				&mut settings.noise.lacunarity,
				1.01..=4.0,
			));

			ui.label("Valley Exponent:");
			ui.add(egui::Slider::new(
				&mut settings.noise.valley_exponent,
				0.0..=20.0,
			));

			ui.label("Height Roughness:");
			ui.add(egui::Slider::new(
				&mut settings.noise.height_roughness,
				0.0..=5.0,
			));

			ui.separator();
			if ui.button("Save Settings").clicked() {
				if let Err(e) = settings.save() {
					error!("Failed to save settings: {}", e);
				} else {
					info!("Settings saved successfully");
				}
			}
		});
	}
}


fn setup_terrain(
	mut commands: Commands,
	mut meshes: ResMut<Assets<Mesh>>,
	mut materials: ResMut<Assets<StandardMaterial>>,
	mut images: ResMut<Assets<Image>>,
	settings: Res<Settings>,
) {
	// Generate height map once
	let grid_size = settings.terrain.grid_size();
	let height_map = HeightMap::new(grid_size, grid_size, &settings.noise);

	let terrain_mesh = generate_mesh_from_height_map(
		&height_map,
		grid_size,
		grid_size,
		settings.terrain.world_width(),
		settings.terrain.world_length(),
		settings.terrain.height_multiplier,
	);
	let noise_texture = generate_texture_from_height_map(&height_map, grid_size, grid_size);
	let terrain_handle = meshes.add(terrain_mesh);
	let noise_handle = images.add(noise_texture);

	commands.spawn((
		Mesh3d(terrain_handle),
		MeshMaterial3d(materials.add(Color::srgb(0.3, 0.5, 0.3))),
		TerrainMesh,
	));

	commands.spawn((
		ImageNode::new(noise_handle),
		NoiseTexture,
		Node {
			justify_self: JustifySelf::End,
			align_self: AlignSelf::Start,
			width: Val::Px(256.0),
			height: Val::Px(256.0),
			padding: UiRect::all(Val::Px(10.0)),
			..default()
		},
	));
}


fn update_terrain(
	mut noise_texture_query: Query<&mut ImageNode, With<NoiseTexture>>,
	mut images: ResMut<Assets<Image>>,
	mut terrain_query: Query<&mut Mesh3d, With<TerrainMesh>>,
	mut meshes: ResMut<Assets<Mesh>>,
	settings: Res<Settings>,
) {
	// Only update if settings have changed
	if settings.is_changed() {
		let grid_size = settings.terrain.grid_size();
		let height_map = HeightMap::new(grid_size, grid_size, &settings.noise);
		if let Ok(mut image_node) = noise_texture_query.single_mut() {
			let new_texture = generate_texture_from_height_map(&height_map, grid_size, grid_size);
			let new_texture_handle = images.add(new_texture);
			*image_node = ImageNode::new(new_texture_handle);
		}
		if let Ok(mut mesh_3d) = terrain_query.single_mut() {
			let new_terrain_mesh = generate_mesh_from_height_map(
				&height_map,
				grid_size,
				grid_size,
				settings.terrain.world_width(),
				settings.terrain.world_length(),
				settings.terrain.height_multiplier,
			);

			let new_mesh_handle = meshes.add(new_terrain_mesh);
			*mesh_3d = Mesh3d(new_mesh_handle);
		}
	}
}

#[derive(Debug)]
struct HeightMap {
	width: u32,
	heights: Vec<f32>,
}

impl HeightMap {
	fn new(grid_length: u32, grid_width: u32, params: &NoiseConfig) -> Self {
		// Create noise objects for each octave once
		let mut octave_noises = Vec::new();
		for octave in 0..params.octaves {
			let octave_seed = params.seed.wrapping_add(octave as u32);
			octave_noises.push(OpenSimplex::new(octave_seed));
		}

		let mut height_map = Self {
			width: grid_width,
			heights: vec![0.0; ((grid_width + 1) * (grid_length + 1)) as usize],
		};

		// Values for normalization
		let mut min_height = f32::INFINITY;
		let mut max_height = f32::NEG_INFINITY;

		for z in 0..=grid_width {
			for x in 0..=grid_length {
				let x_pos = (x as f32 / grid_length as f32) - 0.5;
				let z_pos = (z as f32 / grid_width as f32) - 0.5;

				let height = calculate_height_at_position(x_pos, z_pos, params, &octave_noises);
				height_map.set(x, z, height);

				min_height = min_height.min(height);
				max_height = max_height.max(height);
			}
		}

		// Normalize all values to 0-1 range
		let height_range = max_height - min_height;
		if height_range > 0.0 {
			for z in 0..=grid_width {
				for x in 0..=grid_length {
					let height = height_map.get(x, z);
					let normalized_height = (height - min_height) / height_range;
					height_map.set(x, z, normalized_height);
				}
			}
		}

		height_map
	}

	fn get(&self, x: u32, z: u32) -> f32 {
		let index = (z * (self.width + 1) + x) as usize;
		self.heights[index]
	}

	fn set(&mut self, x: u32, z: u32, height: f32) {
		let index = (z * (self.width + 1) + x) as usize;
		self.heights[index] = height;
	}
}

fn calculate_height_at_position(
	x_pos: f32,
	z_pos: f32,
	params: &NoiseConfig,
	noise_octaves: &[OpenSimplex],
) -> f32 {
	let mut amplitude = 1.0_f64;
	let mut frequency = 1.0_f64;
	let mut height = 0.0_f64;
	let mut max_height = 0.0_f64;

	// Generate fractal noise using multiple octaves
	for octave in noise_octaves.iter() {
		let sample_x =
			(x_pos as f64 * params.scale as f64 * frequency) + (params.offset_x as f64 * frequency);
		let sample_z =
			(z_pos as f64 * params.scale as f64 * frequency) + (params.offset_z as f64 * frequency);

		let raw_noise_sample = octave.get([sample_x, sample_z]);
		height += raw_noise_sample * amplitude;
		max_height += amplitude;

		// Calculate height-dependent persistence
		// Convert current height to 0-1 range for the height roughness calculation
		let current_height_normalized = (height / max_height + 1.0) * 0.5;

		// Apply height roughness: higher elevations get more roughness (higher persistence)
		// The height_roughness parameter controls how much the height affects persistence
		let height_factor = 1.0 + (current_height_normalized * params.height_roughness as f64);
		let dynamic_persistence = params.persistence as f64 * height_factor;

		amplitude *= dynamic_persistence;
		frequency *= params.lacunarity as f64;
	}

	// Normalize fractal noise
	height /= max_height;
	let normalized_height = (height + 1.0) * 0.5;

	// Apply exponential curve to create valleys
	normalized_height.powf(params.valley_exponent as f64) as f32
}

fn generate_mesh_from_height_map(
	height_map: &HeightMap,
	grid_length: u32,
	grid_width: u32,
	world_length: f32,
	world_width: f32,
	height_multiplier: f32,
) -> Mesh {
	let mut positions = Vec::new();
	let mut uvs = Vec::new();
	let mut indices = Vec::new();

	let length_step = world_length / grid_length as f32;
	let width_step = world_width / grid_width as f32;

	// Generate vertices
	for z in 0..=grid_width {
		for x in 0..=grid_length {
			let x_pos = (x as f32 * length_step) - world_length / 2.0;
			let z_pos = (z as f32 * width_step) - world_width / 2.0;
			let y_pos = height_map.get(x, z) * height_multiplier;

			positions.push([x_pos, y_pos, z_pos]);
			uvs.push([x as f32 / grid_length as f32, z as f32 / grid_width as f32]);
		}
	}

	// Generate triangle indices
	for z in 0..grid_width {
		for x in 0..grid_length {
			let current = z * (grid_length + 1) + x;
			let next_x = current + 1;
			let next_z = (z + 1) * (grid_length + 1) + x;
			let next_both = next_z + 1;

			// First triangle (counter-clockwise winding)
			indices.extend_from_slice(&[current, next_z, next_x]);
			// Second triangle (counter-clockwise winding)
			indices.extend_from_slice(&[next_x, next_z, next_both]);
		}
	}

	Mesh::new(
		PrimitiveTopology::TriangleList,
		RenderAssetUsages::RENDER_WORLD,
	)
	.with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
	.with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
	.with_inserted_indices(Indices::U32(indices))
	.with_computed_normals()
}

fn generate_texture_from_height_map(
	height_map: &HeightMap,
	grid_length: u32,
	grid_width: u32,
) -> Image {
	let mut texture_data = Vec::with_capacity(((grid_length + 1) * (grid_width + 1) * 4) as usize);

	for z in 0..=grid_width {
		for x in 0..=grid_length {
			let height = height_map.get(x, z);
			let pixel_value = (height * 255.0) as u8;
			texture_data.extend_from_slice(&[pixel_value, pixel_value, pixel_value, 255]);
		}
	}

	Image::new_fill(
		Extent3d {
			width: grid_length + 1,
			height: grid_width + 1,
			depth_or_array_layers: 1,
		},
		TextureDimension::D2,
		&texture_data,
		TextureFormat::Rgba8UnormSrgb,
		RenderAssetUsages::all(),
	)
}