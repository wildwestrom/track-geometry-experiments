use bevy::{
	asset::RenderAssetUsages,
	prelude::*,
	render::{
		mesh::{Indices, PrimitiveTopology},
		render_resource::{Extent3d, TextureDimension, TextureFormat},
	},
};
use bevy_egui::{EguiContexts, egui};
use noise::{NoiseFn, OpenSimplex};

pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
	fn build(&self, app: &mut App) {
		app.insert_resource(TerrainConfig::default())
			.insert_resource(NoiseConfig::default())
			.add_systems(Startup, setup_terrain)
			.add_systems(Update, update_terrain)
			.add_systems(bevy_egui::EguiPrimaryContextPass, ui_system);
	}
}

#[derive(Resource)]
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
			world_size: 5.0,
			resolution_multiplier: 4, // This gives us 64 grid cells (16 * 4)
			height_multiplier: 1.0,
		}
	}
}

#[derive(Resource)]
struct NoiseConfig {
	seed: u32,
	offset_x: f32,
	offset_z: f32,
	scale: f32,
	octaves: u8,
	persistence: f32,
	lacunarity: f32,
	valley_exponent: f32,
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
	}
}

impl Default for NoiseConfig {
	fn default() -> Self {
		Self {
			seed: 0,
			offset_x: 0.0,
			offset_z: 0.0,
			scale: 2.5,
			octaves: 8,
			persistence: 0.4,
			lacunarity: 2.0,
			valley_exponent: 1.0,
		}
	}
}

#[derive(Component)]
struct NoiseTexture;

#[derive(Component)]
struct TerrainMesh;

fn ui_system(
	mut contexts: EguiContexts,
	mut noise_params: ResMut<NoiseConfig>,
	mut terrain_config: ResMut<TerrainConfig>,
) {
	if let Ok(ctx) = contexts.ctx_mut() {
		egui::Window::new("Terrain Controls").show(ctx, |ui| {
			ui.label("Terrain Configuration:");
			ui.label("World Size:");
			ui.add(egui::Slider::new(&mut terrain_config.world_size, 1.0..=20.0).step_by(0.5));

			ui.label("Resolution Multiplier:");
			ui.add(
				egui::Slider::new(&mut terrain_config.resolution_multiplier, 1..=32).step_by(1.0),
			);

			ui.label(&format!(
				"Grid Size: {}x{}",
				terrain_config.grid_size(),
				terrain_config.grid_size()
			));

			ui.label("Height Multiplier:");
			ui.add(egui::Slider::new(
				&mut terrain_config.height_multiplier,
				0.1..=5.0,
			));

			ui.separator();
			ui.label("Noise Parameters:");

			ui.label("Seed:");
			ui.add(egui::DragValue::new(&mut noise_params.seed).speed(1.0));

			ui.label("Offset X:");
			ui.add(egui::Slider::new(&mut noise_params.offset_x, -10.0..=10.0).step_by(0.1));

			ui.label("Offset Z:");
			ui.add(egui::Slider::new(&mut noise_params.offset_z, -10.0..=10.0).step_by(0.1));

			ui.label("Scale:");
			ui.add(egui::Slider::new(&mut noise_params.scale, 0.01..=10.0));

			ui.label("Octaves:");
			ui.add(egui::Slider::new(&mut noise_params.octaves, 1..=8).step_by(1.0));

			ui.label("Persistence:");
			ui.add(egui::Slider::new(&mut noise_params.persistence, 0.0..=1.0));

			ui.label("Lacunarity:");
			ui.add(egui::Slider::new(&mut noise_params.lacunarity, 1.01..=4.0));

			ui.label("Valley Exponent:");
			ui.add(egui::Slider::new(
				&mut noise_params.valley_exponent,
				0.0..=10.0,
			));
		});
	}
}

