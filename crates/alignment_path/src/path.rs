use glam::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
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
}

#[derive(Debug, Serialize, Deserialize, Default, Clone, Copy)]
pub struct PathSegment {
	pub tangent_vertex: Vec3,
	pub circular_section_radius: f32,
	pub circular_section_angle: f32,
}

impl PathSegment {
	pub const fn new(tangent_vertex: Vec3) -> Self {
		Self {
			tangent_vertex,
			circular_section_radius: 50.0,
			circular_section_angle: 0.5,
		}
	}
}
