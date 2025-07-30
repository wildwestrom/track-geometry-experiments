use std::f64::consts::PI;

use bevy::gizmos::config::{GizmoConfigGroup, GizmoConfigStore};
use bevy::math::ops::atan2;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::render::view::RenderLayers;
use bevy_egui::{EguiContexts, egui};
use serde::{Deserialize, Serialize};
use spec_math::Fresnel;

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
		app
			.insert_resource(load_alignment())
			// .insert_resource(init_alignments())
			.init_gizmo_group::<AlignmentGizmos>()
			.add_systems(Startup, (startup, configure_gizmos))
			.add_systems(
				PostStartup,
				(update_pins_from_alignment_state, update_alignment_pins),
			)
			.add_systems(
				Update,
				(
					update_alignment_from_pins,
					update_alignment_pins,
					update_alignment_from_intermediate_pins,
					render_alignment_path,
				),
			)
			.add_systems(bevy_egui::EguiPrimaryContextPass, ui);
	}
}

#[derive(Component)]
pub struct AlignmentPoint {
	pub alignment_id: usize, // 0 for linear, 1+ for multi-turn alignments
	pub point_type: PointType,
}

#[derive(PartialEq, Debug)]
pub enum PointType {
	Start,
	End,
	Intermediate { segment_index: usize },
}

// Helper methods for color coding
impl AlignmentPoint {
	pub fn get_color(&self) -> Color {
		match self.point_type {
			PointType::Start => Color::srgb(1.0, 0.0, 0.0), // Red
			PointType::End => Color::srgb(0.0, 0.0, 1.0),   // Blue
			PointType::Intermediate { .. } => Color::srgb(0.0, 1.0, 0.0), // Green
		}
	}
}

fn configure_gizmos(mut config_store: ResMut<GizmoConfigStore>) {
	let (config, _) = config_store.config_mut::<AlignmentGizmos>();
	config.render_layers = RenderLayers::layer(0); // Only render on 3D camera
	config.depth_bias = -1.0; // Show through terrain
}

fn update_alignment_from_pins(
	alignment_pins: Query<(&Transform, &AlignmentPoint), Changed<Transform>>,
	mut alignment_state: ResMut<AlignmentState>,
) {
	// Find start and end points for the current alignment
	let mut start_pos = None;
	let mut end_pos = None;

	for (transform, alignment_point) in alignment_pins.iter() {
		if alignment_point.alignment_id == alignment_state.turns {
			match alignment_point.point_type {
				PointType::Start => start_pos = Some(transform.translation),
				PointType::End => end_pos = Some(transform.translation),
				_ => {}
			}
		}
	}

	let (Some(new_start), Some(new_end)) = (start_pos, end_pos) else {
		return;
	};

	// Update existing alignments with new start/end positions while preserving intermediate points
	for alignment in alignment_state.alignments.values_mut() {
		// Only update if positions have actually changed
		if alignment.start != new_start || alignment.end != new_end {
			alignment.start = new_start;
			alignment.end = new_end;
		}
	}
}

fn update_alignment_pins(
	mut commands: Commands,
	alignment_state: Res<AlignmentState>,
	existing_pins: Query<Entity, With<AlignmentPoint>>,
	settings: Res<terrain::Settings>,
	mut last_current_alignment: Local<Option<usize>>,
) {
	// Only update if the current alignment selection has changed
	let current_alignment = alignment_state.turns;
	if *last_current_alignment == Some(current_alignment) {
		return;
	}
	*last_current_alignment = Some(current_alignment);

	// Remove all existing pins
	for entity in existing_pins.iter() {
		commands.entity(entity).despawn();
	}

	// Get the current alignment data
	if let Some(alignment) = alignment_state.alignments.get(&current_alignment) {
		let world_size = world_size_for_height(&settings);

		// Always spawn start and end pins
		let start_point = AlignmentPoint {
			alignment_id: current_alignment,
			point_type: PointType::Start,
		};
		let start_color = start_point.get_color();
		commands.queue(create_pin(
			alignment.start / world_size,
			world_size,
			start_point,
			start_color,
		));

		let end_point = AlignmentPoint {
			alignment_id: current_alignment,
			point_type: PointType::End,
		};
		let end_color = end_point.get_color();
		commands.queue(create_pin(
			alignment.end / world_size,
			world_size,
			end_point,
			end_color,
		));

		// Spawn intermediate pins for multi-turn alignments
		for (i, segment) in alignment.segments.iter().enumerate() {
			// Convert world coordinates to normalized coordinates for create_pin
			let normalized_pos = segment.tangent_vertex / world_size;

			let alignment_point = AlignmentPoint {
				alignment_id: current_alignment,
				point_type: PointType::Intermediate { segment_index: i },
			};
			let point_color = alignment_point.get_color();
			commands.queue(create_pin(
				normalized_pos,
				world_size,
				alignment_point,
				point_color,
			));
		}
	}
}

