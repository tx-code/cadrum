use std::io::Cursor;

use cadrum::{BSplineAxis, BSplineSurface, DVec3, Face};

fn rational_patch() -> BSplineSurface {
	let mut control_points = Vec::new();
	let mut weights = Vec::new();
	for v in 0..4 {
		for u in 0..4 {
			control_points.push(DVec3::new(u as f64, v as f64, (u * v) as f64 * 0.125));
			weights.push(if u == 1 && v == 2 { 0.75 } else { 1.0 });
		}
	}
	let axis = BSplineAxis { degree: 3, knots: vec![0.0, 1.0], multiplicities: vec![4, 4], periodic: false };
	BSplineSurface { control_points, weights: Some(weights), u_count: 4, v_count: 4, u: axis.clone(), v: axis }
}

fn assert_surface_close(actual: &BSplineSurface, expected: &BSplineSurface) {
	assert_eq!(actual.u_count, expected.u_count);
	assert_eq!(actual.v_count, expected.v_count);
	assert_eq!(actual.u, expected.u);
	assert_eq!(actual.v, expected.v);
	assert_eq!(actual.control_points.len(), expected.control_points.len());
	for (actual, expected) in actual.control_points.iter().zip(&expected.control_points) {
		assert!(actual.abs_diff_eq(*expected, 1.0e-12), "{actual:?} != {expected:?}");
	}
	let actual_weights = actual.weights.as_ref().expect("surface should remain rational");
	let expected_weights = expected.weights.as_ref().expect("fixture is rational");
	for (actual, expected) in actual_weights.iter().zip(expected_weights) {
		assert!((actual - expected).abs() <= 1.0e-12, "{actual} != {expected}");
	}
}

#[test]
fn exact_bspline_face_roundtrips_in_memory() {
	let expected = rational_patch();
	let face = Face::from_bspline_surface(&expected).expect("B-spline face construction");
	assert_eq!(face.boundary_loop_count(), 1);
	assert_eq!(face.outer_boundary_edge_count(), 4);
	assert!(face.uses_natural_surface_bounds());
	let actual = face.bspline_surface().expect("B-spline surface extraction");
	assert_surface_close(&actual, &expected);
}

#[test]
fn exact_bspline_face_roundtrips_through_step_and_brep() {
	let expected = rational_patch();
	let face = Face::from_bspline_surface(&expected).expect("B-spline face construction");

	let mut step = Vec::new();
	Face::write_step([&face], &mut step).expect("STEP face write");
	let step_faces = Face::read_step(&mut Cursor::new(step)).expect("STEP face read");
	assert_eq!(step_faces.len(), 1);
	assert_surface_close(&step_faces[0].bspline_surface().expect("STEP B-spline extraction"), &expected);

	let mut brep = Vec::new();
	Face::write_brep([&face], &mut brep).expect("BRep face write");
	let brep_faces = Face::read_brep(&mut Cursor::new(brep)).expect("BRep face read");
	assert_eq!(brep_faces.len(), 1);
	assert_surface_close(&brep_faces[0].bspline_surface().expect("BRep B-spline extraction"), &expected);
}

#[test]
fn invalid_bspline_surface_fails_before_occt() {
	let mut surface = rational_patch();
	surface.control_points.pop();
	let error = match Face::from_bspline_surface(&surface) {
		Ok(_) => panic!("invalid control count must fail"),
		Err(error) => error,
	};
	assert!(error.to_string().contains("control point count"));
}

#[test]
fn face_writers_reject_empty_models() {
	let mut step = Vec::new();
	let error = Face::write_step(std::iter::empty::<&Face>(), &mut step).expect_err("empty STEP must fail");
	assert_eq!(error.to_string(), "STEP write failed");
	assert!(step.is_empty());
}
