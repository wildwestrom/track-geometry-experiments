use bevy::{
	prelude::*,
	render::{
		RenderPlugin,
		mesh::{Indices, Mesh, PrimitiveTopology},
		render_asset::RenderAssetUsages,
		render_resource::{Extent3d, TextureDimension, TextureFormat},
		settings::{WgpuFeatures, WgpuSettings},
	},
};
use bevy_egui::{EguiContexts, EguiPlugin, egui};
use bevy_panorbit_camera::PanOrbitCameraPlugin;
use noise::{NoiseFn, OpenSimplex};
use std::f32::consts::PI;

mod hud;
use hud::CameraDebugHud;

#[derive(Resource)]
struct NoiseParameters {
	seed: u32,
	offset_x: f32,
	offset_z: f32,
	scale: f32,
	octaves: u8,
	persistence: f32,
	lacunarity: f32,
	valley_exponent: f32,
	// This is only for the mesh height, everything else is normalized
	terrain_height: f32,
}

impl Default for NoiseParameters {
	fn default() -> Self {
		Self {
			seed: 0,
			offset_x: 0.0,
			offset_z: 0.0,
			scale: 0.75,
			octaves: 8,
			persistence: 0.4,
			lacunarity: 2.0,
			valley_exponent: 6.0,
			terrain_height: 1.0,
		}
	}
}

#[derive(Component)]
struct NoiseTexture;

#[derive(Component)]
struct TerrainMesh;

const NOISE_MAX: f64 = 0.544;
const TERRAIN_SIZE: u32 = 64;

fn main() {
	App::new()
		.add_plugins(DefaultPlugins.set(RenderPlugin {
			render_creation: bevy::render::settings::RenderCreation::Automatic(WgpuSettings {
				features: WgpuFeatures::POLYGON_MODE_LINE,
				..default()
			}),
			..default()
		}))
		.add_plugins(PanOrbitCameraPlugin)
		.add_plugins(EguiPlugin::default())
		.add_plugins(CameraDebugHud)
		.insert_resource(NoiseParameters::default())
		.add_systems(Startup, setup)
		.add_systems(Update, update_terrain_mesh)
		.add_systems(bevy_egui::EguiPrimaryContextPass, ui_system)
		.run();
}

