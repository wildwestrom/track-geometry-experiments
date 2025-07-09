use crate::spatial::{grid_to_world, world_size_for_height};
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

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct TerrainUpdateSet;

impl Plugin for TerrainPlugin {
	fn build(&self, app: &mut App) {
		app.insert_resource(load_settings())
			.add_systems(Startup, setup_terrain)
			.add_systems(Update, update_terrain.in_set(TerrainUpdateSet))
			.add_systems(bevy_egui::EguiPrimaryContextPass, ui_system);
	}
}

#[derive(Resource, Serialize, Deserialize, Clone)]
pub struct Settings {
	// Terrain settings
	pub base_grid_resolution: u32,
	pub aspect_x: u32,
	pub aspect_z: u32,
	pub base_world_size: f32,
	pub height_multiplier: f32,

	// Noise settings
	pub seed: u32,
	pub offset_x: f32,
	pub offset_z: f32,
	pub scale: f32,
	pub octaves: u8,
	pub persistence: f32,
	pub lacunarity: f32,
	pub valley_exponent: f32,
	pub height_roughness: f32,
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
			// Terrain defaults
			base_grid_resolution: 8,
			aspect_x: 1,
			aspect_z: 1,
			base_world_size: 1000.0,
			height_multiplier: 0.5,

			// Noise defaults
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

impl Settings {
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

#[derive(Component)]
struct NoiseTexture;

#[derive(Component)]
struct TerrainMesh;

/// Contains computed terrain dimensions and generation methods
struct TerrainGenerator {
	grid_x: u32,
	grid_z: u32,
	world_x: f32,
	world_z: f32,
	height_multiplier: f32,
	height_map: HeightMap,
}

impl TerrainGenerator {
	fn from_settings(settings: &Settings) -> Self {
		let grid_x = settings.grid_x();
		let grid_z = settings.grid_z();
		let world_x = settings.world_x();
		let world_z = settings.world_z();

		// Create empty height map with correct dimensions
		let height_map = HeightMap {
			length: grid_x,
			heights: vec![0.0; ((grid_z + 1) * (grid_x + 1)) as usize],
		};

		let mut generator = Self {
			grid_x,
			grid_z,
			world_x,
			world_z,
			height_multiplier: settings.height_multiplier,
			height_map,
		};

		// Generate the height map data
		generator.generate_height_map(settings);
		generator
	}

	fn calculate_height_at_position(
		&self,
		x_pos: f32,
		z_pos: f32,
		settings: &Settings,
		noise_octaves: &[OpenSimplex],
	) -> f32 {
		let mut amplitude = 1.0_f64;
		let mut frequency = 1.0_f32;
		let mut height = 0.0_f64;
		let mut max_height = 0.0_f64;
		let base_world_size = self.world_x.max(self.world_z);

		// Generate fractal noise using multiple octaves
		for octave in noise_octaves.iter() {
			// Normalize world coordinates for noise sampling
			// Scale down world coordinates to a reasonable range for noise
			let sample_x = (x_pos / base_world_size * settings.scale * frequency)
				+ (settings.offset_x * frequency);
			let sample_z = (z_pos / base_world_size * settings.scale * frequency)
				+ (settings.offset_z * frequency);

			let raw_noise_sample = octave.get([sample_x as f64, sample_z as f64]);
			height += raw_noise_sample * amplitude;
			max_height += amplitude;

			// Calculate height-dependent persistence
			// Convert current height to 0-1 range for the height roughness calculation
			let current_height_normalized = (height / max_height + 1.0) * 0.5;

			// Apply height roughness: higher elevations get more roughness (higher persistence)
			// The height_roughness parameter controls how much the height affects persistence
			let height_factor =
				1.0 + (current_height_normalized * settings.height_roughness as f64);
			let dynamic_persistence = settings.persistence as f64 * height_factor;

			amplitude *= dynamic_persistence;
			frequency *= settings.lacunarity;
		}

		// Normalize fractal noise
		height /= max_height;
		let normalized_height = (height + 1.0) * 0.5;

		// Apply exponential curve to create valleys
		normalized_height.powf(settings.valley_exponent as f64) as f32
	}

