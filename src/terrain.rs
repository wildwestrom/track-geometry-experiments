use crate::alignment::AlignmentState;
use crate::saveable::SaveableSettings;
use crate::spatial::{calculate_terrain_height, clamp_to_terrain_bounds, grid_to_world, world_size_for_height};
use crate::terrain;
use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::{
        mesh::{Indices, PrimitiveTopology},
        render_resource::{Extent3d, TextureDimension, TextureFormat},
    },
};
use bevy_egui::{EguiContexts, egui};
use noise::{HybridMulti, MultiFractal, NoiseFn, OpenSimplex};
use serde::{Deserialize, Serialize};

pub struct TerrainPlugin;

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct TerrainUpdateSet;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(load_settings())
            .add_systems(Startup, setup_terrain)
            .add_systems(Update, update_terrain.in_set(TerrainUpdateSet))
            .add_systems(bevy_egui::EguiPrimaryContextPass, ui_system)
            .add_systems(Update, update_plot_texture.after(ui_system));
    }
}

#[derive(Resource, Serialize, Deserialize, Clone, PartialEq)]
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
    pub octaves: u8,
    pub frequency: f64,
    pub persistence: f64,
    pub lacunarity: f64,
    pub valley_exponent: f32,
    pub height_roughness: f64,
}

impl SaveableSettings for Settings {
    fn filename() -> &'static str {
        "terrain_settings.json"
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
            frequency: 3.8,
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
    Settings::load_or_default()
}

