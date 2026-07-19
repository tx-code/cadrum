/// Controls the working tolerance and the absolute topology-tolerance ceiling.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RepairOptions {
	pub tolerance: f64,
	pub maximum_tolerance: f64,
}

impl RepairOptions {
	pub const fn new(tolerance: f64, maximum_tolerance: f64) -> Self {
		Self { tolerance, maximum_tolerance }
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairOperation {
	Sew,
	Heal,
}

/// One zero-based input-to-output topology relation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TopologyHistory {
	pub input_index: usize,
	pub output_index: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RepairReport {
	pub operation: RepairOperation,
	pub changed: bool,
	pub input_face_count: usize,
	pub input_edge_count: usize,
	pub output_face_count: usize,
	pub output_edge_count: usize,
	pub component_count: usize,
	pub boundary_edge_count: usize,
	pub non_manifold_edge_count: usize,
	pub sewing_free_edge_count: usize,
	pub sewing_multiple_edge_count: usize,
	pub sewn_edge_count: usize,
	pub degenerated_shape_count: usize,
	pub deleted_face_count: usize,
	pub requested_tolerance: f64,
	pub effective_tolerance: f64,
	pub maximum_tolerance: f64,
	pub max_input_tolerance: f64,
	pub max_output_tolerance: f64,
	/// Sampled maximum separation of boundary curves that sewing actually merged.
	pub max_detected_seam_gap: Option<f64>,
	pub face_history: Vec<TopologyHistory>,
	pub edge_history: Vec<TopologyHistory>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairFailure {
	InvalidTolerance,
	EmptyInput,
	KernelFailure,
	NoOutput,
	MultipleComponents,
	NonManifoldTopology,
	InvalidTopology,
	ToleranceExceeded,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RepairError {
	pub failure: RepairFailure,
	pub report: Box<RepairReport>,
}

impl std::fmt::Display for RepairError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let operation = match self.report.operation {
			RepairOperation::Sew => "sewing",
			RepairOperation::Heal => "healing",
		};
		let reason = match self.failure {
			RepairFailure::InvalidTolerance => "tolerance must be finite, positive, and no greater than the maximum tolerance",
			RepairFailure::EmptyInput => "no faces given",
			RepairFailure::KernelFailure => "OCCT raised a kernel failure",
			RepairFailure::NoOutput => "OCCT produced no shell",
			RepairFailure::MultipleComponents => "result contains more than one connected shell",
			RepairFailure::NonManifoldTopology => "result contains an edge shared by more than two faces",
			RepairFailure::InvalidTopology => "resulting shell topology is invalid",
			RepairFailure::ToleranceExceeded => "result exceeds the maximum topology tolerance",
		};
		write!(f, "{operation} failed: {reason}")
	}
}

impl std::error::Error for RepairError {}
