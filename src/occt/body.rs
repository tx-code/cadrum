use std::io::{Read, Write};

use super::{body_io, shell::Shell, solid::Solid};
use crate::{Error, ShapeTopology};

/// One exact body exchanged through an OCCT STEP or BRep payload.
///
/// Shells nested inside a solid remain owned by that solid. Independent open
/// or closed shells are returned separately and are never promoted to solids.
pub enum BrepBody {
	Solid(Solid),
	Shell(Shell),
}

impl BrepBody {
	/// Decode explicit solids and independent shells in STEP body order.
	pub fn read_step<R: Read>(reader: &mut R) -> Result<Vec<Self>, Error> {
		body_io::read_step_bodies(reader)
	}

	/// Decode solids and independent shells in one native BRep read.
	pub fn read_brep<R: Read>(reader: &mut R) -> Result<Vec<Self>, Error> {
		body_io::read_brep_bodies(reader)
	}

	/// Encode mixed solids and shells to STEP without changing body kinds.
	pub fn write_step<'a, W: Write>(bodies: impl IntoIterator<Item = &'a Self>, writer: &mut W) -> Result<(), Error> {
		body_io::write_step_bodies(bodies, writer)
	}

	/// Encode mixed solids and shells to one BRep payload in body order.
	pub fn write_brep<'a, W: Write>(bodies: impl IntoIterator<Item = &'a Self>, writer: &mut W) -> Result<(), Error> {
		body_io::write_brep_bodies(bodies, writer)
	}

	/// Return an ordered exact topology snapshot for this body.
	pub fn topology(&self) -> Result<ShapeTopology, Error> {
		match self {
			Self::Solid(solid) => solid.topology(),
			Self::Shell(shell) => shell.topology(),
		}
	}
}
