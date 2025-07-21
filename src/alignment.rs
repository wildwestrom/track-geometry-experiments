use bevy::gizmos::config::{GizmoConfigGroup, GizmoConfigStore};
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::render::view::RenderLayers;
use bevy_egui::{EguiContexts, egui};
use serde::{Deserialize, Serialize};

use crate::pin::create_pin;
use crate::saveable::SaveableSettings;
use crate::spatial::world_size_for_height;
use crate::terrain;

const MAX_TURNS: usize = 8;

/// Gizmo configuration for alignment path visualization
#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct AlignmentGizmos;

pub struct AlignmentPlugin;

impl Plugin for AlignmentPlugin {
	fn build(&self, app: &mut App) {
		app.insert_resource(load_alignment())
			// .insert_resource(init_alignments())
			.init_gizmo_group::<AlignmentGizmos>()
			.add_systems(Startup, (startup, configure_gizmos))
			.add_systems(PostStartup, update_pins_from_alignment_state)
			.add_systems(
				Update,
				(
					update_alignment_from_pins,
					update_intermediate_pins,
					update_alignment_from_intermediate_pins,
					draw_alignment_path,
				),
			)
			.add_systems(bevy_egui::EguiPrimaryContextPass, ui);
	}
}

#[derive(Component)]
pub struct PointA;
#[derive(Component)]
pub struct PointB;

#[derive(Component)]
pub struct IntermediatePoint {
	pub alignment_turns: usize,
	pub segment_index: usize,
}

fn configure_gizmos(mut config_store: ResMut<GizmoConfigStore>) {
	let (config, _) = config_store.config_mut::<AlignmentGizmos>();
	config.render_layers = RenderLayers::layer(0); // Only render on 3D camera
	config.depth_bias = -1.0; // Show through terrain
}

fn update_alignment_from_pins(
	point_a: Query<&Transform, With<PointA>>,
	point_b: Query<&Transform, With<PointB>>,
	mut alignment_state: ResMut<AlignmentState>,
) {
	let Ok(a) = point_a.single() else {
		return;
	};
	let Ok(b) = point_b.single() else {
		return;
	};
	let new_start = a.translation;
	let new_end = b.translation;

	// Update existing alignments with new start/end positions while preserving intermediate points
	for alignment in alignment_state.alignments.values_mut() {
		// Only update if positions have actually changed
		if alignment.start != new_start || alignment.end != new_end {
			alignment.start = new_start;
			alignment.end = new_end;
		}
	}
}

fn update_intermediate_pins(
	mut commands: Commands,
	alignment_state: Res<AlignmentState>,
	existing_intermediate_pins: Query<Entity, With<IntermediatePoint>>,
	settings: Res<terrain::Settings>,
	mut last_current_alignment: Local<Option<usize>>,
) {
	// Only update if the current alignment selection has changed
	let current_alignment = alignment_state.current;
	if *last_current_alignment == Some(current_alignment) {
		return;
	}
	*last_current_alignment = Some(current_alignment);

	// Remove all existing intermediate pins
	for entity in existing_intermediate_pins.iter() {
		commands.entity(entity).despawn();
	}

	// If current alignment is linear (0) or no alignment selected, don't spawn any intermediate pins
	if current_alignment == 0 {
		return;
	}

	// Spawn green pins for intermediate points of the current alignment
	if let Some(alignment) = alignment_state.alignments.get(&current_alignment) {
		let world_size = world_size_for_height(&settings);

		for (i, segment) in alignment.segments.iter().enumerate() {
			// Convert world coordinates to normalized coordinates for create_pin
			let normalized_pos = segment.tangent_vertex / world_size;

			commands.queue(create_pin(
				normalized_pos,
				world_size,
				IntermediatePoint {
					alignment_turns: current_alignment,
					segment_index: i,
				},
				Color::srgb(0.0, 1.0, 0.0),
			));
		}
	}
}

fn update_alignment_from_intermediate_pins(
	intermediate_pins: Query<(&Transform, &IntermediatePoint), Changed<Transform>>,
	mut alignment_state: ResMut<AlignmentState>,
) {
	for (transform, intermediate_point) in intermediate_pins.iter() {
		// Get the alignment for this intermediate point
		if let Some(alignment) = alignment_state
			.alignments
			.get_mut(&intermediate_point.alignment_turns)
		{
			// Update the segment's tangent vertex with the pin's current position
			if let Some(segment) = alignment.segments.get_mut(intermediate_point.segment_index) {
				segment.tangent_vertex = transform.translation;
			}
		}
	}
}

