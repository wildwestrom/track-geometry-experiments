use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::elevation::VerticalProfileData;

const DEFAULT_STRAIGHT_FRACTION: f32 = 0.5;
const STRAIGHT_FRACTION_EPSILON: f32 = 1.0e-4;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Alignment {
	pub start: Vec3,
	pub end: Vec3,
	#[serde(default)]
	pub segments: Vec<PathSegment>,
	#[serde(default)]
	pub vertical_profile: VerticalProfileData,
}

impl Default for Alignment {
	fn default() -> Self {
		Self {
			start: Vec3::ZERO,
			end: Vec3::ZERO,
			segments: Vec::new(),
			vertical_profile: VerticalProfileData::default(),
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
			vertical_profile: VerticalProfileData::default(),
		}
	}

	pub fn turn_count(&self) -> usize {
		self
			.segments
			.iter()
			.filter(|segment| matches!(segment, PathSegment::Turn(_)))
			.count()
	}

	pub fn segment_control_point(&self, segment_index: usize) -> Option<Vec3> {
		self.segment_control_point_with_endpoints(segment_index, self.start, self.end)
	}

	pub fn segment_control_point_with_endpoints(
		&self,
		segment_index: usize,
		start: Vec3,
		end: Vec3,
	) -> Option<Vec3> {
		let segment = self.segments.get(segment_index)?;
		Some(match segment {
			PathSegment::Straight(straight) => {
				self.resolve_straight_control_point(segment_index, *straight, start, end)
			}
			PathSegment::Turn(turn) => turn.tangent_vertex,
		})
	}

	pub fn control_points(&self) -> Vec<Vec3> {
		self.control_points_with_endpoints(self.start, self.end)
	}

	pub fn control_points_with_endpoints(&self, start: Vec3, end: Vec3) -> Vec<Vec3> {
		let mut control_points = Vec::with_capacity(self.segments.len() + 2);
		control_points.push(start);
		for (segment_index, _) in self.segments.iter().enumerate() {
			let Some(control_point) =
				self.segment_control_point_with_endpoints(segment_index, start, end)
			else {
				continue;
			};
			control_points.push(control_point);
		}
		control_points.push(end);
		control_points
	}

	pub fn append_segment_boundary(&mut self, point: Vec3, next_anchor: Vec3) {
		if self.end != point {
			return;
		}
		if self
			.segments
			.last()
			.and_then(|_| self.segment_control_point(self.segments.len().saturating_sub(1)))
			.is_some_and(|last| last.distance_squared(point) <= f32::EPSILON)
		{
			return;
		}
		let run_start = self.trailing_straight_run_start();
		let section_start = self.previous_anchor_before_segment_index(run_start, self.start);
		let mut boundary_points: Vec<Vec3> = (run_start..self.segments.len())
			.filter_map(|segment_index| self.segment_control_point(segment_index))
			.collect();
		boundary_points.push(point);

		let max_fraction = 1.0 - STRAIGHT_FRACTION_EPSILON;
		let mut min_fraction = STRAIGHT_FRACTION_EPSILON;

		for (offset, boundary_point) in boundary_points.iter().enumerate() {
			let projected_fraction =
				project_fraction_onto_span(*boundary_point, section_start, next_anchor);
			let remaining = boundary_points.len() - offset - 1;
			let max_for_current =
				(max_fraction - STRAIGHT_FRACTION_EPSILON * remaining as f32).max(min_fraction);
			let normalized_fraction =
				clamp_straight_fraction(projected_fraction).clamp(min_fraction, max_for_current);

			if let Some(straight) = self
				.segments
				.get_mut(run_start + offset)
				.and_then(PathSegment::as_straight_mut)
			{
				straight.set_fraction(normalized_fraction);
			} else {
				self
					.segments
					.push(PathSegment::Straight(StraightSegment::from_fraction(
						normalized_fraction,
					)));
			}
			min_fraction = (normalized_fraction + STRAIGHT_FRACTION_EPSILON).min(max_fraction);
		}
	}

	pub fn set_segment_control_point(&mut self, segment_index: usize, point: Vec3) -> bool {
		let anchors = if matches!(
			self.segments.get(segment_index),
			Some(PathSegment::Straight(_))
		) {
			self.straight_segment_anchors(segment_index, self.start, self.end)
		} else {
			None
		};
		let Some(segment) = self.segments.get_mut(segment_index) else {
			return false;
		};
		match segment {
			PathSegment::Turn(turn) => {
				if turn.tangent_vertex == point {
					return false;
				}
				turn.tangent_vertex = point;
				true
			}
			PathSegment::Straight(straight) => {
				let Some((previous_anchor, next_anchor)) = anchors else {
					return false;
				};
				let fraction = clamp_straight_fraction(project_fraction_onto_span(
					point,
					previous_anchor,
					next_anchor,
				));
				if straight.fraction() == fraction {
					return false;
				}
				straight.set_fraction(fraction);
				true
			}
		}
	}

