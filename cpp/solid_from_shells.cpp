#include "solid_from_shells.h"

#include <BRepAlgoAPI_Common.hxx>
#include <BRepBuilderAPI_MakeSolid.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepExtrema_DistShapeShape.hxx>
#include <BRepGProp.hxx>
#include <BRepLib.hxx>
#include <BRepTools.hxx>
#include <BRep_Tool.hxx>
#include <GProp_GProps.hxx>
#include <NCollection_IndexedDataMap.hxx>
#include <Precision.hxx>
#include <Standard_Failure.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Edge.hxx>
#include <TopoDS_Face.hxx>
#include <TopoDS_Shell.hxx>
#include <TopoDS_Solid.hxx>
#include <TopTools_ShapeMapHasher.hxx>

#include <algorithm>
#include <cmath>
#include <cstddef>
#include <cstdint>
#include <memory>
#include <vector>

namespace cadrum {
namespace {

constexpr std::uint32_t SOLIDIFY_SUCCESS = 0;
constexpr std::uint32_t SOLIDIFY_INVALID_SHELL = 1;
constexpr std::uint32_t SOLIDIFY_OPEN_SHELL = 2;
constexpr std::uint32_t SOLIDIFY_NON_MANIFOLD_SHELL = 3;
constexpr std::uint32_t SOLIDIFY_BUILD_FAILED = 4;
constexpr std::uint32_t SOLIDIFY_ORIENTATION_FAILED = 5;
constexpr std::uint32_t SOLIDIFY_INVALID_SOLID = 6;
constexpr std::uint32_t SOLIDIFY_NON_POSITIVE_VOLUME = 7;
constexpr std::uint32_t SOLIDIFY_KERNEL_FAILURE = 8;

constexpr std::uint32_t MULTI_EMPTY = 20;
constexpr std::uint32_t MULTI_INVALID_SHELL = 21;
constexpr std::uint32_t MULTI_OPEN_SHELL = 22;
constexpr std::uint32_t MULTI_NON_MANIFOLD_SHELL = 23;
constexpr std::uint32_t MULTI_BUILD_FAILED = 24;
constexpr std::uint32_t MULTI_ORIENTATION_FAILED = 25;
constexpr std::uint32_t MULTI_CAVITY_OUTSIDE = 26;
constexpr std::uint32_t MULTI_SHELL_INTERSECTION = 27;
constexpr std::uint32_t MULTI_INVALID_SOLID = 28;
constexpr std::uint32_t MULTI_NON_POSITIVE_VOLUME = 29;
constexpr std::uint32_t MULTI_KERNEL_FAILURE = 30;

void count_shell_edges(
    const TopoDS_Shell& shell,
    std::size_t& boundary_edges,
    std::size_t& non_manifold_edges)
{
    NCollection_IndexedDataMap<
        TopoDS_Shape,
        NCollection_List<TopoDS_Shape>,
        TopTools_ShapeMapHasher> edge_faces;
    TopExp::MapShapesAndAncestors(shell, TopAbs_EDGE, TopAbs_FACE, edge_faces);
    boundary_edges = 0;
    non_manifold_edges = 0;
    const bool shell_is_closed = BRep_Tool::IsClosed(shell);
    for (int index = 1; index <= edge_faces.Extent(); ++index) {
        const auto& faces = edge_faces(index);
        const int incidence = faces.Extent();
        if (!shell_is_closed && incidence == 1) {
            const TopoDS_Edge edge = TopoDS::Edge(edge_faces.FindKey(index));
            const TopoDS_Face face = TopoDS::Face(faces.First());
            if (!BRepTools::IsReallyClosed(edge, face)) ++boundary_edges;
        } else if (incidence > 2) {
            ++non_manifold_edges;
        }
    }
}

double volume_of(const TopoDS_Shape& shape) {
    GProp_GProps properties;
    BRepGProp::VolumeProperties(shape, properties);
    return properties.Mass();
}

bool positive_solid_from_shell(
    const TopoDS_Shell& shell,
    TopoDS_Solid& solid,
    std::uint32_t& failure)
{
    BRepBuilderAPI_MakeSolid maker(shell);
    if (!maker.IsDone()) {
        failure = MULTI_BUILD_FAILED;
        return false;
    }
    solid = maker.Solid();
    if (!BRepLib::OrientClosedSolid(solid)) {
        failure = MULTI_ORIENTATION_FAILED;
        return false;
    }
    if (!BRepCheck_Analyzer(solid).IsValid()) {
        failure = MULTI_INVALID_SOLID;
        return false;
    }
    const double volume = volume_of(solid);
    if (!std::isfinite(volume) || volume <= 0.0) {
        failure = MULTI_NON_POSITIVE_VOLUME;
        return false;
    }
    return true;
}

TopoDS_Shell first_shell(const TopoDS_Solid& solid) {
    for (TopExp_Explorer explorer(solid, TopAbs_SHELL);
         explorer.More(); explorer.Next()) {
        return TopoDS::Shell(explorer.Current());
    }
    return {};
}

bool shell_distance(
    const TopoDS_Shell& left,
    const TopoDS_Shell& right,
    double& value)
{
    BRepExtrema_DistShapeShape distance(left, right);
    if (!distance.IsDone()) return false;
    value = distance.Value();
    return std::isfinite(value);
}

bool common_volume(
    const TopoDS_Solid& left,
    const TopoDS_Solid& right,
    double& volume)
{
    BRepAlgoAPI_Common common(left, right);
    if (!common.IsDone()) return false;
    volume = volume_of(common.Shape());
    return std::isfinite(volume);
}

} // namespace

void shell_edge_counts(
    const TopoDS_Shell& shell,
    std::size_t& boundary_edges,
    std::size_t& non_manifold_edges)
{
    count_shell_edges(shell, boundary_edges, non_manifold_edges);
}

std::unique_ptr<TopoDS_Shape> checked_solid_from_shell(
    const TopoDS_Shape& shape,
    std::uint32_t& out_status,
    std::size_t& out_detail)
{
    out_status = SOLIDIFY_INVALID_SHELL;
    out_detail = 0;
    try {
        if (shape.IsNull() || shape.ShapeType() != TopAbs_SHELL) return nullptr;
        const TopoDS_Shell shell = TopoDS::Shell(shape);

        std::size_t boundary_edges = 0;
        std::size_t non_manifold_edges = 0;
        count_shell_edges(shell, boundary_edges, non_manifold_edges);
        if (non_manifold_edges != 0) {
            out_status = SOLIDIFY_NON_MANIFOLD_SHELL;
            out_detail = non_manifold_edges;
            return nullptr;
        }
        if (boundary_edges != 0 || !BRep_Tool::IsClosed(shell)) {
            out_status = SOLIDIFY_OPEN_SHELL;
            out_detail = boundary_edges;
            return nullptr;
        }
        if (!BRepCheck_Analyzer(shell).IsValid()) return nullptr;

        out_status = SOLIDIFY_BUILD_FAILED;
        BRepBuilderAPI_MakeSolid maker(shell);
        if (!maker.IsDone()) return nullptr;
        TopoDS_Solid solid = maker.Solid();
        out_status = SOLIDIFY_ORIENTATION_FAILED;
        if (!BRepLib::OrientClosedSolid(solid)) return nullptr;
        out_status = SOLIDIFY_INVALID_SOLID;
        if (!BRepCheck_Analyzer(solid).IsValid()) return nullptr;

        const double volume = volume_of(solid);
        if (!std::isfinite(volume) || volume <= 0.0) {
            out_status = SOLIDIFY_NON_POSITIVE_VOLUME;
            return nullptr;
        }
        out_status = SOLIDIFY_SUCCESS;
        return std::make_unique<TopoDS_Shape>(solid);
    } catch (const Standard_Failure&) {
        out_status = SOLIDIFY_KERNEL_FAILURE;
        return nullptr;
    }
}

std::unique_ptr<TopoDS_Shape> make_solid_from_shell(
    const TopoDS_Shape& shell,
    std::uint32_t& out_status,
    std::size_t& out_detail)
{
    return checked_solid_from_shell(shell, out_status, out_detail);
}

std::unique_ptr<TopoDS_Shape> make_solid_from_shells(
    const std::vector<TopoDS_Shape>& shapes,
    std::uint32_t& out_status,
    std::size_t& out_detail,
    std::size_t& out_related)
{
    out_status = MULTI_EMPTY;
    out_detail = 0;
    out_related = 0;
    if (shapes.empty()) return nullptr;

    try {
        std::vector<TopoDS_Shell> shells;
        shells.reserve(shapes.size());
        for (std::size_t index = 0; index < shapes.size(); ++index) {
            const TopoDS_Shape& shape = shapes[index];
            out_detail = index;
            if (shape.IsNull() || shape.ShapeType() != TopAbs_SHELL) {
                out_status = MULTI_INVALID_SHELL;
                return nullptr;
            }
            const TopoDS_Shell shell = TopoDS::Shell(shape);
            std::size_t boundary_edges = 0;
            std::size_t non_manifold_edges = 0;
            count_shell_edges(shell, boundary_edges, non_manifold_edges);
            if (non_manifold_edges != 0) {
                out_status = MULTI_NON_MANIFOLD_SHELL;
                out_related = non_manifold_edges;
                return nullptr;
            }
            if (boundary_edges != 0 || !BRep_Tool::IsClosed(shell)) {
                out_status = MULTI_OPEN_SHELL;
                out_related = boundary_edges;
                return nullptr;
            }
            if (!BRepCheck_Analyzer(shape).IsValid()) {
                out_status = MULTI_INVALID_SHELL;
                return nullptr;
            }
            shells.push_back(shell);
        }

        std::vector<TopoDS_Solid> positive_solids;
        positive_solids.reserve(shells.size());
        for (std::size_t index = 0; index < shells.size(); ++index) {
            TopoDS_Solid solid;
            out_detail = index;
            if (!positive_solid_from_shell(shells[index], solid, out_status)) {
                return nullptr;
            }
            positive_solids.push_back(solid);
        }

        const double outer_volume = volume_of(positive_solids.front());
        double cavity_volume_sum = 0.0;
        for (std::size_t index = 1; index < shells.size(); ++index) {
            out_detail = index;
            out_related = 0;
            double separation = 0.0;
            if (!shell_distance(shells.front(), shells[index], separation)) {
                out_status = MULTI_KERNEL_FAILURE;
                return nullptr;
            }
            if (separation <= Precision::Confusion()) {
                out_status = MULTI_SHELL_INTERSECTION;
                return nullptr;
            }
            const double cavity_volume = volume_of(positive_solids[index]);
            double overlap = 0.0;
            if (!common_volume(positive_solids.front(), positive_solids[index], overlap)) {
                out_status = MULTI_KERNEL_FAILURE;
                return nullptr;
            }
            const double tolerance = std::max(
                Precision::Confusion() * Precision::Confusion()
                    * Precision::Confusion(),
                std::abs(cavity_volume) * 1.0e-8);
            if (std::abs(overlap - cavity_volume) > tolerance) {
                out_status = MULTI_CAVITY_OUTSIDE;
                return nullptr;
            }
            for (std::size_t previous = 1; previous < index; ++previous) {
                out_related = previous;
                if (!shell_distance(shells[previous], shells[index], separation)) {
                    out_status = MULTI_KERNEL_FAILURE;
                    return nullptr;
                }
                if (separation <= Precision::Confusion()) {
                    out_status = MULTI_SHELL_INTERSECTION;
                    return nullptr;
                }
                double cavity_overlap = 0.0;
                if (!common_volume(
                        positive_solids[previous], positive_solids[index],
                        cavity_overlap)) {
                    out_status = MULTI_KERNEL_FAILURE;
                    return nullptr;
                }
                if (std::abs(cavity_overlap) > tolerance) {
                    out_status = MULTI_SHELL_INTERSECTION;
                    return nullptr;
                }
            }
            cavity_volume_sum += cavity_volume;
        }

        BRepBuilderAPI_MakeSolid maker;
        TopoDS_Shell outer = first_shell(positive_solids.front());
        if (outer.IsNull()) {
            out_status = MULTI_BUILD_FAILED;
            return nullptr;
        }
        maker.Add(outer);
        for (std::size_t index = 1; index < positive_solids.size(); ++index) {
            TopoDS_Shell cavity = first_shell(positive_solids[index]);
            if (cavity.IsNull()) {
                out_status = MULTI_BUILD_FAILED;
                out_detail = index;
                return nullptr;
            }
            cavity.Reverse();
            maker.Add(cavity);
        }
        if (!maker.IsDone()) {
            out_status = MULTI_BUILD_FAILED;
            return nullptr;
        }

        TopoDS_Solid solid = maker.Solid();
        if (!BRepLib::OrientClosedSolid(solid)) {
            out_status = MULTI_ORIENTATION_FAILED;
            return nullptr;
        }
        if (!BRepCheck_Analyzer(solid).IsValid()) {
            out_status = MULTI_INVALID_SOLID;
            return nullptr;
        }
        const double expected_volume = outer_volume - cavity_volume_sum;
        const double actual_volume = volume_of(solid);
        const double volume_tolerance = std::max(
            Precision::Confusion() * Precision::Confusion()
                * Precision::Confusion(),
            std::abs(expected_volume) * 1.0e-8);
        if (!std::isfinite(actual_volume) || expected_volume <= 0.0
            || actual_volume <= 0.0) {
            out_status = MULTI_NON_POSITIVE_VOLUME;
            return nullptr;
        }
        if (std::abs(actual_volume - expected_volume) > volume_tolerance) {
            out_status = MULTI_INVALID_SOLID;
            return nullptr;
        }

        out_status = SOLIDIFY_SUCCESS;
        out_detail = 0;
        out_related = 0;
        return std::make_unique<TopoDS_Shape>(solid);
    } catch (const Standard_Failure&) {
        out_status = MULTI_KERNEL_FAILURE;
        return nullptr;
    }
}

} // namespace cadrum
