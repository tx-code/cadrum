//! The BRep reader must ignore bytes past its own payload — that is what lets the
//! color trailer live there.

use cadrum::{DVec3, Solid, Tessellation};

fn test_box() -> Vec<Solid> {
	vec![Solid::cube(DVec3::ZERO, DVec3::ONE)]
}

#[test]
fn read_brep_with_trailing_garbage() {
	let shape = test_box();

	// Write valid BRep binary
	let mut buf = Vec::new();
	cadrum::Solid::write_brep(&shape, &mut buf).expect("write_brep should succeed");
	let brep_len = buf.len();
	assert!(brep_len > 0);

	// Append 1 KB of garbage
	buf.extend_from_slice(&[0xAB; 1024]);
	assert_eq!(buf.len(), brep_len + 1024);

	// Read back — should succeed despite trailing garbage
	let result = cadrum::Solid::read_brep(&mut buf.as_slice());
	match &result {
		Ok(solids) => {
			assert!(!solids.is_empty(), "should read at least one solid");
			let vol = solids[0].volume();
			assert!((vol - 1.0).abs() < 1e-6, "unit box volume should be ~1.0, got {}", vol);
			println!("BINARY: OK — read {} solid(s), volume={:.6}, brep_len={}, total={}", solids.len(), vol, brep_len, brep_len + 1024);
		}
		Err(e) => {
			println!("FAILED — {:?} (brep_len={}, total={})", e, brep_len, brep_len + 1024);
			panic!("read_brep failed with trailing garbage: {:?}", e);
		}
	}
}

#[test]
fn read_ascii_brep_with_or_without_draw_wrapper() {
	let wrapped = include_bytes!("fixtures/ascii_box.brep");
	let topology_start = wrapped.windows(b"CASCADE Topology".len()).position(|window| window == b"CASCADE Topology").expect("fixture should contain the topology header");
	for source in [wrapped.as_slice(), &wrapped[topology_start..]] {
		assert_ascii_box(source);
	}
}

fn assert_ascii_box(source: &[u8]) {
	let mut input = source;
	let solids = Solid::read_brep(&mut input).expect("ASCII BRep should parse");

	assert_eq!(solids.len(), 1);
	assert!((solids[0].volume() - 1.0).abs() < 1e-6);
	assert_eq!(solids[0].iter_face().count(), 6);
	assert_eq!(solids[0].iter_edge().count(), 12);
	let mesh = Solid::mesh([&solids[0]], Tessellation::default()).expect("ASCII BRep should mesh");
	assert_eq!(mesh.face_indices.len(), mesh.indices.len() / 3);
	let face_ids = solids[0].iter_face().map(|face| face.id()).collect::<Vec<_>>();
	for (&face_index, &face_id) in mesh.face_indices.iter().zip(&mesh.face_ids) {
		assert_eq!(face_ids[face_index as usize], face_id);
	}
	for face_edge in solids[0].iter_face().flat_map(|face| face.iter_edge()) {
		let matches = solids[0].iter_edge().filter(|edge| edge.topology_hash() == face_edge.topology_hash() && edge.is_same(face_edge)).count();
		assert_eq!(matches, 1, "each face edge should resolve to one solid edge occurrence");
	}
}
