use super::{face::Face, ffi, shell::Shell};
use crate::{Error, RepairError, RepairFailure, RepairOperation, RepairOptions, RepairReport, TopologyHistory};

fn empty_report(options: RepairOptions) -> ffi::ShapeRepairData {
	ffi::ShapeRepairData {
		status: 3,
		changed: false,
		input_face_count: 0,
		input_edge_count: 0,
		output_face_count: 0,
		output_edge_count: 0,
		component_count: 0,
		boundary_edge_count: 0,
		non_manifold_edge_count: 0,
		sewing_free_edge_count: 0,
		sewing_multiple_edge_count: 0,
		sewn_edge_count: 0,
		degenerated_shape_count: 0,
		deleted_face_count: 0,
		requested_tolerance: options.tolerance,
		effective_tolerance: options.tolerance,
		maximum_tolerance: options.maximum_tolerance,
		max_input_tolerance: 0.0,
		max_output_tolerance: 0.0,
		max_detected_seam_gap: -1.0,
		face_history: Vec::new(),
		edge_history: Vec::new(),
	}
}

fn topology_history(flat: Vec<u32>) -> Vec<TopologyHistory> {
	flat.chunks_exact(2).map(|pair| TopologyHistory { input_index: pair[0] as usize, output_index: pair[1] as usize }).collect()
}

fn public_report(operation: RepairOperation, report: ffi::ShapeRepairData) -> RepairReport {
	RepairReport {
		operation,
		changed: report.changed,
		input_face_count: report.input_face_count,
		input_edge_count: report.input_edge_count,
		output_face_count: report.output_face_count,
		output_edge_count: report.output_edge_count,
		component_count: report.component_count,
		boundary_edge_count: report.boundary_edge_count,
		non_manifold_edge_count: report.non_manifold_edge_count,
		sewing_free_edge_count: report.sewing_free_edge_count,
		sewing_multiple_edge_count: report.sewing_multiple_edge_count,
		sewn_edge_count: report.sewn_edge_count,
		degenerated_shape_count: report.degenerated_shape_count,
		deleted_face_count: report.deleted_face_count,
		requested_tolerance: report.requested_tolerance,
		effective_tolerance: report.effective_tolerance,
		maximum_tolerance: report.maximum_tolerance,
		max_input_tolerance: report.max_input_tolerance,
		max_output_tolerance: report.max_output_tolerance,
		max_detected_seam_gap: (report.max_detected_seam_gap >= 0.0).then_some(report.max_detected_seam_gap),
		face_history: topology_history(report.face_history),
		edge_history: topology_history(report.edge_history),
	}
}

fn failure(status: u32) -> RepairFailure {
	match status {
		1 => RepairFailure::InvalidTolerance,
		2 => RepairFailure::EmptyInput,
		4 => RepairFailure::NoOutput,
		5 => RepairFailure::MultipleComponents,
		6 => RepairFailure::InvalidTopology,
		7 => RepairFailure::ToleranceExceeded,
		8 => RepairFailure::NonManifoldTopology,
		_ => RepairFailure::KernelFailure,
	}
}

impl Shell {
	/// Sew faces into one shell and return topology, tolerance, and source diagnostics.
	pub fn sew_with_report<'a>(faces: impl IntoIterator<Item = &'a Face>, options: RepairOptions) -> Result<(Self, RepairReport), RepairError> {
		let mut face_vec = ffi::face_vec_new();
		for face in faces {
			ffi::face_vec_push(face_vec.pin_mut(), &face.inner);
		}
		let mut native_report = empty_report(options);
		let inner = ffi::sew_faces_with_report(&face_vec, options.tolerance, options.maximum_tolerance, &mut native_report);
		let status = native_report.status;
		let report = public_report(RepairOperation::Sew, native_report);
		if status != 0 || inner.is_null() {
			return Err(RepairError { failure: if status == 0 { RepairFailure::NoOutput } else { failure(status) }, report: Box::new(report) });
		}
		Ok((Self::new(inner), report))
	}

	/// Heal a copy of this shell without mutating the source on failure.
	pub fn heal(&self, options: RepairOptions) -> Result<(Self, RepairReport), RepairError> {
		let mut native_report = empty_report(options);
		let inner = ffi::heal_shell_with_report(self.inner(), options.tolerance, options.maximum_tolerance, &mut native_report);
		let status = native_report.status;
		let report = public_report(RepairOperation::Heal, native_report);
		if status != 0 || inner.is_null() {
			return Err(RepairError { failure: if status == 0 { RepairFailure::NoOutput } else { failure(status) }, report: Box::new(report) });
		}
		Ok((Self::new(inner), report))
	}

	/// Sew all input faces into exactly one connected shell.
	pub fn sew<'a>(faces: impl IntoIterator<Item = &'a Face>, tolerance: f64) -> Result<Self, Error> {
		Self::sew_with_report(faces, RepairOptions::new(tolerance, f64::MAX)).map(|(shell, _)| shell).map_err(|error| Error::SewFailed(error.to_string()))
	}
}
