use std::collections::BTreeSet;
use std::io::Cursor;

use cadrum::{BSplineAxis, BSplineSurface, DVec3, Face, RepairFailure, RepairOperation, RepairOptions, Shell, Solid};

fn source_indices(history: &[cadrum::TopologyHistory]) -> BTreeSet<usize> {
	history.iter().map(|relation| relation.input_index).collect()
}

fn detached_face(face: &Face) -> Face {
	let mut brep = Vec::new();
	Face::write_brep([face], &mut brep).expect("face BRep write");
	Face::read_brep(&mut Cursor::new(brep)).expect("face BRep read").pop().expect("one detached face")
}

fn detached_adjacent_faces() -> (Face, Face) {
	let cube = Solid::cube(DVec3::ZERO, DVec3::ONE);
	let faces: Vec<_> = cube.iter_face().collect();
	let (left, right) = faces.iter().enumerate().flat_map(|(left_index, left)| faces.iter().skip(left_index + 1).map(move |right| (*left, *right))).find(|(left, right)| left.iter_edge().any(|left_edge| right.iter_edge().any(|right_edge| left_edge.is_same(right_edge)))).expect("adjacent cube faces");
	(detached_face(left), detached_face(right))
}

fn planar_face(origin: DVec3, u: DVec3, v: DVec3) -> Face {
	let axis = || BSplineAxis { degree: 1, knots: vec![0.0, 1.0], multiplicities: vec![2, 2], periodic: false };
	Face::from_bspline_surface(&BSplineSurface { control_points: vec![origin, origin + u, origin + v, origin + u + v], weights: None, u_count: 2, v_count: 2, u: axis(), v: axis() }).expect("planar face")
}

#[test]
fn sewn_detached_faces_report_topology_tolerances_gap_and_sources() {
	let (left, right) = detached_adjacent_faces();
	let options = RepairOptions::new(1.0e-7, 1.0e-4);
	let (shell, report) = Shell::sew_with_report([&left, &right], options).expect("reported face sewing");

	assert_eq!(report.operation, RepairOperation::Sew);
	assert!(report.changed);
	assert_eq!((report.input_face_count, report.output_face_count), (2, 2));
	assert_eq!((report.input_edge_count, report.output_edge_count), (8, 7));
	assert_eq!(report.component_count, 1);
	assert_eq!(report.boundary_edge_count, 6);
	assert_eq!(report.non_manifold_edge_count, 0);
	assert_eq!(report.sewing_multiple_edge_count, 0);
	assert_eq!(report.sewn_edge_count, 1);
	assert!(report.max_detected_seam_gap.expect("merged seam gap") <= options.tolerance);
	assert!(report.max_output_tolerance <= options.maximum_tolerance);
	assert_eq!(source_indices(&report.face_history), BTreeSet::from([0, 1]));
	assert_eq!(source_indices(&report.edge_history), (0..8).collect::<BTreeSet<_>>());
	assert!(!shell.is_closed());
	assert!(shell.is_valid());
}

#[test]
fn three_faces_sharing_one_geometric_edge_report_multiple_use() {
	let first = planar_face(DVec3::ZERO, DVec3::X, DVec3::Y);
	let second = planar_face(DVec3::ZERO, DVec3::Z, DVec3::Y);
	let third = planar_face(DVec3::ZERO, DVec3::new(1.0, 0.0, 1.0), DVec3::Y);
	let result = Shell::sew_with_report([&first, &second, &third], RepairOptions::new(1.0e-7, 1.0e-4));
	let report = match result {
		Ok(_) => panic!("non-manifold sewing must not expose a shell"),
		Err(error) => {
			assert_eq!(error.failure, RepairFailure::NonManifoldTopology);
			*error.report
		}
	};

	assert!(report.non_manifold_edge_count > 0);
}

#[test]
fn disconnected_faces_fail_atomically_with_component_report() {
	let left = Solid::cube(DVec3::ZERO, DVec3::ONE);
	let right = Solid::cube(DVec3::splat(3.0), DVec3::splat(4.0));
	let faces = [left.iter_face().next().expect("left face"), right.iter_face().next().expect("right face")];
	let error = Shell::sew_with_report(faces, RepairOptions::new(1.0e-7, 1.0e-4)).err().expect("disconnected faces");

	assert_eq!(error.failure, RepairFailure::MultipleComponents);
	assert_eq!(error.report.component_count, 2);
	assert_eq!((error.report.output_face_count, error.report.output_edge_count), (2, 8));
}

#[test]
fn tolerance_ceiling_rejects_input_before_exposing_output() {
	let cube = Solid::cube(DVec3::ZERO, DVec3::ONE);
	let error = Shell::sew_with_report(cube.iter_face(), RepairOptions::new(1.0e-12, 1.0e-12)).err().expect("input tolerance exceeds ceiling");

	assert_eq!(error.failure, RepairFailure::ToleranceExceeded);
	assert!(error.report.max_input_tolerance > error.report.maximum_tolerance);
	assert_eq!(error.report.output_face_count, 0);
	assert_eq!(error.report.component_count, 0);
}

#[test]
fn healing_copies_shell_and_reports_source_history() {
	let cube = Solid::cube(DVec3::ZERO, DVec3::ONE);
	let source = Shell::sew(cube.iter_face().take(5), 1.0e-7).expect("open shell");
	let source_boundary_edges = source.boundary_edge_count();
	let (healed, report) = source.heal(RepairOptions::new(1.0e-7, 1.0e-4)).expect("reported healing");

	assert_eq!(report.operation, RepairOperation::Heal);
	assert_eq!((report.input_face_count, report.output_face_count), (5, 5));
	assert_eq!((report.input_edge_count, report.output_edge_count), (12, 12));
	assert_eq!(report.component_count, 1);
	assert_eq!(report.boundary_edge_count, source_boundary_edges);
	assert_eq!(source_indices(&report.face_history), (0..5).collect::<BTreeSet<_>>());
	assert_eq!(source_indices(&report.edge_history), (0..12).collect::<BTreeSet<_>>());
	assert!(report.max_detected_seam_gap.is_none());
	assert!(report.max_output_tolerance <= report.maximum_tolerance);
	assert!(healed.is_valid());
	assert_eq!(source.boundary_edge_count(), source_boundary_edges);
}

#[test]
fn failed_healing_leaves_the_source_shell_usable() {
	let cube = Solid::cube(DVec3::ZERO, DVec3::ONE);
	let source = Shell::sew(cube.iter_face().take(5), 1.0e-7).expect("open shell");
	let before = (source.is_valid(), source.boundary_edge_count());
	let error = source.heal(RepairOptions::new(1.0e-12, 1.0e-12)).err().expect("strict ceiling");

	assert_eq!(error.failure, RepairFailure::ToleranceExceeded);
	assert_eq!((source.is_valid(), source.boundary_edge_count()), before);
}
