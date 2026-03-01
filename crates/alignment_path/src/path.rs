use glam::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Alignment {
	pub start: Vec3,
	pub end: Vec3,
	#[serde(default)]
	pub segments: Vec<PathSegment>,
}

impl Default for Alignment {
	fn default() -> Self {
		Self {
			start: Vec3::ZERO,
			end: Vec3::ZERO,
			segments: Vec::new(),
		}
	}
}

impl Alignment {
	pub fn new(start: Vec3, end: Vec3, n_tangents: usize) -> Self {
		let mut segments = Vec::with_capacity(n_tangents);
		if n_tangents > 0 {
			for i in 1..=n_tangents {
				let s = i as f32 / (n_tangents + 1) as f32;
				let vertex = start.lerp(end, s);
				segments.push(PathSegment::Turn(TurnSegment::new(vertex)));
			}
		}
		Self {
			start,
			end,
			segments,
		}
	}

	pub fn turn_count(&self) -> usize {
		self
			.segments
			.iter()
			.filter(|segment| matches!(segment, PathSegment::Turn(_)))
			.count()
	}

	pub fn append_segment_boundary(&mut self, point: Vec3) {
		if self
			.segments
			.last()
			.map(PathSegment::control_point)
			.is_some_and(|last| last == point)
		{
			return;
		}
		if self.end == point {
			self
				.segments
				.push(PathSegment::Straight(StraightSegment { point }));
		}
	}

	pub fn append_turn(&mut self, tangent_vertex: Vec3) {
		self
			.segments
			.push(PathSegment::Turn(TurnSegment::new(tangent_vertex)));
	}
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
#[serde(untagged)]
pub enum PathSegment {
	Straight(StraightSegment),
	Turn(TurnSegment),
}

impl PathSegment {
	pub fn control_point(&self) -> Vec3 {
		match self {
			Self::Straight(segment) => segment.point,
			Self::Turn(segment) => segment.tangent_vertex,
		}
	}

	pub fn set_control_point(&mut self, point: Vec3) {
		match self {
			Self::Straight(segment) => segment.point = point,
			Self::Turn(segment) => segment.tangent_vertex = point,
		}
	}

	pub fn as_turn(&self) -> Option<&TurnSegment> {
		match self {
			Self::Turn(segment) => Some(segment),
			Self::Straight(_) => None,
		}
	}

	pub fn as_turn_mut(&mut self) -> Option<&mut TurnSegment> {
		match self {
			Self::Turn(segment) => Some(segment),
			Self::Straight(_) => None,
		}
	}
}

#[derive(Debug, Serialize, Deserialize, Default, Clone, Copy)]
pub struct StraightSegment {
	pub point: Vec3,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone, Copy)]
pub struct TurnSegment {
	pub tangent_vertex: Vec3,
	pub circular_section_radius: f32,
	pub circular_section_angle: f32,
}

impl TurnSegment {
	pub const fn new(tangent_vertex: Vec3) -> Self {
		Self {
			tangent_vertex,
			circular_section_radius: 50.0,
			circular_section_angle: 0.5,
		}
	}
}