	pub fn append_turn(&mut self, tangent_vertex: Vec3) {
		self
			.segments
			.push(PathSegment::Turn(TurnSegment::new(tangent_vertex)));
	}

	fn resolve_straight_control_point(
		&self,
		segment_index: usize,
		straight: StraightSegment,
		start: Vec3,
		end: Vec3,
	) -> Vec3 {
		let Some((previous_anchor, next_anchor)) =
			self.straight_segment_anchors(segment_index, start, end)
		else {
			return start;
		};
		let fraction = straight
			.legacy_point()
			.map(|legacy_point| {
				clamp_straight_fraction(project_fraction_onto_span(
					legacy_point,
					previous_anchor,
					next_anchor,
				))
			})
			.unwrap_or_else(|| straight.fraction());
		previous_anchor.lerp(next_anchor, fraction)
	}

	fn straight_segment_anchors(
		&self,
		segment_index: usize,
		start: Vec3,
		end: Vec3,
	) -> Option<(Vec3, Vec3)> {
		if segment_index > self.segments.len() {
			return None;
		}
		let previous_anchor = self.previous_anchor_before_segment_index(segment_index, start);
		let next_anchor = self.next_anchor_after_segment_index(segment_index, end);
		Some((previous_anchor, next_anchor))
	}

	fn previous_anchor_before_segment_index(&self, segment_index: usize, start: Vec3) -> Vec3 {
		self
			.segments
			.iter()
			.take(segment_index)
			.rev()
			.find_map(PathSegment::as_turn)
			.map(|turn| turn.tangent_vertex)
			.unwrap_or(start)
	}

	fn next_anchor_after_segment_index(&self, segment_index: usize, end: Vec3) -> Vec3 {
		self
			.segments
			.iter()
			.skip(segment_index + 1)
			.find_map(PathSegment::as_turn)
			.map(|turn| turn.tangent_vertex)
			.unwrap_or(end)
	}

	fn trailing_straight_run_start(&self) -> usize {
		let mut run_start = self.segments.len();
		while run_start > 0 && matches!(self.segments[run_start - 1], PathSegment::Straight(_)) {
			run_start -= 1;
		}
		run_start
	}
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
#[serde(untagged)]
pub enum PathSegment {
	Straight(StraightSegment),
	Turn(TurnSegment),
}

impl PathSegment {
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

	pub fn as_straight(&self) -> Option<&StraightSegment> {
		match self {
			Self::Straight(segment) => Some(segment),
			Self::Turn(_) => None,
		}
	}

	pub fn as_straight_mut(&mut self) -> Option<&mut StraightSegment> {
		match self {
			Self::Straight(segment) => Some(segment),
			Self::Turn(_) => None,
		}
	}
}

#[derive(Debug, Serialize, Clone, Copy)]
pub struct StraightSegment {
	pub fraction: f32,
	#[serde(skip)]
	legacy_point: Option<Vec3>,
}

impl StraightSegment {
	pub fn from_fraction(fraction: f32) -> Self {
		Self {
			fraction: clamp_straight_fraction(fraction),
			legacy_point: None,
		}
	}

	pub fn fraction(&self) -> f32 {
		clamp_straight_fraction(self.fraction)
	}

	pub fn set_fraction(&mut self, fraction: f32) {
		self.fraction = clamp_straight_fraction(fraction);
		self.legacy_point = None;
	}

	fn legacy_point(&self) -> Option<Vec3> {
		self.legacy_point
	}
}

impl Default for StraightSegment {
	fn default() -> Self {
		Self::from_fraction(DEFAULT_STRAIGHT_FRACTION)
	}
}

impl<'de> Deserialize<'de> for StraightSegment {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>,
	{
		#[derive(Deserialize)]
		#[serde(untagged)]
		enum StraightSegmentSerde {
			Fraction { fraction: f32 },
			LegacyPoint { point: Vec3 },
		}

		match StraightSegmentSerde::deserialize(deserializer)? {
			StraightSegmentSerde::Fraction { fraction } => Ok(Self::from_fraction(fraction)),
			StraightSegmentSerde::LegacyPoint { point } => Ok(Self {
				fraction: DEFAULT_STRAIGHT_FRACTION,
				legacy_point: Some(point),
			}),
		}
	}
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

pub(crate) fn clamp_straight_fraction(fraction: f32) -> f32 {
	if !fraction.is_finite() {
		return DEFAULT_STRAIGHT_FRACTION;
	}
	fraction.clamp(STRAIGHT_FRACTION_EPSILON, 1.0 - STRAIGHT_FRACTION_EPSILON)
}

pub(crate) fn project_fraction_onto_span(point: Vec3, start: Vec3, end: Vec3) -> f32 {
	let span = end - start;
	let span_length_sq = span.length_squared();
	if !span_length_sq.is_finite() || span_length_sq <= f32::EPSILON {
		return DEFAULT_STRAIGHT_FRACTION;
	}
	(point - start).dot(span) / span_length_sq
}