fn draw_alignment_path(
	mut gizmos: Gizmos<AlignmentGizmos>,
	alignment_state: Res<AlignmentState>,
	point_a: Query<&Transform, With<PointA>>,
	point_b: Query<&Transform, With<PointB>>,
) {
	let Ok(a) = point_a.single() else { return };
	let Ok(b) = point_b.single() else { return };

	let start = a.translation;
	let end = b.translation;

	if !start.is_finite() || !end.is_finite() || start == end {
		return;
	}

	// Draw linear alignment (0 turns)
	if alignment_state.current == 0 {
		gizmos.line(start, end, Color::srgb(0.5, 0.8, 1.0));
		return;
	}

	// Draw multi-turn alignment (1+ turns)
	if let Some(alignment) = alignment_state.alignments.get(&alignment_state.current) {
		let segments = &alignment.segments;

		if segments.is_empty() {
			return;
		}

		// Draw line from start to first tangent vertex
		let first_vertex = segments[0].tangent_vertex;
		if first_vertex.is_finite() {
			gizmos.line(start, first_vertex, Color::srgb(0.5, 0.8, 1.0));
		}

		// Draw lines between consecutive tangent vertices
		for i in 0..segments.len() - 1 {
			let current_vertex = segments[i].tangent_vertex;
			let next_vertex = segments[i + 1].tangent_vertex;

			if current_vertex.is_finite() && next_vertex.is_finite() {
				gizmos.line(current_vertex, next_vertex, Color::srgb(0.5, 0.8, 1.0));
			}
		}

		// Draw line from last tangent vertex to end
		let last_vertex = segments[segments.len() - 1].tangent_vertex;
		if last_vertex.is_finite() {
			gizmos.line(last_vertex, end, Color::srgb(0.5, 0.8, 1.0));
		}
	}
}

fn ui(
	mut contexts: EguiContexts,
	mut alignment_state: ResMut<AlignmentState>,
	point_a: Query<&Transform, With<PointA>>,
	point_b: Query<&Transform, With<PointB>>,
) {
	if let Ok(ctx) = contexts.ctx_mut() {
		egui::Window::new("Alignment Properties")
			.default_pos((20.0, 225.0))
			.show(ctx, |ui| {
				// Get current start/end positions from pins
				let (start_pos, end_pos) =
					if let (Ok(a), Ok(b)) = (point_a.single(), point_b.single()) {
						(a.translation, b.translation)
					} else {
						(Vec3::ZERO, Vec3::ZERO)
					};

				display_position(ui, "Start (Red)", start_pos);
				display_position(ui, "End (Blue)", end_pos);
				ui.separator();

				render_alignment_selection(ui, &mut alignment_state);
				ui.separator();

				render_vertex_coordinates(ui, &alignment_state);
				ui.separator();

				render_new_alignment_creation(ui, &mut alignment_state, start_pos, end_pos);
				ui.separator();

				let alignment_state: &AlignmentState = &alignment_state;
				alignment_state.handle_save_operation_ui(ui, "Save Alignments");
			});
	}
}

/// Helper function to display a position with consistent formatting
fn display_position(ui: &mut egui::Ui, label: &str, position: Vec3) {
	ui.label(&format!(
		"{}: ({:.2},{:.2},{:.2})",
		label, position.x, position.y, position.z
	));
}

fn render_alignment_selection(ui: &mut egui::Ui, alignment_state: &mut AlignmentState) {
	ui.label("Select Alignment:");
	ui.radio_value(&mut alignment_state.current, 0, "Linear Alignment");

	// Collect alignment keys and sort them
	let mut alignment_keys: Vec<usize> = alignment_state.alignments.keys().cloned().collect();
	alignment_keys.sort();

	for turns in alignment_keys {
		ui.radio_value(
			&mut alignment_state.current,
			turns,
			&format!("{} Turn{}", turns, if turns == 1 { "" } else { "s" }),
		);
	}
}

fn render_vertex_coordinates(ui: &mut egui::Ui, alignment_state: &AlignmentState) {
	if alignment_state.current > 0 {
		if let Some(alignment) = alignment_state.alignments.get(&alignment_state.current) {
			let segments: &[PathSegment] = &alignment.segments;
			ui.label("Vertex Coordinates:");
			for (i, segment) in segments.iter().enumerate() {
				let vertex = segment.tangent_vertex;
				ui.label(&format!(
					"V{}: ({:.2}, {:.2}, {:.2})",
					i + 1,
					vertex.x,
					vertex.y,
					vertex.z
				));
			}
		}
	}
}

