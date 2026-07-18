use std::io::{Read, Write};
use std::sync::OnceLock;

use super::edge::Edge;
use super::face::Face;
use super::{ffi, io};
use crate::Error;

/// One connected OCCT shell, which may be open or closed.
pub struct Shell {
	inner: cxx::UniquePtr<ffi::TopoDS_Shape>,
	edges: OnceLock<Vec<Edge>>,
	faces: OnceLock<Vec<Face>>,
}

impl Shell {
	pub(crate) fn new(inner: cxx::UniquePtr<ffi::TopoDS_Shape>) -> Self {
		debug_assert!(ffi::shape_is_null(&inner) || ffi::shape_is_shell(&inner));
		Self { inner, edges: OnceLock::new(), faces: OnceLock::new() }
	}

	pub(crate) fn inner(&self) -> &ffi::TopoDS_Shape {
		&self.inner
	}

	/// Sew all input faces into exactly one connected shell.
	pub fn sew<'a>(faces: impl IntoIterator<Item = &'a Face>, tolerance: f64) -> Result<Self, Error> {
		if !tolerance.is_finite() || tolerance <= 0.0 {
			return Err(Error::SewFailed("tolerance must be finite and positive".to_string()));
		}
		let mut face_vec = ffi::face_vec_new();
		let mut face_count = 0usize;
		for face in faces {
			ffi::face_vec_push(face_vec.pin_mut(), &face.inner);
			face_count += 1;
		}
		if face_count == 0 {
			return Err(Error::SewFailed("face set is empty".to_string()));
		}
		let inner = ffi::make_sewn_shell(&face_vec, tolerance);
		if inner.is_null() {
			return Err(Error::SewFailed("faces did not form exactly one connected shell".to_string()));
		}
		Ok(Self::new(inner))
	}

	pub fn is_closed(&self) -> bool {
		ffi::shell_is_closed(&self.inner)
	}

	pub fn is_valid(&self) -> bool {
		ffi::shape_is_valid(&self.inner)
	}

	pub fn boundary_edge_count(&self) -> usize {
		ffi::shell_boundary_edge_count(&self.inner)
	}

	pub fn iter_edge(&self) -> impl Iterator<Item = &Edge> + '_ {
		self.edges.get_or_init(|| ffi::shape_edges(&self.inner).iter().map(|edge| Edge::try_from_ffi(ffi::clone_edge_handle(edge), "shell edge is null".into()).expect("shape_edges returned null")).collect()).iter()
	}

	pub fn iter_face(&self) -> impl Iterator<Item = &Face> + '_ {
		self.faces.get_or_init(|| ffi::shape_faces(&self.inner).iter().map(|face| Face::new(ffi::clone_face_handle(face))).collect()).iter()
	}

	pub fn read_brep<R: Read>(reader: &mut R) -> Result<Vec<Self>, Error> {
		io::read_brep_shells(reader)
	}

	pub fn write_brep<'a, W: Write>(shells: impl IntoIterator<Item = &'a Self>, writer: &mut W) -> Result<(), Error> {
		io::write_brep_shells(shells, writer)
	}
}