	fn generate_height_map(&mut self, settings: &Settings) {
		// Create noise objects for each octave once
		let mut octave_noises = Vec::new();
		for octave in 0..settings.octaves {
			let octave_seed = settings.seed.wrapping_add(octave as u32);
			octave_noises.push(OpenSimplex::new(octave_seed));
		}

		// Values for normalization
		let mut min_height = f32::INFINITY;
		let mut max_height = f32::NEG_INFINITY;

		for z in 0..=self.grid_z {
			for x in 0..=self.grid_x {
				// Use spatial utilities for world coordinate conversion
				let world_pos = grid_to_world(x, z, settings);
				let height = self.calculate_height_at_position(
					world_pos.x,
					world_pos.z,
					settings,
					&octave_noises,
				);
				self.height_map.set(x, z, height);

				min_height = min_height.min(height);
				max_height = max_height.max(height);
			}
		}

		// Normalize all values to 0-1 range
		let height_range = max_height - min_height;
		if height_range > 0.0 {
			for z in 0..=self.grid_z {
				for x in 0..=self.grid_x {
					let height = self.height_map.get(x, z);
					let normalized_height = (height - min_height) / height_range;
					self.height_map.set(x, z, normalized_height);
				}
			}
		}
	}

	fn generate_mesh(&self, settings: &Settings) -> Mesh {
		let mut positions = Vec::new();
		let mut uvs = Vec::new();
		let mut indices = Vec::new();

		// Generate vertices
		for z in 0..=self.grid_z {
			for x in 0..=self.grid_x {
				let world_pos = grid_to_world(x, z, settings);
				let y_pos = self.height_map.get(x, z)
					* world_size_for_height(settings)
					* self.height_multiplier;

				positions.push([world_pos.x, y_pos, world_pos.z]);
				uvs.push([x as f32 / self.grid_x as f32, z as f32 / self.grid_z as f32]);
			}
		}

		// Generate triangle indices
		for z in 0..self.grid_z {
			for x in 0..self.grid_x {
				let current = z * (self.grid_x + 1) + x;
				let next_x = current + 1;
				let next_z = (z + 1) * (self.grid_x + 1) + x;
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

	fn generate_texture(&self) -> Image {
		let mut texture_data =
			Vec::with_capacity(((self.grid_x + 1) * (self.grid_z + 1) * 4) as usize);

		// Fill the texture data in the same order as the mesh vertices (z-major, then x)
		for z in 0..=self.grid_z {
			for x in 0..=self.grid_x {
				let height = self.height_map.get(x, z);
				let pixel_value = (height * 255.0) as u8;
				texture_data.extend_from_slice(&[pixel_value, pixel_value, pixel_value, 255]);
			}
		}

		Image::new_fill(
			Extent3d {
				width: self.grid_x + 1,
				height: self.grid_z + 1,
				depth_or_array_layers: 1,
			},
			TextureDimension::D2,
			&texture_data,
			TextureFormat::Rgba8UnormSrgb,
			RenderAssetUsages::all(),
		)
	}

	fn calculate_preview_dimensions(&self) -> (f32, f32) {
		let src_width = self.grid_x + 1;
		let src_height = self.grid_z + 1;
		let max_side = src_width.max(src_height) as f32;
		let scale = if max_side > 256.0 {
			256.0 / max_side
		} else {
			1.0
		};
		let preview_width = src_width as f32 * scale;
		let preview_height = src_height as f32 * scale;
		(preview_width, preview_height)
	}
}

/// Creates mesh and texture handles from current settings
fn create_terrain_assets(
	settings: &Settings,
	meshes: &mut ResMut<Assets<Mesh>>,
	images: &mut ResMut<Assets<Image>>,
) -> (Handle<Mesh>, Handle<Image>, HeightMap, f32, f32) {
	let generator = TerrainGenerator::from_settings(settings);

	let terrain_mesh = generator.generate_mesh(settings);
	let noise_texture = generator.generate_texture();
	let (preview_width, preview_height) = generator.calculate_preview_dimensions();

	let mesh_handle = meshes.add(terrain_mesh);
	let texture_handle = images.add(noise_texture);

	(
		mesh_handle,
		texture_handle,
		generator.height_map,
		preview_width,
		preview_height,
	)
}

fn render_terrain_config_ui(ui: &mut egui::Ui, settings: &mut Settings) {
	ui.label("Base Grid Resolution:");
	ui.add(egui::Slider::new(&mut settings.base_grid_resolution, 1..=128).step_by(1.0));

	ui.label("Aspect Ratio X:");
	ui.add(egui::Slider::new(&mut settings.aspect_x, 1..=8).step_by(1.0));

	ui.label("Aspect Ratio Z:");
	ui.add(egui::Slider::new(&mut settings.aspect_z, 1..=8).step_by(1.0));

	ui.label(&format!(
		"Grid Size: {}x{}",
		settings.grid_x(),
		settings.grid_z()
	));

	ui.label(&format!(
		"World Size: {:.1}x{:.1} (meters)",
		settings.world_x(),
		settings.world_z()
	));

	ui.label("Height Multiplier:");
	ui.add(egui::Slider::new(
		&mut settings.height_multiplier,
		0.0..=2.0,
	));
}

fn render_noise_config_ui(ui: &mut egui::Ui, settings: &mut Settings) {
	ui.label("Seed:");
	ui.add(egui::DragValue::new(&mut settings.seed).speed(1.0));

	ui.label("Offset X:");
	ui.add(egui::Slider::new(&mut settings.offset_x, -10.0..=10.0).step_by(0.1));

	ui.label("Offset Z:");
	ui.add(egui::Slider::new(&mut settings.offset_z, -10.0..=10.0).step_by(0.1));

	ui.label("Scale:");
	ui.add(egui::Slider::new(&mut settings.scale, 0.01..=10.0));

	ui.label("Octaves:");
	ui.add(egui::Slider::new(&mut settings.octaves, 1..=8).step_by(1.0));

	ui.label("Persistence:");
	ui.add(egui::Slider::new(&mut settings.persistence, 0.0..=1.0));

	ui.label("Lacunarity:");
	ui.add(egui::Slider::new(&mut settings.lacunarity, 1.01..=4.0));

	ui.label("Valley Exponent:");
	ui.add(egui::Slider::new(&mut settings.valley_exponent, 0.0..=20.0));

	ui.label("Height Roughness:");
	ui.add(egui::Slider::new(&mut settings.height_roughness, 0.0..=5.0));
}

fn ui_system(mut contexts: EguiContexts, mut settings: ResMut<Settings>) {
	if let Ok(ctx) = contexts.ctx_mut() {
		egui::Window::new("Terrain Controls").default_pos((10.0, 100.0)).show(ctx, |ui| {
			ui.collapsing("Terrain Configuration", |ui| {
				render_terrain_config_ui(ui, &mut settings);
			});
			ui.collapsing("Noise Parameters:", |ui| {
				render_noise_config_ui(ui, &mut settings);
			});

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
	let (mesh_handle, texture_handle, height_map, preview_width, preview_height) =
		create_terrain_assets(&settings, &mut meshes, &mut images);

	// Spawn terrain mesh
	commands.spawn((
		Mesh3d(mesh_handle),
		MeshMaterial3d(materials.add(Color::srgb(0.3, 0.5, 0.3))),
		TerrainMesh,
		height_map,
	));

	// Spawn noise texture preview
	commands.spawn((
		ImageNode::new(texture_handle),
		NoiseTexture,
		Node {
			justify_self: JustifySelf::End,
			align_self: AlignSelf::Start,
			width: Val::Px(preview_width),
			height: Val::Px(preview_height),
			padding: UiRect::all(Val::Px(10.0)),
			..default()
		},
	));
}

fn update_terrain(
	mut noise_texture_query: Query<(&mut ImageNode, &mut Node), With<NoiseTexture>>,
	mut images: ResMut<Assets<Image>>,
	mut terrain_query: Query<(&mut Mesh3d, &mut HeightMap), With<TerrainMesh>>,
	mut meshes: ResMut<Assets<Mesh>>,
	settings: Res<Settings>,
) {
	if settings.is_changed() {
		// Regenerate terrain assets
		let generator = TerrainGenerator::from_settings(&settings);
		let new_mesh = generator.generate_mesh(&settings);
		let new_texture = generator.generate_texture();
		let (preview_width, preview_height) = generator.calculate_preview_dimensions();

		// Update the terrain mesh
		if let Ok((mut mesh_handle, mut heightmap)) = terrain_query.single_mut() {
			*heightmap = generator.height_map;
			*mesh_handle = Mesh3d(meshes.add(new_mesh));
		}

		// Update the noise texture
		if let Ok((mut image_handle, mut ui_node)) = noise_texture_query.single_mut() {
			*image_handle = ImageNode::new(images.add(new_texture));

			// Update the UI node size
			ui_node.width = Val::Px(preview_width);
			ui_node.height = Val::Px(preview_height);
		}
	}
}

#[derive(Debug, Component)]
pub struct HeightMap {
	length: u32,
	heights: Vec<f32>,
}

impl HeightMap {
	pub fn get(&self, x: u32, z: u32) -> f32 {
		// For rectangular terrain: index = z * (length + 1) + x
		// This matches the loop order: for z in 0..=grid_width, for x in 0..=grid_length
		let index = (z * (self.length + 1) + x) as usize;
		*self
			.heights
			.get(index)
			.expect("The code should be written in a way such that this doesn't happen")
	}

	fn set(&mut self, x: u32, z: u32, height: f32) {
		// For rectangular terrain: index = z * (length + 1) + x
		// This matches the loop order: for z in 0..=grid_width, for x in 0..=grid_length
		let index = (z * (self.length + 1) + x) as usize;
		*self
			.heights
			.get_mut(index)
			.expect("The code should be written in a way such that this doesn't happen") = height;
	}
}
