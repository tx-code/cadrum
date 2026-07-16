use glam::DVec3;

/// One parametric axis of a tensor-product B-spline surface.
#[derive(Debug, Clone, PartialEq)]
pub struct BSplineAxis {
	pub degree: u32,
	pub knots: Vec<f64>,
	pub multiplicities: Vec<u32>,
	pub periodic: bool,
}

/// Exact tensor-product B-spline surface data.
///
/// Control points and optional weights are V-major with U varying fastest.
#[derive(Debug, Clone, PartialEq)]
pub struct BSplineSurface {
	pub control_points: Vec<DVec3>,
	pub weights: Option<Vec<f64>>,
	pub u_count: usize,
	pub v_count: usize,
	pub u: BSplineAxis,
	pub v: BSplineAxis,
}

impl BSplineSurface {
	pub(crate) fn validate(&self) -> Result<(), String> {
		let count = self.u_count.checked_mul(self.v_count).ok_or_else(|| "control grid dimensions overflow".to_string())?;
		if self.u_count < 2 || self.v_count < 2 {
			return Err("control grid dimensions must both be at least 2".to_string());
		}
		if self.control_points.len() != count {
			return Err(format!("control point count must be {count}, got {}", self.control_points.len()));
		}
		if self.control_points.iter().any(|point| !point.is_finite()) {
			return Err("control points must be finite".to_string());
		}
		if let Some(weights) = &self.weights {
			if weights.len() != count {
				return Err(format!("weight count must be {count}, got {}", weights.len()));
			}
			if weights.iter().any(|weight| !weight.is_finite() || *weight <= 0.0) {
				return Err("weights must be finite and positive".to_string());
			}
		}
		validate_axis("u", self.u_count, &self.u)?;
		validate_axis("v", self.v_count, &self.v)
	}
}

fn validate_axis(name: &str, control_count: usize, axis: &BSplineAxis) -> Result<(), String> {
	if axis.degree == 0 || axis.degree as usize >= control_count {
		return Err(format!("{name} degree must be in 1..{control_count}"));
	}
	if axis.knots.len() < 2 || axis.knots.len() != axis.multiplicities.len() {
		return Err(format!("{name} knots and multiplicities must have the same length of at least 2"));
	}
	if axis.knots.iter().any(|knot| !knot.is_finite()) || axis.knots.windows(2).any(|pair| pair[0] >= pair[1]) {
		return Err(format!("{name} knots must be finite and strictly increasing"));
	}
	if axis.multiplicities.iter().any(|multiplicity| *multiplicity == 0 || *multiplicity > axis.degree + 1) {
		return Err(format!("{name} multiplicities must be in 1..={}", axis.degree + 1));
	}
	if !axis.periodic {
		let expected = control_count.checked_add(axis.degree as usize + 1).ok_or_else(|| format!("{name} knot count overflow"))?;
		let actual = axis.multiplicities.iter().try_fold(0usize, |sum, value| sum.checked_add(*value as usize)).ok_or_else(|| format!("{name} multiplicity sum overflow"))?;
		if actual != expected {
			return Err(format!("{name} multiplicities must sum to {expected}, got {actual}"));
		}
	}
	Ok(())
}
