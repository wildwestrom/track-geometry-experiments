use std::f64::consts::PI;

use bevy::prelude::*;

mod components;
mod constraints;
mod geometry;
mod render;
mod state;
mod systems;
mod ui;

pub(crate) use components::{AlignmentGizmos, configure_gizmos};
pub(crate) use state::load_alignment;

pub(crate) const MAX_TURNS: usize = 8;
pub(crate) const FRAC_PI_180: f64 = PI / 180.;
pub(crate) const MAX_GEOMETRY_DEBUG_LEVEL: u8 = 3;
pub(crate) const MIN_ARC_RADIUS: f32 = 1.0;
pub(crate) const MAX_ARC_RADIUS: f32 = 2000.0;

#[derive(Resource)]
pub(crate) struct GeometryDebugLevel(pub u8);

pub struct AlignmentPlugin;

impl Plugin for AlignmentPlugin {
	fn build(&self, app: &mut App) {
		app
			.insert_resource(load_alignment())
			.insert_resource(GeometryDebugLevel(MAX_GEOMETRY_DEBUG_LEVEL))
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
					constraints::enforce_alignment_constraints,
					systems::update_alignment_from_pins,
					systems::update_alignment_pins,
					systems::update_alignment_from_intermediate_pins,
					render::render_alignment_path,
				),
			)
			.add_systems(bevy_egui::EguiPrimaryContextPass, ui::ui);
	}
}
