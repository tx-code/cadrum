use super::ffi;
use crate::common::bspline::{BSplineAxis, BSplineSurface};
use crate::common::trimmed_bspline::{BSplineCurve2, BSplineCurve3};
use glam::{DVec2, DVec3};

pub(crate) fn surface_to_ffi(surface: &BSplineSurface) -> Result<ffi::BSplineSurfaceData, String> {
	let u_count = u32::try_from(surface.u_count).map_err(|_| "u control count exceeds u32".to_string())?;
	let v_count = u32::try_from(surface.v_count).map_err(|_| "v control count exceeds u32".to_string())?;
	Ok(ffi::BSplineSurfaceData {
		control_points: surface.control_points.iter().flat_map(|point| [point.x, point.y, point.z]).collect(),
		weights: surface.weights.clone().unwrap_or_default(),
		u_knots: surface.u.knots.clone(),
		v_knots: surface.v.knots.clone(),
		u_multiplicities: surface.u.multiplicities.clone(),
		v_multiplicities: surface.v.multiplicities.clone(),
		u_count,
		v_count,
		u_degree: surface.u.degree,
		v_degree: surface.v.degree,
		u_periodic: surface.u.periodic,
		v_periodic: surface.v.periodic,
		success: true,
	})
}

pub(crate) fn surface_from_ffi(data: ffi::BSplineSurfaceData) -> Result<BSplineSurface, String> {
	if !data.success || !data.control_points.chunks_exact(3).remainder().is_empty() {
		return Err("OCCT could not expose the face as a B-spline surface".to_string());
	}
	let surface = BSplineSurface {
		control_points: data.control_points.chunks_exact(3).map(|point| DVec3::new(point[0], point[1], point[2])).collect(),
		weights: (!data.weights.is_empty()).then_some(data.weights),
		u_count: data.u_count as usize,
		v_count: data.v_count as usize,
		u: BSplineAxis { degree: data.u_degree, knots: data.u_knots, multiplicities: data.u_multiplicities, periodic: data.u_periodic },
		v: BSplineAxis { degree: data.v_degree, knots: data.v_knots, multiplicities: data.v_multiplicities, periodic: data.v_periodic },
	};
	surface.validate()?;
	Ok(surface)
}

pub(crate) fn curve2_to_ffi(curve: &BSplineCurve2) -> ffi::BSplineCurveData {
	ffi::BSplineCurveData {
		control_points: curve.control_points.iter().flat_map(|point| [point.x, point.y]).collect(),
		weights: curve.weights.clone().unwrap_or_default(),
		knots: curve.axis.knots.clone(),
		multiplicities: curve.axis.multiplicities.clone(),
		degree: curve.axis.degree,
		periodic: curve.axis.periodic,
		first: curve.parameter_range[0],
		last: curve.parameter_range[1],
		dimension: 2,
		success: true,
	}
}

pub(crate) fn curve3_to_ffi(curve: &BSplineCurve3) -> ffi::BSplineCurveData {
	ffi::BSplineCurveData {
		control_points: curve.control_points.iter().flat_map(|point| [point.x, point.y, point.z]).collect(),
		weights: curve.weights.clone().unwrap_or_default(),
		knots: curve.axis.knots.clone(),
		multiplicities: curve.axis.multiplicities.clone(),
		degree: curve.axis.degree,
		periodic: curve.axis.periodic,
		first: curve.parameter_range[0],
		last: curve.parameter_range[1],
		dimension: 3,
		success: true,
	}
}

pub(crate) fn curve2_from_ffi(data: ffi::BSplineCurveData) -> Result<BSplineCurve2, String> {
	if !data.success || data.dimension != 2 || !data.control_points.chunks_exact(2).remainder().is_empty() {
		return Err("OCCT could not expose an exact 2D B-spline curve".to_string());
	}
	let curve = BSplineCurve2 {
		control_points: data.control_points.chunks_exact(2).map(|point| DVec2::new(point[0], point[1])).collect(),
		weights: (!data.weights.is_empty()).then_some(data.weights),
		axis: BSplineAxis { degree: data.degree, knots: data.knots, multiplicities: data.multiplicities, periodic: data.periodic },
		parameter_range: [data.first, data.last],
	};
	curve.validate()?;
	Ok(curve)
}

pub(crate) fn curve3_from_ffi(data: ffi::BSplineCurveData) -> Result<BSplineCurve3, String> {
	if !data.success || data.dimension != 3 || !data.control_points.chunks_exact(3).remainder().is_empty() {
		return Err("OCCT could not expose an exact 3D B-spline curve".to_string());
	}
	let curve = BSplineCurve3 {
		control_points: data.control_points.chunks_exact(3).map(|point| DVec3::new(point[0], point[1], point[2])).collect(),
		weights: (!data.weights.is_empty()).then_some(data.weights),
		axis: BSplineAxis { degree: data.degree, knots: data.knots, multiplicities: data.multiplicities, periodic: data.periodic },
		parameter_range: [data.first, data.last],
	};
	curve.validate()?;
	Ok(curve)
}
