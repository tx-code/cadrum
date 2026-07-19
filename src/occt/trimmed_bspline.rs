use super::bspline;
use super::edge::Edge;
use super::face::Face;
use super::ffi;
use crate::common::error::Error;
use crate::common::trimmed_bspline::{BSplineCurve3, TrimEdgeUse, TrimLoop, TrimOrientation, TrimmedBSplineFace};

impl Edge {
	/// Construct an edge from exact rational or non-rational B-spline data.
	pub fn from_bspline_curve(curve: &BSplineCurve3) -> Result<Self, Error> {
		curve.validate().map_err(Error::BsplineFailed)?;
		Self::try_from_ffi(ffi::make_exact_bspline_edge(&bspline::curve3_to_ffi(curve)), "OCCT rejected the exact B-spline curve".to_string())
	}

	/// Extract this edge's exact 3D curve and active parameter range.
	pub fn bspline_curve(&self) -> Result<BSplineCurve3, Error> {
		bspline::curve3_from_ffi(ffi::edge_bspline_curve(&self.inner)).map_err(Error::BsplineFailed)
	}
}

impl Face {
	/// Construct a validated face from exact surface, 3D edges, and ordered p-curves.
	pub fn from_trimmed_bspline_surface(face: &TrimmedBSplineFace) -> Result<Self, Error> {
		face.validate().map_err(Error::TrimmedFaceFailed)?;
		let data = ffi::TrimmedFaceData {
			surface: bspline::surface_to_ffi(&face.surface).map_err(Error::TrimmedFaceFailed)?,
			edges: face.edges.iter().map(bspline::curve3_to_ffi).collect(),
			loops: face
				.loops
				.iter()
				.map(|boundary| ffi::TrimLoopData {
					edges: boundary.edges.iter().map(|edge_use| ffi::TrimEdgeUseData { edge: edge_use.edge as u32, reversed: edge_use.orientation == TrimOrientation::Reversed, pcurve: bspline::curve2_to_ffi(&edge_use.pcurve) }).collect(),
				})
				.collect(),
			tolerance: face.tolerance,
			success: true,
		};
		let mut status = 0;
		let inner = ffi::make_trimmed_bspline_face(&data, &mut status);
		if inner.is_null() {
			let reason = match status {
				1 => "invalid exchange data",
				2 => "invalid B-spline surface",
				3 => "invalid 3D edge curve",
				4 => "invalid p-curve or parameter range",
				5 => "invalid periodic seam occurrences",
				6 => "ordered edges do not form closed wires",
				7 => "3D curve and p-curve are inconsistent on the surface",
				8 => "constructed face topology is invalid",
				_ => "OCCT kernel failure",
			};
			return Err(Error::TrimmedFaceFailed(reason.to_string()));
		}
		Ok(Self::new(inner))
	}

	/// Extract exact surface, unique 3D edges, ordered loops, and p-curves.
	pub fn trimmed_bspline_surface(&self) -> Result<TrimmedBSplineFace, Error> {
		let data = ffi::face_trimmed_bspline_data(&self.inner);
		if !data.success {
			return Err(Error::TrimmedFaceFailed("OCCT could not expose complete trimmed B-spline topology".to_string()));
		}
		let face = TrimmedBSplineFace {
			surface: bspline::surface_from_ffi(data.surface).map_err(Error::TrimmedFaceFailed)?,
			edges: data.edges.into_iter().map(bspline::curve3_from_ffi).collect::<Result<_, _>>().map_err(Error::TrimmedFaceFailed)?,
			loops: data.loops.into_iter().map(|boundary| boundary.edges.into_iter().map(|edge_use| Ok(TrimEdgeUse { edge: edge_use.edge as usize, orientation: if edge_use.reversed { TrimOrientation::Reversed } else { TrimOrientation::Forward }, pcurve: bspline::curve2_from_ffi(edge_use.pcurve)? })).collect::<Result<Vec<_>, String>>().map(|edges| TrimLoop { edges })).collect::<Result<_, _>>().map_err(Error::TrimmedFaceFailed)?,
			tolerance: data.tolerance,
		};
		face.validate().map_err(Error::TrimmedFaceFailed)?;
		Ok(face)
	}
}