fn update_terrain(
	mut noise_texture_query: Query<&mut ImageNode, With<NoiseTexture>>,
	mut images: ResMut<Assets<Image>>,
	mut terrain_query: Query<&mut Mesh3d, With<TerrainMesh>>,
	mut meshes: ResMut<Assets<Mesh>>,
	noise_params: Res<NoiseConfig>,
	terrain_config: Res<TerrainConfig>,
) {
	// Only update if either noise or terrain config has changed
	if noise_params.is_changed() || terrain_config.is_changed() {
		let grid_size = terrain_config.grid_size();
		let height_map = generate_height_map(grid_size, grid_size, &noise_params);
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
				terrain_config.world_width(),
				terrain_config.world_length(),
				terrain_config.height_multiplier,
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
	fn new(width: u32, height: u32) -> Self {
		Self {
			width,
			heights: vec![0.0; ((width + 1) * (height + 1)) as usize],
		}
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

fn generate_height_map(grid_width: u32, grid_height: u32, params: &NoiseConfig) -> HeightMap {
	let noise = OpenSimplex::new(params.seed);
	let mut height_map = HeightMap::new(grid_width, grid_height);

	// Values for normalization
	let mut min_height = f32::INFINITY;
	let mut max_height = f32::NEG_INFINITY;

	for z in 0..=grid_height {
		for x in 0..=grid_width {
			let x_pos = (x as f32 / grid_width as f32) - 0.5;
			let z_pos = (z as f32 / grid_height as f32) - 0.5;

			let height = calculate_height_at_position(x_pos, z_pos, params, &noise);
			height_map.set(x, z, height);

			min_height = min_height.min(height);
			max_height = max_height.max(height);
		}
	}

	// Normalize all values to 0-1 range
	let height_range = max_height - min_height;
	if height_range > 0.0 {
		for z in 0..=grid_height {
			for x in 0..=grid_width {
				let height = height_map.get(x, z);
				let normalized_height = (height - min_height) / height_range;
				height_map.set(x, z, normalized_height);
			}
		}
	}

	height_map
}

fn calculate_height_at_position(
	x_pos: f32,
	z_pos: f32,
	params: &NoiseConfig,
	noise: &OpenSimplex,
) -> f32 {
	let mut amplitude = 1.0_f64;
	let mut frequency = 1.0_f64;
	let mut height = 0.0_f64;
	let mut max_height = 0.0_f64;

	// Generate fractal noise using multiple octaves
	for _ in 0..params.octaves {
		let sample_x =
			(x_pos as f64 * params.scale as f64 * frequency) + (params.offset_x as f64 * frequency);
		let sample_z =
			(z_pos as f64 * params.scale as f64 * frequency) + (params.offset_z as f64 * frequency);

		let raw_noise_sample = noise.get([sample_x, sample_z]);
		height += raw_noise_sample * amplitude;
		max_height += amplitude;

		amplitude *= params.persistence as f64;
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
	grid_width: u32,
	grid_height: u32,
	world_length: f32,
	world_width: f32,
	height_multiplier: f32,
) -> Mesh {
	let mut positions = Vec::new();
	let mut normals = Vec::new();
	let mut uvs = Vec::new();
	let mut indices = Vec::new();

	let length_step = world_length / grid_width as f32;
	let width_step = world_width / grid_height as f32;

	// Generate vertices
	for z in 0..=grid_height {
		for x in 0..=grid_width {
			let x_pos = (x as f32 * length_step) - world_length / 2.0;
			let z_pos = (z as f32 * width_step) - world_width / 2.0;
			let y_pos = height_map.get(x, z) * height_multiplier;

			positions.push([x_pos, y_pos, z_pos]);
			normals.push([0.0, 1.0, 0.0]); // Will be recalculated
			uvs.push([x as f32 / grid_width as f32, z as f32 / grid_height as f32]);
		}
	}

	// Generate indices
	for z in 0..grid_height {
		for x in 0..grid_width {
			let top_left = z * (grid_width + 1) + x;
			let top_right = top_left + 1;
			let bottom_left = (z + 1) * (grid_width + 1) + x;
			let bottom_right = bottom_left + 1;

			// First triangle
			indices.push(top_left);
			indices.push(bottom_left);
			indices.push(top_right);

			// Second triangle
			indices.push(top_right);
			indices.push(bottom_left);
			indices.push(bottom_right);
		}
	}

	// Calculate normals
	let mut normals_calculated = vec![[0.0, 0.0, 0.0]; positions.len()];
	let mut normal_counts = vec![0; positions.len()];

	// First pass: calculate face normals and accumulate them
	for chunk in indices.chunks(3) {
		if chunk.len() == 3 {
			let i0 = chunk[0] as usize;
			let i1 = chunk[1] as usize;
			let i2 = chunk[2] as usize;

			let v0 = Vec3::from(positions[i0]);
			let v1 = Vec3::from(positions[i1]);
			let v2 = Vec3::from(positions[i2]);

			let edge1 = v1 - v0;
			let edge2 = v2 - v0;
			let face_normal = edge1.cross(edge2);

			// Add this face normal to all three vertices
			normals_calculated[i0][0] += face_normal.x;
			normals_calculated[i0][1] += face_normal.y;
			normals_calculated[i0][2] += face_normal.z;
			normal_counts[i0] += 1;

			normals_calculated[i1][0] += face_normal.x;
			normals_calculated[i1][1] += face_normal.y;
			normals_calculated[i1][2] += face_normal.z;
			normal_counts[i1] += 1;

			normals_calculated[i2][0] += face_normal.x;
			normals_calculated[i2][1] += face_normal.y;
			normals_calculated[i2][2] += face_normal.z;
			normal_counts[i2] += 1;
		}
	}

	// Second pass: normalize the accumulated normals
	for i in 0..normals_calculated.len() {
		if normal_counts[i] > 0 {
			let normal = Vec3::new(
				normals_calculated[i][0],
				normals_calculated[i][1],
				normals_calculated[i][2],
			);
			let normalized = normal.normalize();
			normals_calculated[i] = [normalized.x, normalized.y, normalized.z];
		} else {
			// Fallback for vertices not used in any triangle
			normals_calculated[i] = [0.0, 1.0, 0.0];
		}
	}

	Mesh::new(
		PrimitiveTopology::TriangleList,
		RenderAssetUsages::RENDER_WORLD,
	)
	.with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
	.with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals_calculated)
	.with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
	.with_inserted_indices(Indices::U32(indices))
}

fn generate_texture_from_height_map(
	height_map: &HeightMap,
	grid_width: u32,
	grid_height: u32,
) -> Image {
	let mut texture_data = Vec::with_capacity(((grid_width + 1) * (grid_height + 1) * 4) as usize);

	for z in 0..=grid_height {
		for x in 0..=grid_width {
			let height = height_map.get(x, z);
			let pixel_value = (height * 255.0) as u8;
			texture_data.extend_from_slice(&[pixel_value, pixel_value, pixel_value, 255]);
		}
	}

	Image::new_fill(
		Extent3d {
			width: grid_width + 1,
			height: grid_height + 1,
			depth_or_array_layers: 1,
		},
		TextureDimension::D2,
		&texture_data,
		TextureFormat::Rgba8UnormSrgb,
		RenderAssetUsages::all(),
	)
}

fn setup_terrain(
	mut commands: Commands,
	mut meshes: ResMut<Assets<Mesh>>,
	mut materials: ResMut<Assets<StandardMaterial>>,
	mut images: ResMut<Assets<Image>>,
	noise_config: Res<NoiseConfig>,
	terrain_config: Res<TerrainConfig>,
) {
	// Generate height map once
	let grid_size = terrain_config.grid_size();
	let height_map = generate_height_map(grid_size, grid_size, &noise_config);

	let terrain_mesh = generate_mesh_from_height_map(
		&height_map,
		grid_size,
		grid_size,
		terrain_config.world_width(),
		terrain_config.world_length(),
		terrain_config.height_multiplier,
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