fn update_alignment_from_intermediate_pins(
	intermediate_pins: Query<(&Transform, &AlignmentPoint), Changed<Transform>>,
	mut alignment_state: ResMut<AlignmentState>,
) {
	for (transform, intermediate_point) in intermediate_pins.iter() {
		// Only process intermediate points, not start/end points
		if let PointType::Intermediate { segment_index } = intermediate_point.point_type {
			// Get the alignment for this intermediate point
			if let Some(alignment) = alignment_state
				.alignments
				.get_mut(&intermediate_point.alignment_id)
			{
				// Update the segment's tangent vertex with the pin's current position
				if let Some(segment) = alignment.segments.get_mut(segment_index) {
					segment.tangent_vertex = transform.translation;
				}
			}
		}
	}
}

fn ui(
	mut contexts: EguiContexts,
	mut alignment_state: ResMut<AlignmentState>,
	alignment_pins: Query<(&Transform, &AlignmentPoint)>,
) {
	if let Ok(ctx) = contexts.ctx_mut() {
		egui::Window::new("Alignment Properties")
			.default_pos((00.0, 35.0))
			.default_open(false)
			.show(ctx, |ui| {
				// Debug: show current alignment and pin count
				ui.label(format!("Total turns: {}", alignment_state.turns));
				ui.label(format!("Total pins: {}", alignment_pins.iter().count()));

				// Get current start/end positions from pins
				let mut start_pos = Vec3::ZERO;
				let mut end_pos = Vec3::ZERO;

				for (transform, alignment_point) in alignment_pins.iter() {
					if alignment_point.alignment_id == alignment_state.turns {
						match alignment_point.point_type {
							PointType::Start => {
								start_pos = transform.translation;
							}
							PointType::End => {
								end_pos = transform.translation;
							}
							_ => {}
						}
					}
				}

				ui.separator();

				display_position(ui, "Start (Red)", start_pos);
				display_position(ui, "End (Blue)", end_pos);
				ui.separator();

				alignment_selection_ui(ui, &mut alignment_state);
				ui.separator();

				vertex_coordinates_ui(ui, &alignment_state);
				ui.separator();

				alignment_creation_ui(ui, &mut alignment_state, start_pos, end_pos);
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

fn alignment_selection_ui(ui: &mut egui::Ui, alignment_state: &mut AlignmentState) {
	ui.label("Select Alignment:");

	// Use a more efficient approach: iterate directly over keys without cloning
	let mut alignment_keys: Vec<&usize> = alignment_state.alignments.keys().collect();
	alignment_keys.sort();

	for &turns in alignment_keys {
		let mut turns_str = turns.to_string();
		ui.radio_value(
			&mut alignment_state.turns,
			turns,
			match turns {
				0 => "Linear Alignment",
				1 => {
					turns_str.push_str(" Turn");
					&turns_str
				}
				_ => {
					turns_str.push_str(" Turns");
					&turns_str
				}
			},
		);
	}
}

fn vertex_coordinates_ui(ui: &mut egui::Ui, alignment_state: &AlignmentState) {
	if alignment_state.turns > 0 {
		if let Some(alignment) = alignment_state.alignments.get(&alignment_state.turns) {
			let segments: &[PathSegment] = &alignment.segments;
			ui.label("Vertices:");
			for (i, segment) in segments.iter().enumerate() {
				let vertex = segment.tangent_vertex;
				ui.label(&format!(
					"V{}: ({:.2}, {:.2}, {:.2}), Angle: {:.2}, Radius: {:.2}",
					i + 1,
					vertex.x,
					vertex.y,
					vertex.z,
					segment.circular_section_angle,
					segment.circular_section_radius
				));
			}
		}
	}
}

fn alignment_creation_ui(
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
				alignment_state.turns = draft_turns;
			}
		}
	});
}

