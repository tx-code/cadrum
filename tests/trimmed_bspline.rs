use std::io::Cursor;

use cadrum::{BSplineAxis, BSplineCurve2, BSplineCurve3, BSplineSurface, DVec2, DVec3, Edge, Face, Shell, Solid, Tessellation, TrimEdgeUse, TrimLoop, TrimOrientation, TrimmedBSplineFace};

fn linear_axis() -> BSplineAxis {
	BSplineAxis { degree: 1, knots: vec![0.0, 1.0], multiplicities: vec![2, 2], periodic: false }
}

fn plane_surface() -> BSplineSurface {
	BSplineSurface {
		control_points: vec![DVec3::new(0.0, 0.0, 0.0), DVec3::new(1.0, 0.0, 0.0), DVec3::new(0.0, 1.0, 0.0), DVec3::new(1.0, 1.0, 0.0)],
		weights: None,
		u_count: 2,
		v_count: 2,
		u: linear_axis(),
		v: linear_axis(),
	}
}

fn line2(start: DVec2, end: DVec2) -> BSplineCurve2 {
	BSplineCurve2 { control_points: vec![start, end], weights: None, axis: linear_axis(), parameter_range: [0.0, 1.0] }
}

fn line3(start: DVec2, end: DVec2) -> BSplineCurve3 {
	BSplineCurve3 { control_points: vec![DVec3::new(start.x, start.y, 0.0), DVec3::new(end.x, end.y, 0.0)], weights: None, axis: linear_axis(), parameter_range: [0.0, 1.0] }
}

fn add_loop(points: &[DVec2], edges: &mut Vec<BSplineCurve3>) -> TrimLoop {
	let first_edge = edges.len();
	for pair in points.windows(2) {
		edges.push(line3(pair[0], pair[1]));
	}
	TrimLoop { edges: points.windows(2).enumerate().map(|(index, pair)| TrimEdgeUse { edge: first_edge + index, orientation: TrimOrientation::Forward, pcurve: line2(pair[0], pair[1]) }).collect() }
}

fn face_with_hole() -> TrimmedBSplineFace {
	let mut edges = Vec::new();
	let outer = add_loop(&[DVec2::new(0.1, 0.1), DVec2::new(0.9, 0.1), DVec2::new(0.9, 0.9), DVec2::new(0.1, 0.9), DVec2::new(0.1, 0.1)], &mut edges);
	let inner = add_loop(&[DVec2::new(0.4, 0.4), DVec2::new(0.4, 0.6), DVec2::new(0.6, 0.6), DVec2::new(0.6, 0.4), DVec2::new(0.4, 0.4)], &mut edges);
	TrimmedBSplineFace { surface: plane_surface(), edges, loops: vec![outer, inner], tolerance: 1.0e-9 }
}

fn assert_curve3_close(actual: &BSplineCurve3, expected: &BSplineCurve3) {
	assert_eq!(actual.axis, expected.axis);
	assert_eq!(actual.parameter_range, expected.parameter_range);
	assert_eq!(actual.control_points.len(), expected.control_points.len());
	for (actual, expected) in actual.control_points.iter().zip(&expected.control_points) {
		assert!(actual.abs_diff_eq(*expected, 1.0e-12), "{actual:?} != {expected:?}");
	}
	let actual_weights = actual.weights.as_ref().expect("curve should remain rational");
	let expected_weights = expected.weights.as_ref().expect("fixture is rational");
	for (actual, expected) in actual_weights.iter().zip(expected_weights) {
		assert!((actual - expected).abs() <= 1.0e-12, "{actual} != {expected}");
	}
}

#[test]
fn exact_rational_bspline_edge_roundtrips() {
	let expected = BSplineCurve3 {
		control_points: vec![DVec3::X, DVec3::new(1.0, 1.0, 0.0), DVec3::Y],
		weights: Some(vec![1.0, std::f64::consts::FRAC_1_SQRT_2, 1.0]),
		axis: BSplineAxis { degree: 2, knots: vec![0.0, 1.0], multiplicities: vec![3, 3], periodic: false },
		parameter_range: [0.0, 1.0],
	};
	let edge = Edge::from_bspline_curve(&expected).expect("exact edge construction");
	assert_curve3_close(&edge.bspline_curve().expect("exact edge extraction"), &expected);
}

