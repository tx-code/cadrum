use super::edge::Edge;
use super::ffi;
use super::io;
use crate::common::bspline::{BSplineAxis, BSplineSurface};
use crate::common::error::Error;
use crate::traits::FaceStruct;
use glam::DVec3;
use std::io::{Read, Write};
use std::sync::OnceLock;

/// A face topology shape.
///
/// `edges` is a lazy `OnceLock` cache populated on first `iter_edge` call,
/// matching the pattern used by `Solid`. Faces yielded from `Solid::iter_face`
/// are constructed fresh each time the parent solid's face cache is built, so
/// the OnceLock matches the lifetime of the enclosing `Vec<Face>`.
pub struct Face {
	pub(crate) inner: cxx::UniquePtr<ffi::TopoDS_Face>,
	edges: OnceLock<Vec<Edge>>,
}

impl Face {
	/// Create a Face wrapping a `TopoDS_Face`.
	pub(crate) fn new(inner: cxx::UniquePtr<ffi::TopoDS_Face>) -> Self {
		Face { inner, edges: OnceLock::new() }
	}

	/// Construct an untrimmed face from exact tensor-product B-spline data.
	pub fn from_bspline_surface(surface: &BSplineSurface) -> Result<Self, Error> {
		surface.validate().map_err(Error::BsplineFailed)?;
		let u_count = u32::try_from(surface.u_count).map_err(|_| Error::BsplineFailed("u control count exceeds u32".to_string()))?;
		let v_count = u32::try_from(surface.v_count).map_err(|_| Error::BsplineFailed("v control count exceeds u32".to_string()))?;
		let mut points = Vec::with_capacity(surface.control_points.len() * 3);
		for point in &surface.control_points {
			points.extend_from_slice(&[point.x, point.y, point.z]);
		}
		let data = ffi::BSplineSurfaceData {
			control_points: points,
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
		};
		let inner = ffi::make_bspline_face(&data);
		if inner.is_null() {
			return Err(Error::BsplineFailed("OCCT rejected the B-spline surface".to_string()));
		}
		Ok(Self::new(inner))
	}

	/// Extract exact tensor-product B-spline data from this face's surface.
	pub fn bspline_surface(&self) -> Result<BSplineSurface, Error> {
		let data = ffi::face_bspline_surface(&self.inner);
		if !data.success || !data.control_points.chunks_exact(3).remainder().is_empty() {
			return Err(Error::BsplineFailed("OCCT could not expose the face as a B-spline surface".to_string()));
		}
		let surface = BSplineSurface {
			control_points: data.control_points.chunks_exact(3).map(|point| DVec3::new(point[0], point[1], point[2])).collect(),
			weights: (!data.weights.is_empty()).then_some(data.weights),
			u_count: data.u_count as usize,
			v_count: data.v_count as usize,
			u: BSplineAxis { degree: data.u_degree, knots: data.u_knots, multiplicities: data.u_multiplicities, periodic: data.u_periodic },
			v: BSplineAxis { degree: data.v_degree, knots: data.v_knots, multiplicities: data.v_multiplicities, periodic: data.v_periodic },
		};
		surface.validate().map_err(Error::BsplineFailed)?;
		Ok(surface)
	}

	/// Count this face's outer and inner boundary loops.
	pub fn boundary_loop_count(&self) -> usize {
		ffi::face_boundary_loop_count(&self.inner)
	}

	/// Count unique edges in this face's outer boundary loop.
	pub fn outer_boundary_edge_count(&self) -> usize {
		ffi::face_outer_boundary_edge_count(&self.inner)
	}

	/// Whether this face covers the natural parameter bounds of its surface.
	pub fn uses_natural_surface_bounds(&self) -> bool {
		ffi::face_uses_natural_surface_bounds(&self.inner)
	}

	/// Read every face found in a STEP stream, including open surface models.
	pub fn read_step<R: Read>(reader: &mut R) -> Result<Vec<Self>, Error> {
		io::read_step_faces(reader)
	}

	/// Write one or more faces as a STEP model.
	pub fn write_step<'a, W: Write>(faces: impl IntoIterator<Item = &'a Self>, writer: &mut W) -> Result<(), Error> {
		io::write_step_faces(faces, writer)
	}

	/// Read every face found in an OCCT BRep stream.
	pub fn read_brep<R: Read>(reader: &mut R) -> Result<Vec<Self>, Error> {
		io::read_brep_faces(reader)
	}

	/// Write one or more faces as an OCCT BRep model.
	pub fn write_brep<'a, W: Write>(faces: impl IntoIterator<Item = &'a Self>, writer: &mut W) -> Result<(), Error> {
		io::write_brep_faces(faces, writer)
	}
}

impl FaceStruct for Face {
	type Edge = Edge;

	fn id(&self) -> u64 {
		ffi::face_tshape_id(&self.inner)
	}

	fn project(&self, p: DVec3) -> (DVec3, DVec3) {
		let (mut cpx, mut cpy, mut cpz) = (0.0_f64, 0.0_f64, 0.0_f64);
		let (mut nx, mut ny, mut nz) = (0.0_f64, 0.0_f64, 0.0_f64);
		// FFI returns false only on truly catastrophic OCCT failure; for a
		// well-formed face this is effectively unreachable.
		assert!(ffi::face_project_point(&self.inner, p.x, p.y, p.z, &mut cpx, &mut cpy, &mut cpz, &mut nx, &mut ny, &mut nz), "Face::project: BRepExtrema_ExtPF failed (this is a bug)");
		(DVec3::new(cpx, cpy, cpz), DVec3::new(nx, ny, nz))
	}

	fn iter_edge(&self) -> impl Iterator<Item = &Edge> + '_ {
		self.edges
			.get_or_init(|| {
				ffi::face_edges(&self.inner)
					.iter()
					.map(|e_ref| {
						let owned = ffi::clone_edge_handle(e_ref);
						Edge::try_from_ffi(owned, "face_edges: null".into()).expect("face_edges: unexpected null (this is a bug)")
					})
					.collect()
			})
			.iter()
	}
}
