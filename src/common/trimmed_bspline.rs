use super::bspline::{validate_axis, BSplineAxis, BSplineSurface};
use glam::{DVec2, DVec3};

/// Exact rational or non-rational B-spline curve in parameter space.
#[derive(Debug, Clone, PartialEq)]
pub struct BSplineCurve2 {
	pub control_points: Vec<DVec2>,
	pub weights: Option<Vec<f64>>,
	pub axis: BSplineAxis,
	pub parameter_range: [f64; 2],
}

impl BSplineCurve2 {
	pub(crate) fn validate(&self) -> Result<(), String> {
		validate_curve("2D curve", self.control_points.len(), self.weights.as_deref(), &self.axis, self.parameter_range, self.control_points.iter().all(|point| point.is_finite()))
	}
}

/// Exact rational or non-rational B-spline curve in model space.
#[derive(Debug, Clone, PartialEq)]
pub struct BSplineCurve3 {
	pub control_points: Vec<DVec3>,
	pub weights: Option<Vec<f64>>,
	pub axis: BSplineAxis,
	pub parameter_range: [f64; 2],
}

impl BSplineCurve3 {
	pub(crate) fn validate(&self) -> Result<(), String> {
		validate_curve("3D curve", self.control_points.len(), self.weights.as_deref(), &self.axis, self.parameter_range, self.control_points.iter().all(|point| point.is_finite()))
	}
}

/// Orientation of one edge occurrence inside an ordered trim loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrimOrientation {
	Forward,
	Reversed,
}

/// One use of a unique 3D edge in a face boundary loop.
#[derive(Debug, Clone, PartialEq)]
pub struct TrimEdgeUse {
	pub edge: usize,
	pub orientation: TrimOrientation,
	pub pcurve: BSplineCurve2,
}

/// One ordered face boundary loop. The outer loop is first in a face exchange.
#[derive(Debug, Clone, PartialEq)]
pub struct TrimLoop {
	pub edges: Vec<TrimEdgeUse>,
}

/// Exact B-spline surface plus its ordered trimming topology.
#[derive(Debug, Clone, PartialEq)]
pub struct TrimmedBSplineFace {
	pub surface: BSplineSurface,
	pub edges: Vec<BSplineCurve3>,
	pub loops: Vec<TrimLoop>,
	pub tolerance: f64,
}

impl TrimmedBSplineFace {
	pub(crate) fn validate(&self) -> Result<(), String> {
		self.surface.validate()?;
		if !self.tolerance.is_finite() || self.tolerance <= 0.0 {
			return Err("trim tolerance must be finite and positive".to_string());
		}
		if self.edges.is_empty() {
			return Err("trim topology must contain at least one 3D edge".to_string());
		}
		if self.edges.len() > u32::MAX as usize {
			return Err("trim topology contains more than u32::MAX edges".to_string());
		}
		if self.loops.is_empty() {
			return Err("trim topology must contain an outer loop".to_string());
		}
		for (index, edge) in self.edges.iter().enumerate() {
			edge.validate().map_err(|reason| format!("edge {index}: {reason}"))?;
		}

		let mut uses = vec![Vec::new(); self.edges.len()];
		for (loop_index, boundary) in self.loops.iter().enumerate() {
			if boundary.edges.is_empty() {
				return Err(format!("trim loop {loop_index} is empty"));
			}
			for (use_index, edge_use) in boundary.edges.iter().enumerate() {
				let Some(edge) = self.edges.get(edge_use.edge) else {
					return Err(format!("trim loop {loop_index} edge use {use_index} references missing edge {}", edge_use.edge));
				};
				edge_use.pcurve.validate().map_err(|reason| format!("trim loop {loop_index} edge use {use_index}: {reason}"))?;
				if !ranges_match(edge.parameter_range, edge_use.pcurve.parameter_range) {
					return Err(format!("trim loop {loop_index} edge use {use_index} does not share the 3D edge parameter range"));
				}
				uses[edge_use.edge].push(edge_use.orientation);
			}
		}

		for (edge_index, orientations) in uses.iter().enumerate() {
			match orientations.as_slice() {
				[] => return Err(format!("3D edge {edge_index} is not used by a trim loop")),
				[_] => {}
				[left, right] if left != right => {}
				[_, _] => return Err(format!("seam edge {edge_index} must occur once in each orientation")),
				_ => return Err(format!("3D edge {edge_index} occurs more than twice")),
			}
		}
		Ok(())
	}
}

fn validate_curve(name: &str, control_count: usize, weights: Option<&[f64]>, axis: &BSplineAxis, parameter_range: [f64; 2], points_finite: bool) -> Result<(), String> {
	if control_count < 2 {
		return Err(format!("{name} must contain at least two control points"));
	}
	if !points_finite {
		return Err(format!("{name} control points must be finite"));
	}
	if let Some(weights) = weights {
		if weights.len() != control_count {
			return Err(format!("{name} weight count must be {control_count}, got {}", weights.len()));
		}
		if weights.iter().any(|weight| !weight.is_finite() || *weight <= 0.0) {
			return Err(format!("{name} weights must be finite and positive"));
		}
	}
	validate_axis(name, control_count, axis)?;
	if !parameter_range[0].is_finite() || !parameter_range[1].is_finite() || parameter_range[0] >= parameter_range[1] {
		return Err(format!("{name} parameter range must be finite and increasing"));
	}
	Ok(())
}

fn ranges_match(left: [f64; 2], right: [f64; 2]) -> bool {
	left.into_iter().zip(right).all(|(left, right)| {
		let scale = left.abs().max(right.abs()).max(1.0);
		(left - right).abs() <= 1.0e-12 * scale
	})
}
