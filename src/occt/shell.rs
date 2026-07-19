use std::io::{Read, Write};
use std::sync::OnceLock;

use super::edge::Edge;
use super::face::Face;
use super::solid::Solid;
use super::{ffi, io};
use crate::{Error, SolidificationFailure};

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

	/// Promote this shell only after closed-body validation succeeds.
	pub fn try_to_solid(&self) -> Result<Solid, Error> {
		let mut status = 0u32;
		let mut detail = 0usize;
		let inner = ffi::make_solid_from_shell(&self.inner, &mut status, &mut detail);
		if status == 0 && !inner.is_null() {
			return Ok(Solid::new(
				inner,
				#[cfg(feature = "color")]
				std::collections::HashMap::new(),
				Default::default(),
			));
		}
		let failure = match status {
			1 => SolidificationFailure::InvalidShell,
			2 => SolidificationFailure::OpenShell { boundary_edge_count: detail },
			3 => SolidificationFailure::NonManifoldShell { edge_count: detail },
			4 => SolidificationFailure::BuildFailed,
			5 => SolidificationFailure::OrientationFailed,
			6 => SolidificationFailure::InvalidSolid,
			7 => SolidificationFailure::NonPositiveVolume,
			_ => SolidificationFailure::KernelFailure,
		};
		Err(Error::SolidificationFailed(failure))
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

	/// Return an ordered exact topology snapshot for this Shell.
	pub fn topology(&self) -> Result<crate::ShapeTopology, Error> {
		super::topology::snapshot(&self.inner)
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

	/// Read independent shells from STEP without promoting closed shells.
	pub fn read_step<R: Read>(reader: &mut R) -> Result<Vec<Self>, Error> {
		super::body_io::read_step_shells(reader)
	}

	pub fn write_brep<'a, W: Write>(shells: impl IntoIterator<Item = &'a Self>, writer: &mut W) -> Result<(), Error> {
		io::write_brep_shells(shells, writer)
	}

	/// Write shells to STEP while preserving their Shell classification.
	pub fn write_step<'a, W: Write>(shells: impl IntoIterator<Item = &'a Self>, writer: &mut W) -> Result<(), Error> {
		super::body_io::write_step_shells(shells, writer)
	}

	pub fn mesh<'a>(shells: impl IntoIterator<Item = &'a Self>, options: crate::Tessellation) -> Result<crate::Mesh, Error> {
		io::mesh_shells(shells, options)
	}
}