#[derive(Component)]
pub struct TerrainMesh;

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

        Self {
            grid_x,
            grid_z,
            world_x,
            world_z,
            height_multiplier: settings.height_multiplier,
            height_map,
        }
    }

    fn generate_height_map(&mut self, settings: &Settings) {
        let noise = HybridMulti::<OpenSimplex>::new(settings.seed)
            .set_octaves(settings.octaves as usize)
            .set_frequency(settings.frequency as f64)
            .set_lacunarity(settings.lacunarity as f64)
            .set_persistence(settings.persistence as f64);

        // Values for normalization
        let mut min_height = f32::INFINITY;
        let mut max_height = f32::NEG_INFINITY;

        for z in 0..=self.grid_z {
            for x in 0..=self.grid_x {
                // Use spatial utilities for world coordinate conversion
                let world_pos = grid_to_world(x, z, settings);
                let height = self.calculate_height_at_position(
                    world_pos.x as f64,
                    world_pos.z as f64,
                    settings,
                    &noise,
                ) as f32;
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

                    // Redistribute the noise
                    let final_height = normalized_height.powf(settings.valley_exponent);

                    self.height_map.set(x, z, final_height);
                }
            }
        } else {
            panic!(
                "Height range is {} (min: {}, max: {}). This indicates a bug in terrain generation.",
                height_range, min_height, max_height
            );
        }

        settings.valley_exponent;
    }

    fn calculate_height_at_position(
        &self,
        x_pos: f64,
        z_pos: f64,
        settings: &Settings,
        noise: impl NoiseFn<f64, 2>,
    ) -> f64 {
        let base_world_size = self.world_x.max(self.world_z) as f64;

        // Scale the offset inversely with frequency to maintain the same relative position
        // when frequency changes (since noise function internally scales coordinates by frequency)
        let sample_x = (x_pos / base_world_size) + (settings.offset_x as f64 / settings.frequency);
        let sample_z = (z_pos / base_world_size) + (settings.offset_z as f64 / settings.frequency);
        let height = noise.get([sample_x, sample_z]);

        height
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
    let mut generator = TerrainGenerator::from_settings(settings);
    generator.generate_height_map(settings);

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

/// Helper function to create a labeled slider with standard formatting
fn add_labeled_slider<T>(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut T,
    range: impl Into<std::ops::RangeInclusive<T>>,
) where
    T: egui::emath::Numeric,
{
    ui.label(label);
    ui.add(egui::Slider::new(value, range.into()));
}

/// Helper function to create a labeled integer slider with step
fn add_labeled_int_slider<T>(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut T,
    range: impl Into<std::ops::RangeInclusive<T>>,
) where
    T: egui::emath::Numeric,
{
    ui.label(label);
    ui.add(egui::Slider::new(value, range.into()).step_by(1.0));
}

/// Helper function to add an info label with formatting
fn add_info_label(ui: &mut egui::Ui, label: &str, args: std::fmt::Arguments) {
    ui.label(&format!("{}: {}", label, args));
}

fn render_terrain_config_ui(ui: &mut egui::Ui, settings: &mut Settings) {
    add_labeled_int_slider(
        ui,
        "Base Grid Resolution",
        &mut settings.base_grid_resolution,
        1..=128,
    );
    add_labeled_int_slider(ui, "Aspect Ratio X", &mut settings.aspect_x, 1..=8);
    add_labeled_int_slider(ui, "Aspect Ratio Z", &mut settings.aspect_z, 1..=8);

    add_info_label(
        ui,
        "Grid Size",
        format_args!("{}x{}", settings.grid_x(), settings.grid_z()),
    );
    add_info_label(
        ui,
        "World Size",
        format_args!(
            "{:.1}x{:.1} (meters)",
            settings.world_x(),
            settings.world_z()
        ),
    );

    add_labeled_slider(
        ui,
        "Height Multiplier",
        &mut settings.height_multiplier,
        0.0..=1.0,
    );
    ui.label("Height Multiplier (fine):");
    ui.add(
        egui::Slider::new(&mut settings.height_multiplier, 0.0..=0.25)
            .clamping(egui::SliderClamping::Edits),
    );
}

fn render_noise_config_ui(ui: &mut egui::Ui, settings: &mut Settings) {
    ui.label("Seed");
    ui.add(egui::DragValue::new(&mut settings.seed).speed(1));

    add_labeled_slider(ui, "Offset X", &mut settings.offset_x, -5.0..=5.0);
    add_labeled_slider(ui, "Offset Z", &mut settings.offset_z, -5.0..=5.0);
    add_labeled_slider(ui, "Frequency", &mut settings.frequency, 0.01..=10.0);
    add_labeled_int_slider(ui, "Octaves", &mut settings.octaves, 1..=8);
    add_labeled_slider(ui, "Persistence", &mut settings.persistence, 0.001..=1.0);
    add_labeled_slider(ui, "Lacunarity", &mut settings.lacunarity, 1.01..=4.0);
    add_labeled_slider(
        ui,
        "Valley Exponent",
        &mut settings.valley_exponent,
        0.0..=20.0,
    );
}

#[derive(Resource, Clone)]
pub struct NoiseTextureResource {
    pub handle: Handle<Image>,
    pub width: f32,
    pub height: f32,
}

#[derive(Resource, Clone)]
pub struct PlotTextureResource {
    pub handle: Handle<Image>,
    pub width: f32,
    pub height: f32,
}

#[derive(Resource, Clone, Debug, PartialEq)]
pub struct PlotWidth(pub usize);

fn ui_system(
    mut contexts: EguiContexts,
    mut settings: ResMut<Settings>,
    noise_texture_res: Res<NoiseTextureResource>,
    plot_texture_res: Res<PlotTextureResource>,
    mut plot_width: ResMut<PlotWidth>,
) {
    // Get the texture_id before borrowing ctx_mut
    let texture_id = contexts.add_image(noise_texture_res.handle.clone());
    let plot_texture_id = contexts.add_image(plot_texture_res.handle.clone());

    if let Ok(ctx) = contexts.ctx_mut() {
        // Create a snapshot of current settings to detect actual changes
        let settings_snapshot = settings.clone();

        // Bypass change detection so we can manually control when changes are detected
        let settings_ptr = settings.bypass_change_detection();

        egui::Window::new("Terrain Controls")
            .default_pos(egui::pos2(10.0, 100.0))
            .show(ctx, |ui| {
                ui.collapsing("Terrain Configuration", |ui| {
                    render_terrain_config_ui(ui, settings_ptr);
                });
                ui.collapsing("Noise Parameters:", |ui| {
                    render_noise_config_ui(ui, settings_ptr);
                });
                ui.separator();
                settings_ptr.handle_save_operation_ui(ui, "Save Settings");
            });

        let image_width = noise_texture_res.width;
        let image_height = noise_texture_res.height;
        let aspect_ratio = image_width / image_height;

        egui::Window::new("Visualizations")
            .default_pos(egui::pos2(300.0, 100.0))
            .vscroll(true)
            .show(ctx, |ui| {
                let available = ui.available_size();
                let avail_aspect = available.x / available.y;

                let (draw_width, draw_height) = if avail_aspect > aspect_ratio {
                    // Available area is wider than image: fit by height
                    let h = available.y;
                    let w = h * aspect_ratio;
                    (w, h)
                } else {
                    // Available area is taller than image: fit by width
                    let w = available.x;
                    let h = w / aspect_ratio;
                    (w, h)
                };

                ui.label("Noise Texture");
                ui.image((texture_id, egui::vec2(draw_width, draw_height)));
                let plot_width_pixels = available.x as usize;
                if plot_width_pixels != plot_width.0 {
                    plot_width.0 = plot_width_pixels;
                }
                let plot_width_f = plot_texture_res.as_ref().width;
                let plot_height_f = plot_texture_res.as_ref().height;
                let plot_aspect = plot_width_f / plot_height_f;
                let plot_draw_width = draw_width;
                let plot_draw_height = draw_width / plot_aspect;

                ui.label("Elevation Profile");
                ui.image((
                    plot_texture_id,
                    egui::vec2(plot_draw_width, plot_draw_height),
                ));
            });

        // Only mark as changed if settings actually changed
        if *settings_ptr != settings_snapshot {
            settings.set_changed();
        }
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

    // Store the noise texture handle as a resource for egui
    commands.insert_resource(NoiseTextureResource {
        handle: texture_handle,
        width: preview_width,
        height: preview_height,
    });

    // Set initial plot width resource (default to 256)
    commands.insert_resource(PlotWidth(256));
}

// TODO: Replace with bevy_prototype_lyon for better 2d drawing
fn update_plot_texture(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    plot_width: Res<PlotWidth>,
    alignment_state: Option<Res<AlignmentState>>,
) {
    let plot_width = plot_width.0;
    let plot_height = plot_width / 8;
    let mut plot_data = Vec::with_capacity(plot_width * plot_height * 4);
    for _ in 0..(plot_width * plot_height) {
        plot_data.extend_from_slice(&[0, 0, 0, 255]); // Black
    }
    let mut has_profile = false;
    if let Some(alignment_state) = alignment_state {
        if let Some(alignment) = alignment_state.alignments.get(&alignment_state.current) {
            let samples = alignment.sample_elevation_profile(plot_width);
            if !samples.is_empty() {
                has_profile = true;
                // Find min/max height for normalization
                let min_h = samples
                    .iter()
                    .map(|&(_, h)| h)
                    .fold(f32::INFINITY, f32::min);
                let max_h = samples
                    .iter()
                    .map(|&(_, h)| h)
                    .fold(f32::NEG_INFINITY, f32::max);
                let height_range = (max_h - min_h).max(1e-5);
                // Draw polyline
                for (i, &(_, h)) in samples.iter().enumerate() {
                    let x = i;
                    let norm_h = (h - min_h) / height_range;
                    let y = ((1.0 - norm_h) * (plot_height as f32 - 1.0)).round() as usize;
                    if x < plot_width && y < plot_height {
                        let idx = (y * plot_width + x) * 4;
                        plot_data[idx..idx + 4].copy_from_slice(&[255, 0, 0, 255]); // Red pixel
                    }
                }
            }
        }
    }
    if !has_profile {
        plot_data.clear();
        for _ in 0..(plot_width * plot_height) {
            plot_data.extend_from_slice(&[0, 0, 0, 255]);
        }
    }
    let plot_image = Image::new_fill(
        Extent3d {
            width: plot_width as u32,
            height: plot_height as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &plot_data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::all(),
    );
    let plot_handle = images.add(plot_image);
    let plot_res = PlotTextureResource {
        handle: plot_handle,
        width: plot_width as f32,
        height: plot_height as f32,
    };
    commands.insert_resource(plot_res);
}

fn update_terrain(
    mut images: ResMut<Assets<Image>>,
    mut terrain_query: Query<(&mut Mesh3d, &mut HeightMap), With<TerrainMesh>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut noise_texture_res: ResMut<NoiseTextureResource>,
    settings: Res<Settings>,
) {
    if settings.is_changed() {
        // Create generator and generate height map once
        let mut generator = TerrainGenerator::from_settings(&settings);
        generator.generate_height_map(&settings);

        // Generate mesh and texture from the populated height map
        let new_mesh = generator.generate_mesh(&settings);
        let new_texture = generator.generate_texture();
        let (preview_width, preview_height) = generator.calculate_preview_dimensions();

        // Replace the terrain mesh entity
        if let Ok((mut mesh_handle, mut height_map)) = terrain_query.single_mut() {
            let old_mesh_id = mesh_handle.id();

            // Create new mesh and update the handle
            *mesh_handle = Mesh3d(meshes.add(new_mesh));
            *height_map = generator.height_map;

            // Remove the old mesh asset before creating a new one
            meshes.remove(old_mesh_id);
        }

        // Update the noise texture resource
        let old_image_id = noise_texture_res.handle.id();
        noise_texture_res.handle = images.add(new_texture);
        noise_texture_res.width = preview_width;
        noise_texture_res.height = preview_height;
        images.remove(old_image_id);
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

/// Performs ray-terrain intersection by stepping along the ray and checking heights
pub fn raycast_terrain(
    ray: &Ray3d,
    heightmap: &HeightMap,
    settings: &terrain::Settings,
) -> Option<Vec3> {
    let world_x = settings.world_x();
    let world_z = settings.world_z();

    // Calculate terrain bounds
    let min_x = -world_x / 2.0;
    let max_x = world_x / 2.0;
    let min_z = -world_z / 2.0;
    let max_z = world_z / 2.0;

    // Find the maximum possible terrain height for bounds checking
    let max_terrain_height = world_size_for_height(settings) * settings.height_multiplier;
    let min_terrain_height = 0.0; // Assuming terrain doesn't go below 0

    // Calculate intersection with terrain bounding box
    let mut t_min = 0.0f32;
    let mut t_max = f32::INFINITY;

    // Check X bounds
    if ray.direction.x != 0.0 {
        let tx1 = (min_x - ray.origin.x) / ray.direction.x;
        let tx2 = (max_x - ray.origin.x) / ray.direction.x;
        t_min = t_min.max(tx1.min(tx2));
        t_max = t_max.min(tx1.max(tx2));
    } else if ray.origin.x < min_x || ray.origin.x > max_x {
        return None; // Ray is parallel to X bounds and outside
    }

    // Check Z bounds
    if ray.direction.z != 0.0 {
        let tz1 = (min_z - ray.origin.z) / ray.direction.z;
        let tz2 = (max_z - ray.origin.z) / ray.direction.z;
        t_min = t_min.max(tz1.min(tz2));
        t_max = t_max.min(tz1.max(tz2));
    } else if ray.origin.z < min_z || ray.origin.z > max_z {
        return None; // Ray is parallel to Z bounds and outside
    }

    // Check Y bounds (terrain height range)
    if ray.direction.y != 0.0 {
        let ty1 = (min_terrain_height - ray.origin.y) / ray.direction.y;
        let ty2 = (max_terrain_height - ray.origin.y) / ray.direction.y;
        t_min = t_min.max(ty1.min(ty2));
        t_max = t_max.min(ty1.max(ty2));
    }

    // No intersection with bounding box
    if t_min > t_max || t_max < 0.0 {
        return None;
    }

    // Start from the entry point of the bounding box
    let mut t = t_min.max(0.0);
    let step_size = 0.2;
    let max_iterations = 10000; // Safety limit to prevent infinite loops
    let mut iterations = 0;

    let mut last_valid_point = None;

    while t <= t_max && iterations < max_iterations {
        iterations += 1;

        let point = ray.origin + ray.direction * t;

        // Clamp point to terrain bounds for height lookup
        let clamped_point = clamp_to_terrain_bounds(point, settings);

        // Get height from heightmap and apply the same scaling as the terrain mesh
        let terrain_height = calculate_terrain_height(clamped_point, heightmap, settings);

        // Check if ray point is at or below terrain height
        if point.y <= terrain_height {
            // Use clamped position for the result
            return Some(Vec3::new(clamped_point.x, terrain_height, clamped_point.z));
        }

        // Store the last valid point in case we need to fall back to terrain boundary
        if point.x >= min_x && point.x <= max_x && point.z >= min_z && point.z <= max_z {
            last_valid_point = Some(Vec3::new(clamped_point.x, terrain_height, clamped_point.z));
        }

        t += step_size;
    }

    // If we didn't find an intersection, return the last valid point on terrain boundary
    last_valid_point
}