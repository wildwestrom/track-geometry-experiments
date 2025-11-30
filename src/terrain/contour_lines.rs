use crate::terrain;
use bevy::pbr::MaterialPlugin;
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, Extent3d, ShaderType, TextureDimension, TextureFormat};
use bevy::shader::ShaderRef;
use bevy_egui::{EguiContexts, egui};
use log::debug;

pub struct ContourLinePlugin;

impl Plugin for ContourLinePlugin {
	fn build(&self, app: &mut App) {
		// Ensure the terrain plugin has a buffer to read elevation samples from.
		app
			.init_resource::<ContourState>()
			.add_plugins(MaterialPlugin::<ContourMaterial>::default())
			.add_systems(Startup, create_placeholder_texture)
			.add_systems(PostStartup, setup_contour_terrain_material)
			.add_systems(bevy_egui::EguiPrimaryContextPass, contour_controls_ui)
			.add_systems(
				Update,
				(
					update_contour_materials,
					toggle_material_system,
					apply_material_from_contour_state,
				),
			);
	}
}

/// Resource to store a placeholder texture for ContourMaterial
/// This is needed because the shader requires a texture binding even though we don't use it
#[derive(Resource)]
struct PlaceholderTextureResource {
	handle: Handle<Image>,
}

/// System to create a simple 1x1 white texture as a placeholder for ContourMaterial
fn create_placeholder_texture(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
	// Create a 1x1 white texture
	let placeholder_image = Image::new_fill(
		Extent3d {
			width: 1,
			height: 1,
			depth_or_array_layers: 1,
		},
		TextureDimension::D2,
		&[255, 255, 255, 255], // White RGBA
		TextureFormat::Rgba8UnormSrgb,
		bevy::asset::RenderAssetUsages::all(),
	);

	let handle = images.add(placeholder_image);
	commands.insert_resource(PlaceholderTextureResource { handle });
	debug!("Created placeholder texture for ContourMaterial");
}

/// System to add contour overlay material as a child entity
/// This runs on startup to ensure terrain gets an overlay
/// Keeps the StandardMaterial on the parent and adds contour overlay as a child
fn setup_contour_terrain_material(
	mut commands: Commands,
	mut contour_materials: ResMut<Assets<ContourMaterial>>,
	contour_state: Res<ContourState>,
	placeholder_texture: Option<Res<PlaceholderTextureResource>>,
	terrain_query: Query<
		(Entity, &Mesh3d),
		(
			With<terrain::TerrainMesh>,
			Without<ContourMaterialApplied>,
			Without<StandardMaterialApplied>,
		),
	>,
) {
	let placeholder_handle = placeholder_texture
		.map(|r| r.handle.clone())
		.unwrap_or_else(Handle::default);

	let mut count = 0;
	for (entity, mesh_handle) in &terrain_query {
		// Ensure parent keeps StandardMaterial (from lib.rs setup_terrain)
		// Just add the marker to indicate we've processed it
		commands.entity(entity).insert(StandardMaterialApplied);

		// Create a child entity with the same mesh but contour material overlay
		if contour_state.enabled {
			let contour_material = ContourMaterial {
				settings: ContourSettings {
					metallic: 0.0,
					perceptual_roughness: 0.5,
					enabled: true as u32,
					interval: contour_state.interval,
					line_color: Vec3::new(
						contour_state.line_color[0],
						contour_state.line_color[1],
						contour_state.line_color[2],
					),
					line_thickness: contour_state.thickness,
				},
				placeholder_texture: placeholder_handle.clone(),
			};

			let material_handle = contour_materials.add(contour_material);

			// Spawn child entity with contour overlay
			// mesh_handle is &Mesh3d which derefs to Handle<Mesh>
			let child_entity = commands
				.spawn((
					Mesh3d((**mesh_handle).clone()),
					MeshMaterial3d(material_handle),
					ContourMaterialApplied,
				))
				.id();
			// Set parent-child relationship
			commands.entity(entity).add_child(child_entity);
		}
		count += 1;
	}
	if count > 0 {
		debug!(
			"Set up contour overlay for {} terrain mesh(es) (enabled: {})",
			count, contour_state.enabled
		);
	}
}

/// Marker component to indicate terrain has been updated with contour material
#[derive(Component)]
struct ContourMaterialApplied;

/// Marker component to indicate terrain has been updated with standard material
#[derive(Component)]
struct StandardMaterialApplied;