#[test]
fn trimmed_face_with_hole_roundtrips_in_memory_and_brep() {
	let expected = face_with_hole();
	let face = Face::from_trimmed_bspline_surface(&expected).expect("trimmed face construction");
	assert_eq!(face.boundary_loop_count(), 2);
	assert_eq!(face.outer_boundary_edge_count(), 4);
	assert!(!face.uses_natural_surface_bounds());
	let shell = Shell::sew([&face], 1.0e-9).expect("single trimmed face shell");
	let mesh = Shell::mesh([&shell], Tessellation { deflection_linear: 0.01, relative_linear: false, ..Default::default() }).expect("trimmed face mesh");
	for triangle in mesh.indices.chunks_exact(3) {
		let center = (mesh.vertices[triangle[0]] + mesh.vertices[triangle[1]] + mesh.vertices[triangle[2]]) / 3.0;
		assert!(center.x <= 0.4 || center.x >= 0.6 || center.y <= 0.4 || center.y >= 0.6, "inner loop must remain a hole: {center:?}");
	}

	let extracted = face.trimmed_bspline_surface().expect("trimmed topology extraction");
	assert_eq!(extracted.edges.len(), 8);
	assert_eq!(extracted.loops.len(), 2);
	assert!(extracted.loops.iter().all(|boundary| boundary.edges.len() == 4));
	Face::from_trimmed_bspline_surface(&extracted).expect("extracted topology reconstruction");

	let mut brep = Vec::new();
	Face::write_brep([&face], &mut brep).expect("trimmed BRep write");
	let faces = Face::read_brep(&mut Cursor::new(brep)).expect("trimmed BRep read");
	assert_eq!(faces.len(), 1);
	let reread = faces[0].trimmed_bspline_surface().expect("reread trim extraction");
	assert_eq!(reread.loops.len(), 2);
	assert_eq!(reread.edges.len(), 8);

	let mut step = Vec::new();
	Face::write_step([&face], &mut step).expect("trimmed STEP write");
	let faces = Face::read_step(&mut Cursor::new(step)).expect("trimmed STEP read");
	assert_eq!(faces.len(), 1);
	let reread = faces[0].trimmed_bspline_surface().expect("STEP trim extraction");
	assert_eq!(reread.loops.len(), 2);
	assert_eq!(reread.edges.len(), 8);
}

#[test]
fn inconsistent_pcurve_is_rejected_without_a_partial_face() {
	let mut data = face_with_hole();
	data.loops[0].edges[0].pcurve.control_points[1].y += 0.05;
	let error = match Face::from_trimmed_bspline_surface(&data) {
		Ok(_) => panic!("curve-on-surface mismatch must fail"),
		Err(error) => error,
	};
	assert!(error.to_string().contains("inconsistent"));
}

#[test]
fn broken_ordered_loop_is_rejected() {
	let mut data = face_with_hole();
	data.loops[0].edges[1].orientation = TrimOrientation::Reversed;
	let error = match Face::from_trimmed_bspline_surface(&data) {
		Ok(_) => panic!("disconnected ordered loop must fail"),
		Err(error) => error,
	};
	assert!(error.to_string().contains("closed wires") || error.to_string().contains("topology is invalid"));
}

#[test]
fn periodic_seam_pcurves_roundtrip_as_two_edge_occurrences() {
	let torus = Solid::torus(3.0, 1.0, DVec3::Z);
	let face = torus.iter_face().next().expect("torus face");
	let data = face.trimmed_bspline_surface().expect("periodic trim extraction");

	let mut uses = vec![Vec::new(); data.edges.len()];
	for edge_use in data.loops.iter().flat_map(|boundary| &boundary.edges) {
		uses[edge_use.edge].push(edge_use);
	}
	let seams: Vec<_> = uses.into_iter().filter(|uses| uses.len() == 2).collect();
	assert!(!seams.is_empty(), "torus must expose periodic seam edges");
	for seam in seams {
		assert_ne!(seam[0].orientation, seam[1].orientation);
		assert_ne!(seam[0].pcurve.control_points, seam[1].pcurve.control_points);
	}

	let rebuilt = Face::from_trimmed_bspline_surface(&data).expect("periodic trim reconstruction");
	assert_eq!(rebuilt.boundary_loop_count(), data.loops.len());
}
