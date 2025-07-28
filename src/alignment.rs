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
        self.alignments
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

#[derive(Debug, Serialize, Deserialize, Default)]
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
    // Find start and end points for the current alignment
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

    // Draw linear alignment (0 turns)
    if alignment_state.turns == 0 {
        gizmos.line(start, end, Color::srgb(0.5, 0.8, 1.0));
        return;
    }

    // Draw multi-turn alignment (1+ turns)
    if let Some(alignment) = alignment_state.alignments.get(&alignment_state.turns) {
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