fn render_new_alignment_creation(
	ui: &mut egui::Ui,
	alignment_state: &mut AlignmentState,
	start_pos: Vec3,
	end_pos: Vec3,
) {
	ui.label("Create New Alignment:");
	ui.horizontal(|ui| {
		ui.label("Turns:");

		let mut draft_turns = alignment_state.draft_turns;

		// - button (disabled when at minimum)
		if ui
			.add_enabled(draft_turns > 1, egui::Button::new("-"))
			.clicked()
		{
			draft_turns = (draft_turns - 1).max(1);
		}

		// Current value display
		ui.label(format!("{}", draft_turns));

		// + button (disabled when at maximum)
		if ui
			.add_enabled(draft_turns < MAX_TURNS, egui::Button::new("+"))
			.clicked()
		{
			draft_turns += 1;
		}

		// Update the resource with the new value
		alignment_state.draft_turns = draft_turns;

		// Only show the button if an alignment with this number of turns doesn't exist
		if !alignment_state.alignments.contains_key(&draft_turns) {
			if ui.button("Add Alignment").clicked() {
				alignment_state.add_alignment(draft_turns, start_pos, end_pos);
				alignment_state.current = draft_turns;
			}
		}
	});
}

#[derive(Resource, Serialize, Deserialize, Clone)]
struct AlignmentState {
	current: usize,
	alignments: HashMap<usize, Alignment>,
	#[serde(skip)]
	draft_turns: usize,
}

impl Default for AlignmentState {
	fn default() -> Self {
		Self {
			current: 0,
			alignments: HashMap::new(),
			draft_turns: 1,
		}
	}
}

impl AlignmentState {
	fn add_alignment(&mut self, turns: usize, start: Vec3, end: Vec3) {
		self.alignments
			.insert(turns, Alignment::new(start, end, turns));
	}
}

#[derive(Serialize, Deserialize, Clone, Default)]
struct Alignment {
	start: Vec3,
	end: Vec3,
	n_tangents: usize,
	segments: Vec<PathSegment>,
}

impl Alignment {
	fn new(start: Vec3, end: Vec3, n_tangents: usize) -> Self {
		let mut sections = Vec::with_capacity(n_tangents);
		if n_tangents > 0 {
			for i in 1..=n_tangents {
				let s = i as f32 / (n_tangents + 1) as f32;
				let vertex = start.lerp(end, s);
				sections.push(PathSegment::new(vertex));
			}
		}
		Self {
			start,
			end,
			n_tangents,
			segments: sections,
		}
	}
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
struct PathSegment {
	tangent_vertex: Vec3,
	circular_section_radius: f32,
	circular_section_angle: f32,
}

impl PathSegment {
	fn new(tangent_vertex: Vec3) -> Self {
		Self {
			tangent_vertex,
			..default()
		}
	}
}

impl SaveableSettings for AlignmentState {
	fn filename() -> &'static str {
		"alignments.json"
	}
}

fn load_alignment() -> AlignmentState {
	let mut settings = AlignmentState::load_or_default();
	// Ensure draft_turns is always at least 1 to avoid conflict with linear alignment
	settings.draft_turns = settings.draft_turns.max(1);
	settings
}

fn startup(mut commands: Commands, settings: Res<terrain::Settings>) {
	let world_size = world_size_for_height(&settings);
	commands.queue(create_pin(
		Vec3::new(0.45, 0.0, 0.0),
		world_size,
		PointA,
		Color::srgb(1.0, 0.0, 0.0),
	));
	commands.queue(create_pin(
		Vec3::new(-0.45, 0.0, 0.0),
		world_size,
		PointB,
		Color::srgb(0.0, 0.0, 1.0),
	));
}

fn update_pins_from_alignment_state(
	alignment_state: Res<AlignmentState>,
	mut point_a: Query<&mut Transform, (With<PointA>, Without<PointB>)>,
	mut point_b: Query<&mut Transform, (With<PointB>, Without<PointA>)>,
) {
	// Get start/end positions from any alignment (they should all have the same start/end)
	if let Some(alignment) = alignment_state.alignments.values().next() {
		// Only update if we have meaningful start/end positions from loaded state
		if alignment.start != Vec3::ZERO || alignment.end != Vec3::ZERO {
			// Update Point A (start) position
			if let Ok(mut transform_a) = point_a.single_mut() {
				transform_a.translation = alignment.start;
			}

			// Update Point B (end) position
			if let Ok(mut transform_b) = point_b.single_mut() {
				transform_b.translation = alignment.end;
			}
		}
	}
}