type Turns = usize;

#[derive(Resource, Serialize, Deserialize)]
pub struct AlignmentState {
	pub turns: Turns,
	pub alignments: HashMap<Turns, Alignment>,
	#[serde(skip)]
	pub draft_turns: Turns,
}

impl Default for AlignmentState {
	fn default() -> Self {
		Self {
			turns: 0,
			alignments: HashMap::new(),
			draft_turns: 1,
		}
	}
}

impl AlignmentState {
	fn add_alignment(&mut self, turns: usize, start: Vec3, end: Vec3) {
		self
			.alignments
			.insert(turns, Alignment::new(start, end, turns));
	}
}

#[derive(Serialize, Deserialize, Default)]
pub struct Alignment {
	pub start: Vec3,
	pub end: Vec3,
	pub n_tangents: usize,
	pub segments: Vec<PathSegment>,
}

impl Alignment {
	pub fn new(start: Vec3, end: Vec3, n_tangents: usize) -> Self {
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

	pub fn sample_elevation_profile(&self, num_samples: usize) -> Vec<(f32, f32)> {
		let mut points = Vec::with_capacity(num_samples);
		// Collect all vertices: start, tangents..., end
		let mut vertices = Vec::with_capacity(self.segments.len() + 2);
		vertices.push(self.start);
		for seg in &self.segments {
			vertices.push(seg.tangent_vertex);
		}
		vertices.push(self.end);

		// Compute total path length
		let mut segment_lengths = Vec::with_capacity(vertices.len() - 1);
		let mut total_length = 0.0;
		for i in 0..vertices.len() - 1 {
			let len = vertices[i].distance(vertices[i + 1]);
			segment_lengths.push(len);
			total_length += len;
		}

		// Sample points at regular intervals along the path
		for i in 0..num_samples {
			let t = i as f32 / (num_samples - 1) as f32;
			let target_dist = t * total_length;

			// Find which segment this falls into
			let mut acc = 0.0;
			let mut seg_idx = 0;
			while seg_idx < segment_lengths.len() && acc + segment_lengths[seg_idx] < target_dist {
				acc += segment_lengths[seg_idx];
				seg_idx += 1;
			}
			let seg_start = vertices[seg_idx];
			let seg_end = vertices[seg_idx + 1];
			let seg_len = segment_lengths[seg_idx];
			let seg_t = if seg_len > 0.0 {
				(target_dist - acc) / seg_len
			} else {
				0.0
			};
			let pos = seg_start.lerp(seg_end, seg_t);
			points.push((target_dist, pos.y));
		}
		points
	}
}

#[derive(Debug, Serialize, Deserialize, Default, Clone, Copy)]
pub struct PathSegment {
	pub tangent_vertex: Vec3,
	pub circular_section_radius: f32,
	pub circular_section_angle: f32,
}

impl PathSegment {
	fn new(tangent_vertex: Vec3) -> Self {
		Self {
			tangent_vertex,
			circular_section_radius: 50.0, // Default minimum radius
			circular_section_angle: 0.5,   // Default angle
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

fn startup(mut alignment_state: ResMut<AlignmentState>, settings: Res<terrain::Settings>) {
	let world_size = world_size_for_height(&settings);

	// Calculate world positions for the initial linear alignment
	let start_world_pos = Vec3::new(0.45, 0.0, 0.0) * world_size;
	let end_world_pos = Vec3::new(-0.45, 0.0, 0.0) * world_size;

	// Create linear alignment (id: 0) in the HashMap if it doesn't exist
	if !alignment_state.alignments.contains_key(&0) {
		alignment_state
			.alignments
			.insert(0, Alignment::new(start_world_pos, end_world_pos, 0));
	}

	// Only set current alignment to 0 if no valid current alignment was loaded
	// or if the loaded current alignment doesn't exist in the alignments map
	if !alignment_state
		.alignments
		.contains_key(&alignment_state.turns)
	{
		alignment_state.turns = 0;
	}
}

fn update_pins_from_alignment_state(
	alignment_state: Res<AlignmentState>,
	mut alignment_pins: Query<(&mut Transform, &AlignmentPoint)>,
) {
	// Get start/end positions from any alignment (they should all have the same start/end)
	if let Some(alignment) = alignment_state.alignments.values().next() {
		// Only update if we have meaningful start/end positions from loaded state
		if alignment.start != Vec3::ZERO || alignment.end != Vec3::ZERO {
			for (mut transform, alignment_point) in alignment_pins.iter_mut() {
				// Only update pins for the current alignment
				if alignment_point.alignment_id == alignment_state.turns {
					match alignment_point.point_type {
						PointType::Start => {
							transform.translation = alignment.start;
						}
						PointType::End => {
							transform.translation = alignment.end;
						}
						_ => {}
					}
				}
			}
		}
	}
}

fn render_alignment_path(
	mut gizmos: Gizmos<AlignmentGizmos>,
	alignment_state: Res<AlignmentState>,
	alignment_pins: Query<(&Transform, &AlignmentPoint)>,
) {
	gizmos.axes(Transform::IDENTITY, 100.0);

	let mut start = None;
	let mut end = None;

	for (transform, alignment_point) in alignment_pins.iter() {
		if alignment_point.alignment_id == alignment_state.turns {
			match alignment_point.point_type {
				PointType::Start => start = Some(transform.translation),
				PointType::End => end = Some(transform.translation),
				_ => {}
			}
		}
	}

	let (Some(start), Some(end)) = (start, end) else {
		return;
	};

	if !start.is_finite() || !end.is_finite() || start == end {
		return;
	}

	if let Some(alignment) = alignment_state.alignments.get(&alignment_state.turns) {
		let segments = {
			let mut full_alignment = Vec::new();
			full_alignment.push(PathSegment::new(start));
			let mut incomplete_alignment = alignment.segments.clone();
			full_alignment.append(&mut incomplete_alignment);
			full_alignment.push(PathSegment::new(end));
			full_alignment
		};

		segments
			.iter()
			.for_each(|s| assert!(s.tangent_vertex.is_finite()));
		assert!(!segments.is_empty());

		// Draw lines between consecutive tangent vertices
		for i in 0..segments.len() - 1 {
			let current_segment = segments[i];
			let next_segment = segments[i + 1];
			let v_i = current_segment.tangent_vertex;
			let v_i_plus_1 = next_segment.tangent_vertex;
			let r_i = current_segment.circular_section_radius;
			let omega_i = current_segment.circular_section_angle;
			let clothoid_curve_iterations = 32;
			let u_i_plus_1 = (&v_i_plus_1 - &v_i).normalize();

			if i > 0 {
				let previous_segment = segments[i - 1];
				let v_i_minus_1 = previous_segment.tangent_vertex;
				let phi_i = {
					let delta_x = v_i.x - v_i_minus_1.x;
					let delta_z = v_i.z - v_i_minus_1.z;
					let angle = atan2(delta_z, delta_x);
					-angle // we negate to switch from counterclockwise to clockwise
				};
				let phi_i_plus_1 = {
					let delta_x = &v_i_plus_1.x - v_i.x;
					let delta_z = &v_i_plus_1.z - v_i.z;
					let angle = atan2(delta_z, delta_x);
					-angle // we negate to switch from counterclockwise to clockwise
				};
				let mut theta_i = phi_i_plus_1 - phi_i;
				// Ensure theta_i is positive and handle angle wrapping
				if theta_i < 0.0 {
					theta_i += 2.0 * PI as f32;
				}
				if theta_i > PI as f32 {
					theta_i = 2.0 * PI as f32 - theta_i;
				}
				let l_c = r_i * (theta_i - omega_i);

				let u_i = (&v_i - &v_i_minus_1).normalize();

				gizmos.arc_3d(
					phi_i,
					150.0, // radius
					Isometry3d::new(
						segments[i].tangent_vertex,
						Quat::from_axis_angle(Vec3::Y, 0.),
					),
					Color::srgb(0.9, 1.0, 0.2), // yellow
				);
				// azimuth reference line
				gizmos.line(
					v_i,
					v_i + Vec3::ZERO.with_x(175.0),
					Color::srgb(1.0, 0.8, 0.4), // orange
				);

				// difference in azimuths
				gizmos.arc_3d(
					theta_i,
					200.0, // radius
					Isometry3d::new(
						segments[i].tangent_vertex,
						Quat::from_axis_angle(Vec3::Y, phi_i),
					),
					Color::srgb(0.6, 0.0, 1.0), // purple
				);

				// ------ BEGIN IN-JUNCTION CALCULATION ------
				let theta_i_abs = theta_i.abs() as f64;
				let omega_i_abs = omega_i.abs() as f64;
				let r_i_abs = r_i.abs() as f64;
				let l_c_abs = l_c.abs() as f64;
				let clothoid_angle = theta_i_abs - omega_i_abs;

				let fresnel_arg = (l_c_abs / (PI * r_i_abs)).sqrt();
				let fresnel_scale = (PI * r_i_abs * l_c_abs).sqrt();

				let fresnel = fresnel_arg.fresnel();
				let pf_i = fresnel_scale * fresnel.s;
				let tp_i = fresnel_scale * fresnel.c;

				let cos_half_clothoid_angle = (clothoid_angle / 2.0).cos();
				let sin_half_omega = (omega_i_abs / 2.0).sin();
				let sin_half_interior_angle = ((PI - theta_i_abs) / 2.0).sin();

				let ph_i = pf_i * (clothoid_angle / 2.0).tan();
				let hv_i =
					(r_i_abs + pf_i / cos_half_clothoid_angle) * (sin_half_omega / sin_half_interior_angle);

				let total_tangent_length = tp_i + ph_i + hv_i;

				let t_i = v_i - (total_tangent_length as f32) * u_i;

				gizmos.sphere(
					Isometry3d::from_translation(t_i),
					10.0,
					Color::srgb(1.0, 1.0, 0.0),
				);
				// ------ END IN-JUNCTION CALCULATION ------

				// ------ BEGIN IN-CLOTHOID CALCULATION ------
				// Calculate clothoid parameters
				let r_i_abs = r_i.abs() as f64;
				let l_c_abs = l_c.abs() as f64;

				// Cross product Y component for x-z plane to determine orientation
				let cross_y = u_i.x * u_i_plus_1.z - u_i.z * u_i_plus_1.x;
				let lambda_i = if cross_y >= 0.0 { 1.0_f64 } else { -1.0_f64 };

				let beta_i = u_i.z.atan2(u_i.x) as f64;

				let inner = (PI * r_i_abs * l_c_abs) / lambda_i;
				let fresnel_scale = inner.abs().sqrt();
				let fresnel_scale_sign = inner.signum();
				#[allow(non_snake_case)]
				let ingoing_clothoid = FunctionCurve::new(Interval::UNIT, |s| {
					let tilde_s = s as f64 * l_c_abs;

					let fresnel_arg = tilde_s / fresnel_scale;

					let fresnel = fresnel_arg.fresnel();

					let I_x = (fresnel_scale
						* ((beta_i * fresnel_scale_sign).cos() * fresnel.c
							- (beta_i * fresnel_scale_sign).sin() * fresnel.s)) as f32;
					let I_z = (fresnel_scale_sign
						* fresnel_scale
						* ((beta_i * fresnel_scale_sign).sin() * fresnel.c
							+ (beta_i * fresnel_scale_sign).cos() * fresnel.s)) as f32;

					t_i + Vec3::new(I_x, 0.0, I_z)
				});

				gizmos.curve_3d(
					ingoing_clothoid,
					(0..=clothoid_curve_iterations).map(|i| i as f32 / clothoid_curve_iterations as f32),
					Color::srgb(0.5, 1.0, 0.0),
				);
				// ------ END IN-CLOTHOID CALCULATION ------

				// ------ BEGIN CIRCULAR ARC CALCULATION ------
				let h_i = v_i - (hv_i as f32) * u_i;

				#[allow(non_snake_case)]
				let f_i = {
					let fresnel_arg = l_c_abs / fresnel_scale;
					let fresnel = fresnel_arg.fresnel();

					let I_x = (fresnel_scale
						* ((beta_i * fresnel_scale_sign).cos() * fresnel.c
							- (beta_i * fresnel_scale_sign).sin() * fresnel.s)) as f32;
					let I_z = (fresnel_scale_sign
						* fresnel_scale
						* ((beta_i * fresnel_scale_sign).sin() * fresnel.c
							+ (beta_i * fresnel_scale_sign).cos() * fresnel.s)) as f32;

					t_i + Vec3::new(I_x, 0.0, I_z)
				};
				// At the end of the ingoing clothoid, the tangent has rotated by half the total
				// turn angle
				let clothoid_angle_change = lambda_i * (theta_i as f64 - omega_i as f64) / 2.0;
				let clothoid_end_tangent_angle = beta_i + clothoid_angle_change;

				// w_i should be perpendicular to the tangent at f_i, pointing toward the arc center
				// For a left turn (lambda_i > 0), center is to the left of the tangent direction
				// For a right turn (lambda_i < 0), center is to the right of the tangent direction
				let w_i = if lambda_i > 0.0 {
					// Left turn: rotate tangent direction 90° counter-clockwise
					Vec3::new(
						-(clothoid_end_tangent_angle.sin() as f32),
						0.0,
						clothoid_end_tangent_angle.cos() as f32,
					)
				} else {
					// Right turn: rotate tangent direction 90° clockwise
					Vec3::new(
						clothoid_end_tangent_angle.sin() as f32,
						0.0,
						-(clothoid_end_tangent_angle.cos() as f32),
					)
				};

				let o_i = f_i + (r_i as f32) * w_i;
				// Calculate the vector from arc center to starting point
				let start_vector = f_i - o_i;
				// Bevy's arc_3d starts from positive X direction, so we need the angle from +X to
				// our start vector
				let alpha_i = start_vector.z.atan2(start_vector.x);

				// gizmos.circle(
				//     Isometry3d::new(o_i, Quat::from_axis_angle(Vec3::X, 90_f32.to_radians())),
				//     r_i,
				//     Color::srgba(1.0, 0.7, 0.9, 0.3), // hot pink
				// );
				// Arc direction should match turn direction
				let arc_sweep = -lambda_i.signum() as f32 * omega_i;
				gizmos.arc_3d(
					arc_sweep,
					r_i,
					Isometry3d::new(o_i, Quat::from_axis_angle(Vec3::Y, -alpha_i)),
					Color::srgb(1.0, 0.0, 0.0), // red
				);

				// Draw a line from center to where the arc should start
				gizmos.line(o_i, f_i, Color::srgb(1.0, 0.5, 0.0)); // orange line

				// Mark the clothoid endpoint
				gizmos.sphere(
					Isometry3d::from_translation(f_i),
					8.0,
					Color::srgb(0.0, 1.0, 1.0), // cyan
				);
				// ------ END CIRCULAR ARC CALCULATION ------

				// ------ BEGIN OUT-JUNCTION CALCULATION ------
				let c_i = v_i + (total_tangent_length as f32) * u_i_plus_1;

				gizmos.sphere(
					Isometry3d::from_translation(c_i),
					10.0,
					Color::srgb(1.0, 1.0, 0.0),
				);
				// ------ END OUT-JUNCTION CALCULATION ------

				// ------ BEGIN OUT-CLOTHOID CALCULATION ------
				// For outgoing clothoid, beta should be angle between u_{i+1} and OX+
				let beta_i = u_i_plus_1.z.atan2(u_i_plus_1.x) as f64;
				dbg!(beta_i);
				#[allow(non_snake_case)]
				let outgoing_clothoid = FunctionCurve::new(Interval::UNIT, |s| {
					let tilde_s = -s as f64 * l_c_abs;

					let fresnel_arg = tilde_s / fresnel_scale;

					let fresnel = fresnel_arg.fresnel();

					let I_x = (fresnel_scale
						* ((beta_i * fresnel_scale_sign).cos() * fresnel.c
							+ (beta_i * fresnel_scale_sign).sin() * fresnel.s)) as f32;
					let I_z = (fresnel_scale_sign
						* fresnel_scale
						* ((beta_i * fresnel_scale_sign).sin() * fresnel.c
							- (beta_i * fresnel_scale_sign).cos() * fresnel.s)) as f32;

					c_i + Vec3::new(I_x, 0.0, I_z)
				});

				gizmos.curve_3d(
					outgoing_clothoid,
					(0..=clothoid_curve_iterations).map(|i| i as f32 / clothoid_curve_iterations as f32),
					Color::srgb(0.5, 1.0, 0.0),
				);
				// ------ END OUT-CLOTHOID CALCULATION ------
			}

			let len = 300.0;
			let end_1 = v_i - u_i_plus_1 * len;
			let end_2 = v_i_plus_1 + u_i_plus_1 * len;
			gizmos.line(end_1, end_2, Color::srgb(0.5, 0.8, 1.0));
		}
	}
}