fn contour_controls_ui(mut contexts: EguiContexts, mut contour_state: ResMut<ContourState>) {
	if let Ok(ctx) = contexts.ctx_mut() {
		egui::Window::new("Contour Lines")
			.default_pos(egui::pos2(400.0, 35.0))
			.default_open(false)
			.show(ctx, |ui| {
				ui.heading("Contour Lines");

				// Toggle between contour material and standard material
				// contour_state.enabled controls which material is active
				let material_label = if contour_state.enabled {
					"Showing contour lines"
				} else {
					"Contour lines off"
				};
				ui.label(material_label);

				if ui
					.button(if contour_state.enabled {
						"(M) Turn off contour lines"
					} else {
						"(M) Turn on contour lines"
					})
					.clicked()
				{
					contour_state.enabled = !contour_state.enabled;
					contour_state.needs_update = true;
				}
				ui.separator();

				if !contour_state.enabled {
					ui.label("Contour material is disabled. Enable it above to configure settings.");
				} else {
					// Color picker
					ui.horizontal(|ui| {
						ui.label("Line Color:");
						if ui
							.color_edit_button_rgb(&mut contour_state.line_color)
							.changed()
						{
							contour_state.needs_update = true;
						}
					});

					// Interval slider
					ui.horizontal(|ui| {
						ui.label("Interval:");
						if ui
							.add(egui::Slider::new(&mut contour_state.interval, 1.0..=200.0).suffix(" units"))
							.changed()
						{
							contour_state.needs_update = true;
						}
					});

					// Thickness slider
					ui.horizontal(|ui| {
						ui.label("Thickness:");
						if ui
							.add(egui::Slider::new(&mut contour_state.thickness, 0.001..=0.1))
							.changed()
						{
							contour_state.needs_update = true;
						}
					});
				}
			});
	}
}

fn update_contour_materials(
	mut materials: ResMut<Assets<ContourMaterial>>,
	contour_state: Res<ContourState>,
) {
	// Update existing contour materials when settings change (but not when enabling/disabling)
	// The apply_material_from_contour_state system handles material swapping and resets needs_update
	if contour_state.needs_update && contour_state.enabled {
		let mut count = 0;
		for (_, material) in materials.iter_mut() {
			material.settings.interval = contour_state.interval;
			material.settings.line_color = Vec3::new(
				contour_state.line_color[0],
				contour_state.line_color[1],
				contour_state.line_color[2],
			);
			material.settings.line_thickness = contour_state.thickness;
			count += 1;
		}
		if count > 0 {
			debug!(
				"Updated {} contour material(s). Interval: {}, Thickness: {}, Line Color: {:?}",
				count, contour_state.interval, contour_state.thickness, contour_state.line_color
			);
		}
	}
}

/// System to toggle contour_state.enabled via keyboard (M key)
fn toggle_material_system(
	keyboard_input: Res<ButtonInput<KeyCode>>,
	mut contour_state: ResMut<ContourState>,
) {
	if keyboard_input.just_pressed(KeyCode::KeyM) {
		contour_state.enabled = !contour_state.enabled;
		contour_state.needs_update = true;
		debug!("Toggled contour material to: {}", contour_state.enabled);
	}
}

/// System to toggle contour overlay visibility and update settings
/// Runs when contour_state changes (including needs_update flag)
fn apply_material_from_contour_state(
	mut commands: Commands,
	mut contour_materials: ResMut<Assets<ContourMaterial>>,
	mut contour_state: ResMut<ContourState>,
	placeholder_texture: Res<PlaceholderTextureResource>,
	terrain_query: Query<(Entity, &Mesh3d), With<terrain::TerrainMesh>>,
	contour_children: Query<Entity, (With<ContourMaterialApplied>, With<Mesh3d>)>,
	children_query: Query<&Children>,
) {
	// Only react when needs_update is set (UI sets this when user makes changes)
	if !contour_state.needs_update {
		return;
	}

	for (terrain_entity, mesh_handle) in &terrain_query {
		// Find existing contour overlay child entities for this terrain
		let existing_overlays: Vec<Entity> = children_query
			.get(terrain_entity)
			.map(|children| {
				let mut overlays = Vec::new();
				for entity_ref in children.iter() {
					let entity = entity_ref.clone();
					if contour_children.contains(entity) {
						overlays.push(entity);
					}
				}
				overlays
			})
			.unwrap_or_default();

		if contour_state.enabled {
			// Create overlay if it doesn't exist
			if existing_overlays.is_empty() {
				let contour_material = ContourMaterial {
					settings: ContourSettings {
						metallic: 0.0,
						perceptual_roughness: 0.5,
						enabled: true as u32,
						interval: contour_state.interval,
						line_color: Vec3::new(
							contour_state.line_color[0],
							contour_state.line_color[1],
							contour_state.line_color[2],
						),
						line_thickness: contour_state.thickness,
					},
					placeholder_texture: placeholder_texture.handle.clone(),
				};

				let material_handle = contour_materials.add(contour_material);

				// Spawn child entity with contour overlay
				// mesh_handle is &Mesh3d which derefs to Handle<Mesh>
				let child_entity = commands
					.spawn((
						Mesh3d((**mesh_handle).clone()),
						MeshMaterial3d(material_handle),
						ContourMaterialApplied,
					))
					.id();
				// Set parent-child relationship
				commands.entity(terrain_entity).add_child(child_entity);
				debug!("Created contour overlay child for terrain entity");
			}
			// Material updates happen in update_contour_materials system
		} else {
			// Remove contour overlay children
			for overlay_entity in existing_overlays {
				commands.entity(overlay_entity).despawn();
			}
		}
	}

	debug!(
		"Updated contour overlay (enabled: {})",
		contour_state.enabled
	);

	// Reset needs_update flag after applying changes
	contour_state.bypass_change_detection().needs_update = false;
}

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