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
pub struct Settings {
	pub terrain: TerrainConfig,
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
pub struct TerrainConfig {
	pub base_grid_resolution: u32,
	pub aspect_x: u32,
	pub aspect_z: u32,
	pub base_world_size: f32,
	pub height_multiplier: f32,
}

impl PartialEq for TerrainConfig {
	fn eq(&self, other: &Self) -> bool {
		self.base_grid_resolution == other.base_grid_resolution
			&& self.aspect_x == other.aspect_x
			&& self.aspect_z == other.aspect_z
			&& self.base_world_size.to_bits() == other.base_world_size.to_bits()
			&& self.height_multiplier.to_bits() == other.height_multiplier.to_bits()
	}
}

impl TerrainConfig {
	pub fn grid_x(&self) -> u32 {
		self.base_grid_resolution * self.aspect_x
	}
	pub fn grid_z(&self) -> u32 {
		self.base_grid_resolution * self.aspect_z
	}
	pub fn world_x(&self) -> f32 {
		self.base_world_size * self.aspect_x as f32
	}
	pub fn world_z(&self) -> f32 {
		self.base_world_size * self.aspect_z as f32
	}
}

impl Default for TerrainConfig {
	fn default() -> Self {
		Self {
			base_grid_resolution: 8,
			aspect_x: 1,
			aspect_z: 1,
			base_world_size: 1000.0,
			height_multiplier: 0.5,
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

			ui.label("Base Grid Resolution:");
			ui.add(
				egui::Slider::new(&mut settings.terrain.base_grid_resolution, 1..=128).step_by(1.0),
			);

			ui.label("Aspect Ratio X:");
			ui.add(egui::Slider::new(&mut settings.terrain.aspect_x, 1..=8).step_by(1.0));

			ui.label("Aspect Ratio Z:");
			ui.add(egui::Slider::new(&mut settings.terrain.aspect_z, 1..=8).step_by(1.0));

			ui.label(&format!(
				"Grid Size: {}x{}",
				settings.terrain.grid_x(),
				settings.terrain.grid_z()
			));

			ui.label(&format!(
				"World Size: {:.1}x{:.1} (meters)",
				settings.terrain.world_x(),
				settings.terrain.world_z()
			));

			ui.label("Height Multiplier:");
			ui.add(egui::Slider::new(
				&mut settings.terrain.height_multiplier,
				0.0..=2.0,
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
	let grid_x = settings.terrain.grid_x();
	let grid_z = settings.terrain.grid_z();
	let height_map = HeightMap::new(
		grid_x,
		grid_z,
		settings.terrain.world_x(),
		settings.terrain.world_z(),
		&settings.noise,
	);

	let terrain_mesh = generate_mesh_from_height_map(
		&height_map,
		grid_x,
		grid_z,
		settings.terrain.world_x(), // X
		settings.terrain.world_z(), // Z
		settings.terrain.height_multiplier,
	);
	let noise_texture = generate_texture_from_height_map(&height_map, grid_x, grid_z);
	let terrain_handle = meshes.add(terrain_mesh);
	let noise_handle = images.add(noise_texture);

	commands.spawn((
		Mesh3d(terrain_handle),
		MeshMaterial3d(materials.add(Color::srgb(0.3, 0.5, 0.3))),
		TerrainMesh,
		height_map,
	));

	commands.spawn((
		ImageNode::new(noise_handle),
		NoiseTexture,
		Node {
			justify_self: JustifySelf::End,
			align_self: AlignSelf::Start,
			width: Val::Px(grid_x as f32 + 1.0),
			height: Val::Px(grid_z as f32 + 1.0),
			padding: UiRect::all(Val::Px(10.0)),
			..default()
		},
	));
}

fn update_terrain(
	mut noise_texture_query: Query<(&mut ImageNode, &mut Node), With<NoiseTexture>>,
	mut images: ResMut<Assets<Image>>,
	mut terrain_query: Query<&mut Mesh3d, With<TerrainMesh>>,
	mut meshes: ResMut<Assets<Mesh>>,
	settings: Res<Settings>,
) {
	// Only update if settings have changed
	if settings.is_changed() {
		let grid_x = settings.terrain.grid_x();
		let grid_z = settings.terrain.grid_z();
		let height_map = HeightMap::new(
			grid_x,
			grid_z,
			settings.terrain.world_x(),
			settings.terrain.world_z(),
			&settings.noise,
		);
		if let Ok((mut image_node, mut node)) = noise_texture_query.single_mut() {
			let new_texture = generate_texture_from_height_map(&height_map, grid_x, grid_z);
			let new_texture_handle = images.add(new_texture);
			*image_node = ImageNode::new(new_texture_handle);
			node.width = Val::Px(grid_x as f32 + 1.0);
			node.height = Val::Px(grid_z as f32 + 1.0);
		}
		if let Ok(mut mesh_3d) = terrain_query.single_mut() {
			let new_terrain_mesh = generate_mesh_from_height_map(
				&height_map,
				grid_x,
				grid_z,
				settings.terrain.world_x(), // X
				settings.terrain.world_z(), // Z
				settings.terrain.height_multiplier,
			);

			let new_mesh_handle = meshes.add(new_terrain_mesh);
			*mesh_3d = Mesh3d(new_mesh_handle);
		}
	}
}

#[derive(Debug, Component)]
pub struct HeightMap {
	width: u32,
	length: u32,
	heights: Vec<f32>,
}

impl HeightMap {
	/// Creates a new HeightMap for rectangular terrain
	///
	/// # Arguments
	/// * `grid_x` - Number of vertices along the X-axis (length)
	/// * `grid_z` - Number of vertices along the Z-axis (width)  
	/// * `world_x` - World size along the X-axis (length)
	/// * `world_z` - World size along the Z-axis (width)
	/// * `params` - Noise generation parameters
	///
	/// # Example
	/// ```rust
	/// // Create a 128x64 rectangular terrain (128 vertices along X, 64 along Z)
	/// let height_map = HeightMap::new(128, 64, 1000.0, 500.0, &noise_params);
	///
	/// // Access height at position (x=50, z=30)
	/// let height = height_map.get(50, 30);
	///
	/// // Check if coordinates are in bounds
	/// if height_map.in_bounds(50, 30) {
	///     height_map.set(50, 30, 0.5);
	/// }
	///
	/// // Safe access with bounds checking
	/// let height = height_map.get_safe(50, 30); // Returns 0.0 if out of bounds
	/// height_map.set_safe(50, 30, 0.5); // Ignores if out of bounds
	/// ```
	fn new(grid_x: u32, grid_z: u32, world_x: f32, world_z: f32, params: &NoiseConfig) -> Self {
		// Create noise objects for each octave once
		let mut octave_noises = Vec::new();
		for octave in 0..params.octaves {
			let octave_seed = params.seed.wrapping_add(octave as u32);
			octave_noises.push(OpenSimplex::new(octave_seed));
		}

		let mut height_map = Self {
			width: grid_z,
			length: grid_x,
			heights: vec![0.0; ((grid_z + 1) * (grid_x + 1)) as usize],
		};

		// Values for normalization
		let mut min_height = f32::INFINITY;
		let mut max_height = f32::NEG_INFINITY;

		// Calculate step sizes for world coordinates
		let length_x = world_x / grid_x as f32;
		let width_z = world_z / grid_z as f32;

		for z in 0..=grid_z {
			for x in 0..=grid_x {
				// Use world coordinates for noise sampling
				let x_pos = (x as f32 * length_x) - world_x / 2.0;
				let z_pos = (z as f32 * width_z) - world_z / 2.0;

				let height = calculate_height_at_position(
					x_pos,
					z_pos,
					params,
					&octave_noises,
					world_x.max(world_z),
				);
				height_map.set(x, z, height);

				min_height = min_height.min(height);
				max_height = max_height.max(height);
			}
		}

		// Normalize all values to 0-1 range
		let height_range = max_height - min_height;
		if height_range > 0.0 {
			for z in 0..=grid_z {
				for x in 0..=grid_x {
					let height = height_map.get(x, z);
					let normalized_height = (height - min_height) / height_range;
					height_map.set(x, z, normalized_height);
				}
			}
		}

		height_map
	}

	pub fn get(&self, x: u32, z: u32) -> f32 {
		// For rectangular terrain: index = z * (length + 1) + x
		// This matches the loop order: for z in 0..=grid_width, for x in 0..=grid_length
		let index = (z * (self.length + 1) + x) as usize;
		self.heights[index]
	}

	fn set(&mut self, x: u32, z: u32, height: f32) {
		// For rectangular terrain: index = z * (length + 1) + x
		// This matches the loop order: for z in 0..=grid_width, for x in 0..=grid_length
		let index = (z * (self.length + 1) + x) as usize;
		self.heights[index] = height;
	}

	/// Check if the given coordinates are within bounds
	pub fn in_bounds(&self, x: u32, z: u32) -> bool {
		x <= self.length && z <= self.width
	}

	/// Get height with bounds checking (returns 0.0 if out of bounds)
	pub fn get_safe(&self, x: u32, z: u32) -> f32 {
		if self.in_bounds(x, z) {
			self.get(x, z)
		} else {
			0.0
		}
	}
}

fn calculate_height_at_position(
	x_pos: f32,
	z_pos: f32,
	params: &NoiseConfig,
	noise_octaves: &[OpenSimplex],
	base_world_size: f32,
) -> f32 {
	let mut amplitude = 1.0_f64;
	let mut frequency = 1.0_f32;
	let mut height = 0.0_f64;
	let mut max_height = 0.0_f64;

	// Generate fractal noise using multiple octaves
	for octave in noise_octaves.iter() {
		// Normalize world coordinates for noise sampling
		// Scale down world coordinates to a reasonable range for noise
		let sample_x =
			(x_pos / base_world_size * params.scale * frequency) + (params.offset_x * frequency);
		let sample_z =
			(z_pos / base_world_size * params.scale * frequency) + (params.offset_z * frequency);

		let raw_noise_sample = octave.get([sample_x as f64, sample_z as f64]);
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
		frequency *= params.lacunarity;
	}

	// Normalize fractal noise
	height /= max_height;
	let normalized_height = (height + 1.0) * 0.5;

	// Apply exponential curve to create valleys
	normalized_height.powf(params.valley_exponent as f64) as f32
}

fn generate_mesh_from_height_map(
	height_map: &HeightMap,
	grid_x: u32,
	grid_z: u32,
	world_x: f32,
	world_z: f32,
	height_multiplier: f32,
) -> Mesh {
	let mut positions = Vec::new();
	let mut uvs = Vec::new();
	let mut indices = Vec::new();

	let x_step = world_x / grid_x as f32;
	let z_step = world_z / grid_z as f32;

	// Generate vertices
	for z in 0..=grid_z {
		for x in 0..=grid_x {
			let x_pos = (x as f32 * x_step) - world_x / 2.0;
			let z_pos = (z as f32 * z_step) - world_z / 2.0;
			let y_pos = height_map.get(x, z) * world_x.min(world_z) * height_multiplier;

			positions.push([x_pos, y_pos, z_pos]);
			uvs.push([x as f32 / grid_x as f32, z as f32 / grid_z as f32]);
		}
	}

	// Generate triangle indices
	for z in 0..grid_z {
		for x in 0..grid_x {
			let current = z * (grid_x + 1) + x;
			let next_x = current + 1;
			let next_z = (z + 1) * (grid_x + 1) + x;
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

fn generate_texture_from_height_map(height_map: &HeightMap, grid_x: u32, grid_z: u32) -> Image {
	let mut texture_data = Vec::with_capacity(((grid_x + 1) * (grid_z + 1) * 4) as usize);

	// Fill the texture data in the same order as the mesh vertices (z-major, then x)
	for z in 0..=grid_z {
		for x in 0..=grid_x {
			let height = height_map.get(x, z);
			let pixel_value = (height * 255.0) as u8;
			texture_data.extend_from_slice(&[pixel_value, pixel_value, pixel_value, 255]);
		}
	}

	Image::new_fill(
		Extent3d {
			width: grid_x + 1,
			height: grid_z + 1,
			depth_or_array_layers: 1,
		},
		TextureDimension::D2,
		&texture_data,
		TextureFormat::Rgba8UnormSrgb,
		RenderAssetUsages::all(),
	)
}
