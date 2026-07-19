use super::bspline;
use super::edge::Edge;
use super::ffi;
use super::io;
use crate::common::bspline::BSplineSurface;
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
		let data = bspline::surface_to_ffi(surface).map_err(Error::BsplineFailed)?;
		let inner = ffi::make_bspline_face(&data);
		if inner.is_null() {
			return Err(Error::BsplineFailed("OCCT rejected the B-spline surface".to_string()));
		}
		Ok(Self::new(inner))
	}

	/// Extract exact tensor-product B-spline data from this face's surface.
	pub fn bspline_surface(&self) -> Result<BSplineSurface, Error> {
		bspline::surface_from_ffi(ffi::face_bspline_surface(&self.inner)).map_err(Error::BsplineFailed)
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