fn setup(
	mut commands: Commands,
	mut meshes: ResMut<Assets<Mesh>>,
	mut materials: ResMut<Assets<StandardMaterial>>,
	mut images: ResMut<Assets<Image>>,
	noise_params: Res<NoiseParameters>,
) {
	// Generate height map once
	let height_map = generate_height_map(TERRAIN_SIZE, TERRAIN_SIZE, &noise_params);

	let terrain_mesh = generate_mesh_from_height_map(
		&height_map,
		TERRAIN_SIZE,
		TERRAIN_SIZE,
		5.0,
		5.0,
		noise_params.terrain_height,
	);
	let noise_texture = generate_texture_from_height_map(&height_map, TERRAIN_SIZE, TERRAIN_SIZE);
	let terrain_handle = meshes.add(terrain_mesh);
	let noise_handle = images.add(noise_texture);

	commands.spawn((
		Mesh3d(terrain_handle),
		MeshMaterial3d(materials.add(Color::srgb(0.3, 0.5, 0.3))),
		TerrainMesh,
	));

	commands.spawn((
		Transform::from_translation(Vec3::new(0.0, 1.5, 5.0)),
		bevy_panorbit_camera::PanOrbitCamera::default(),
	));
	commands.spawn((
		DirectionalLight {
			illuminance: light_consts::lux::OVERCAST_DAY,
			shadows_enabled: true,
			..default()
		},
		Transform {
			translation: Vec3::new(0.0, 2.0, 0.0),
			rotation: Quat::from_rotation_x(-PI / 4.),
			..default()
		},
	));

	commands.spawn((
		Camera2d,
		Camera {
			order: 1,
			..default()
		},
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

fn ui_system(mut contexts: EguiContexts, mut noise_params: ResMut<NoiseParameters>) {
	if let Ok(ctx) = contexts.ctx_mut() {
		egui::Window::new("Noise Controls").show(ctx, |ui| {
			ui.label("Seed:");
			ui.add(egui::DragValue::new(&mut noise_params.seed).speed(1.0));

			ui.label("Offset X:");
			ui.add(
				egui::Slider::new(&mut noise_params.offset_x, -10.0..=10.0)
					.step_by(0.1)
					.text("Offset X"),
			);

			ui.label("Offset Z:");
			ui.add(
				egui::Slider::new(&mut noise_params.offset_z, -10.0..=10.0)
					.step_by(0.1)
					.text("Offset Z"),
			);

			ui.label("Scale:");
			ui.add(egui::Slider::new(&mut noise_params.scale, 0.01..=10.0).text("Scale"));

			ui.label("Octaves:");
			ui.add(
				egui::Slider::new(&mut noise_params.octaves, 1..=8)
					.text("Octaves")
					.step_by(1.0),
			);

			ui.label("Persistence:");
			ui.add(egui::Slider::new(&mut noise_params.persistence, 0.0..=1.0).text("Persistence"));

			ui.label("Lacunarity:");
			ui.add(egui::Slider::new(&mut noise_params.lacunarity, 1.01..=4.0).text("Lacunarity"));

			ui.separator();
			ui.label("Valley Controls:");

			ui.label("Valley Exponent:");
			ui.add(
				egui::Slider::new(&mut noise_params.valley_exponent, 0.0..=10.0)
					.text("Valley Exponent"),
			);

			ui.separator();
			ui.label("Terrain Controls:");

			ui.label("Terrain Height:");
			ui.add(
				egui::Slider::new(&mut noise_params.terrain_height, 0.1..=5.0)
					.text("Terrain Height"),
			);
		});
	}
}

fn update_terrain_mesh(
	mut noise_texture_query: Query<&mut ImageNode, With<NoiseTexture>>,
	mut images: ResMut<Assets<Image>>,
	mut terrain_query: Query<&mut Mesh3d, With<TerrainMesh>>,
	mut meshes: ResMut<Assets<Mesh>>,
	noise_params: Res<NoiseParameters>,
) {
	if let Ok(mut mesh_3d) = terrain_query.single_mut() {
		if let Ok(mut image_node) = noise_texture_query.single_mut() {
			let height_map = generate_height_map(TERRAIN_SIZE, TERRAIN_SIZE, &noise_params);

			let new_terrain_mesh = generate_mesh_from_height_map(
				&height_map,
				TERRAIN_SIZE,
				TERRAIN_SIZE,
				5.0,
				5.0,
				noise_params.terrain_height,
			);
			let new_mesh_handle = meshes.add(new_terrain_mesh);
			*mesh_3d = Mesh3d(new_mesh_handle);

			let new_texture =
				generate_texture_from_height_map(&height_map, TERRAIN_SIZE, TERRAIN_SIZE);
			let new_texture_handle = images.add(new_texture);
			*image_node = ImageNode::new(new_texture_handle);
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

fn generate_height_map(grid_width: u32, grid_height: u32, params: &NoiseParameters) -> HeightMap {
	let noise = OpenSimplex::new(params.seed);
	let mut height_map = HeightMap::new(grid_width, grid_height);

	// First pass: generate raw height values
	for z in 0..=grid_height {
		for x in 0..=grid_width {
			let x_pos = (x as f32 / grid_width as f32) - 0.5;
			let z_pos = (z as f32 / grid_height as f32) - 0.5;

			let height = calculate_height_at_position(x_pos, z_pos, params, &noise);
			height_map.set(x, z, height);
		}
	}

	// Second pass: normalize to full 0-1 range
	let mut min_height = f32::INFINITY;
	let mut max_height = f32::NEG_INFINITY;

	// Find min and max values
	for z in 0..=grid_height {
		for x in 0..=grid_width {
			let height = height_map.get(x, z);
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
	params: &NoiseParameters,
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

		let raw_noise_sample = (noise.get([sample_x, sample_z]) / NOISE_MAX).clamp(-1.0, 1.0);
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
	world_width: f32,
	world_height: f32,
	terrain_height: f32,
) -> Mesh {
	let mut positions = Vec::new();
	let mut normals = Vec::new();
	let mut uvs = Vec::new();
	let mut indices = Vec::new();

	let width_step = world_width / grid_width as f32;
	let height_step = world_height / grid_height as f32;

	// Generate vertices
	for z in 0..=grid_height {
		for x in 0..=grid_width {
			let x_pos = (x as f32 * width_step) - world_width / 2.0;
			let z_pos = (z as f32 * height_step) - world_height / 2.0;
			let y_pos = height_map.get(x, z) * terrain_height;

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
			let normal = edge1.cross(edge2).normalize();

			normals_calculated[i0] = [normal.x, normal.y, normal.z];
			normals_calculated[i1] = [normal.x, normal.y, normal.z];
			normals_calculated[i2] = [normal.x, normal.y, normal.z];
		}
	}

	// Average normals for shared vertices
	for i in 0..normals_calculated.len() {
		let mut normal_sum = Vec3::ZERO;
		let mut count = 0;

		for chunk in indices.chunks(3) {
			if chunk.len() == 3 && chunk.contains(&(i as u32)) {
				let i0 = chunk[0] as usize;
				let i1 = chunk[1] as usize;
				let i2 = chunk[2] as usize;

				let v0 = Vec3::from(positions[i0]);
				let v1 = Vec3::from(positions[i1]);
				let v2 = Vec3::from(positions[i2]);

				let edge1 = v1 - v0;
				let edge2 = v2 - v0;
				normal_sum += edge1.cross(edge2);
				count += 1;
			}
		}

		if count > 0 {
			let averaged_normal = normal_sum.normalize();
			normals_calculated[i] = [averaged_normal.x, averaged_normal.y, averaged_normal.z];
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
