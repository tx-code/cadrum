#pragma once

#include <TopoDS_Shape.hxx>

#include <cstddef>
#include <cstdint>
#include <memory>
#include <vector>

class TopoDS_Shell;

namespace cadrum {

using TopoDS_Shape = ::TopoDS_Shape;

void shell_edge_counts(
    const ::TopoDS_Shell& shell,
    std::size_t& boundary_edges,
    std::size_t& non_manifold_edges);

/// Shared checked implementation used by STEP recovery and the public
/// single-shell promotion boundary.
std::unique_ptr<TopoDS_Shape> checked_solid_from_shell(
    const TopoDS_Shape& shell,
    std::uint32_t& out_status,
    std::size_t& out_detail);

std::unique_ptr<TopoDS_Shape> make_solid_from_shell(
    const TopoDS_Shape& shell,
    std::uint32_t& out_status,
    std::size_t& out_detail);

/// Construct one finite solid from an outer shell followed by zero or more
/// cavity shells. The implementation validates closure, manifoldness,
/// containment, shell separation, orientation, and final positive volume.
std::unique_ptr<TopoDS_Shape> make_solid_from_shells(
    const std::vector<TopoDS_Shape>& shells,
    std::uint32_t& out_status,
    std::size_t& out_detail,
    std::size_t& out_related);

} // namespace cadrum
