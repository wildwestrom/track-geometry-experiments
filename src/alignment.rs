use std::f64::consts::PI;

use bevy::prelude::*;

mod components;
mod constraints;
mod render;
mod state;
mod systems;
mod ui;

pub(crate) use alignment_path::constraints::{MAX_ARC_RADIUS, MIN_ARC_RADIUS};
pub(crate) use components::{AlignmentGizmos, configure_gizmos};
pub(crate) use state::{TrackBuildingMode, load_alignment};

pub(crate) const MAX_TURNS: usize = 8;
pub(crate) const FRAC_PI_180: f64 = PI / 180.;
pub(crate) const MAX_GEOMETRY_DEBUG_LEVEL: u8 = 3;

#[derive(Resource)]
pub(crate) struct GeometryDebugLevel(pub u8);

pub struct AlignmentPlugin;

impl Plugin for AlignmentPlugin {
	fn build(&self, app: &mut App) {
		app
			.insert_resource(load_alignment())
			.insert_resource(GeometryDebugLevel(2))
			.init_resource::<TrackBuildingMode>()
			.init_resource::<state::DraftAlignment>()
			.init_gizmo_group::<AlignmentGizmos>()
			.add_systems(Startup, (state::startup, configure_gizmos))
			.add_systems(
				PostStartup,
				(
					systems::update_pins_from_alignment_state,
					systems::update_alignment_pins,
				),
			)
			.add_systems(
				Update,
				(
					systems::update_alignment_from_pins,
					systems::update_alignment_from_intermediate_pins,
					constraints::enforce_alignment_constraints,
					systems::update_pins_from_alignment_state,
					systems::update_alignment_pins,
					render::render_alignment_path,
					(
						systems::toggle_track_building_mode,
						systems::commit_first_segment,
						systems::place_initial_point,
					)
						.chain(),
				),
			)
			.add_systems(bevy_egui::EguiPrimaryContextPass, ui::ui);
	}
}
