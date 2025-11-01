use bevy::camera::visibility::RenderLayers;
use bevy::color::palettes::css::*;
use bevy::gizmos::config::{GizmoConfigGroup, GizmoConfigStore};
use bevy::prelude::*;

#[derive(Component)]
pub(crate) struct AlignmentPoint {
	pub alignment_id: usize, // 0 for linear, 1+ for multi-turn alignments
	pub point_type: PointType,
}

#[derive(PartialEq, Eq, Debug)]
pub(crate) enum PointType {
	Start,
	End,
	Intermediate { segment_index: usize },
}

impl AlignmentPoint {
	pub const fn get_color(&self) -> Color {
		Color::Srgba(match self.point_type {
			PointType::Start => RED,
			PointType::End => BLUE,
			PointType::Intermediate { .. } => LIME,
		})
	}
}

#[derive(Default, Reflect, GizmoConfigGroup)]
pub(crate) struct AlignmentGizmos;

pub(crate) fn configure_gizmos(mut config_store: ResMut<GizmoConfigStore>) {
	let (config, _) = config_store.config_mut::<AlignmentGizmos>();
	config.render_layers = RenderLayers::layer(0);
	config.depth_bias = -1.0;
}
