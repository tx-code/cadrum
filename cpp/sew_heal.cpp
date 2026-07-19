#include "cadrum/src/occt/ffi.rs.h"

#include <BRepAdaptor_Curve.hxx>
#include <BRepBuilderAPI_Copy.hxx>
#include <BRepBuilderAPI_Sewing.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepTools.hxx>
#include <BRep_Tool.hxx>
#include <BRep_Builder.hxx>
#include <NCollection_IndexedDataMap.hxx>
#include <NCollection_IndexedMap.hxx>
#include <Precision.hxx>
#include <ShapeBuild_ReShape.hxx>
#include <ShapeExtend_Status.hxx>
#include <ShapeFix_Shape.hxx>
#include <Standard_Failure.hxx>
#include <TopExp.hxx>
#include <TopExp_Explorer.hxx>
#include <TopTools_ShapeMapHasher.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Compound.hxx>
#include <TopoDS_Shell.hxx>

#include <algorithm>
#include <cmath>
#include <cstdint>
#include <limits>
#include <set>
#include <utility>
#include <vector>

namespace cadrum {
namespace {

constexpr std::uint32_t REPAIR_SUCCESS = 0;
constexpr std::uint32_t REPAIR_INVALID_TOLERANCE = 1;
constexpr std::uint32_t REPAIR_EMPTY_INPUT = 2;
constexpr std::uint32_t REPAIR_KERNEL_FAILURE = 3;
constexpr std::uint32_t REPAIR_NO_OUTPUT = 4;
constexpr std::uint32_t REPAIR_MULTIPLE_COMPONENTS = 5;
constexpr std::uint32_t REPAIR_INVALID_TOPOLOGY = 6;
constexpr std::uint32_t REPAIR_TOLERANCE_EXCEEDED = 7;
constexpr std::uint32_t REPAIR_NON_MANIFOLD = 8;

using ShapeMap = NCollection_IndexedMap<TopoDS_Shape, TopTools_ShapeMapHasher>;
using EdgeFaceMap = NCollection_IndexedDataMap<
    TopoDS_Shape,
    NCollection_List<TopoDS_Shape>,
    TopTools_ShapeMapHasher>;

struct TopologyCounts {
    std::size_t faces = 0;
    std::size_t edges = 0;
    std::size_t boundary_edges = 0;
    std::size_t non_manifold_edges = 0;
};

void reset_report(ShapeRepairData& report, double tolerance, double maximum_tolerance) {
    report.status = REPAIR_KERNEL_FAILURE;
    report.changed = false;
    report.input_face_count = 0;
    report.input_edge_count = 0;
    report.output_face_count = 0;
    report.output_edge_count = 0;
    report.component_count = 0;
    report.boundary_edge_count = 0;
    report.non_manifold_edge_count = 0;
    report.sewing_free_edge_count = 0;
    report.sewing_multiple_edge_count = 0;
    report.sewn_edge_count = 0;
    report.degenerated_shape_count = 0;
    report.deleted_face_count = 0;
    report.requested_tolerance = tolerance;
    report.effective_tolerance = tolerance;
    report.maximum_tolerance = maximum_tolerance;
    report.max_input_tolerance = 0.0;
    report.max_output_tolerance = 0.0;
    report.max_detected_seam_gap = -1.0;
    report.face_history.clear();
    report.edge_history.clear();
}

bool valid_tolerances(double tolerance, double maximum_tolerance) {
    return std::isfinite(tolerance) && tolerance > 0.0
        && std::isfinite(maximum_tolerance) && maximum_tolerance >= tolerance;
}

TopologyCounts topology_counts(const TopoDS_Shape& shape) {
    TopologyCounts counts;
    ShapeMap faces;
    ShapeMap edges;
    TopExp::MapShapes(shape, TopAbs_FACE, faces);
    TopExp::MapShapes(shape, TopAbs_EDGE, edges);
    counts.faces = static_cast<std::size_t>(faces.Extent());
    counts.edges = static_cast<std::size_t>(edges.Extent());

    EdgeFaceMap edge_faces;
    TopExp::MapShapesAndAncestors(shape, TopAbs_EDGE, TopAbs_FACE, edge_faces);
    for (int index = 1; index <= edge_faces.Extent(); ++index) {
        const auto& ancestors = edge_faces(index);
        const int incidence = ancestors.Extent();
        if (incidence > 2) {
            ++counts.non_manifold_edges;
        } else if (incidence == 1) {
            const TopoDS_Edge edge = TopoDS::Edge(edge_faces.FindKey(index));
            const TopoDS_Face face = TopoDS::Face(ancestors.First());
            if (!BRepTools::IsReallyClosed(edge, face)) ++counts.boundary_edges;
        }
    }
    return counts;
}

double max_shape_tolerance(const TopoDS_Shape& shape) {
    double maximum = 0.0;
    for (TopExp_Explorer explorer(shape, TopAbs_VERTEX); explorer.More(); explorer.Next()) {
        maximum = std::max(maximum, BRep_Tool::Tolerance(TopoDS::Vertex(explorer.Current())));
    }
    for (TopExp_Explorer explorer(shape, TopAbs_EDGE); explorer.More(); explorer.Next()) {
        maximum = std::max(maximum, BRep_Tool::Tolerance(TopoDS::Edge(explorer.Current())));
    }
    for (TopExp_Explorer explorer(shape, TopAbs_FACE); explorer.More(); explorer.Next()) {
        maximum = std::max(maximum, BRep_Tool::Tolerance(TopoDS::Face(explorer.Current())));
    }
    return maximum;
}

TopoDS_Compound face_compound(const std::vector<TopoDS_Face>& faces) {
    BRep_Builder builder;
    TopoDS_Compound compound;
    builder.MakeCompound(compound);
    for (const auto& face : faces) builder.Add(compound, face);
    return compound;
}

std::vector<TopoDS_Shell> normalize_shells(const TopoDS_Shape& shape) {
    std::vector<TopoDS_Shell> result;
    ShapeMap shells;
    ShapeMap owned_faces;
    TopExp::MapShapes(shape, TopAbs_SHELL, shells);
    for (int index = 1; index <= shells.Extent(); ++index) {
        const TopoDS_Shell shell = TopoDS::Shell(shells(index));
        ShapeMap shell_faces;
        TopExp::MapShapes(shell, TopAbs_FACE, shell_faces);
        if (shell_faces.IsEmpty()) continue;
        result.push_back(shell);
        for (int face = 1; face <= shell_faces.Extent(); ++face) owned_faces.Add(shell_faces(face));
    }

    ShapeMap all_faces;
    TopExp::MapShapes(shape, TopAbs_FACE, all_faces);
    BRep_Builder builder;
    for (int index = 1; index <= all_faces.Extent(); ++index) {
        if (owned_faces.Contains(all_faces(index))) continue;
        TopoDS_Shell shell;
        builder.MakeShell(shell);
        builder.Add(shell, TopoDS::Face(all_faces(index)));
        result.push_back(shell);
    }
    return result;
}

TopoDS_Shape compound_shells(const std::vector<TopoDS_Shell>& shells) {
    if (shells.size() == 1) return shells.front();
    BRep_Builder builder;
    TopoDS_Compound compound;
    builder.MakeCompound(compound);
    for (const auto& shell : shells) builder.Add(compound, shell);
    return compound;
}

bool shells_are_valid(const std::vector<TopoDS_Shell>& shells) {
    return std::all_of(shells.begin(), shells.end(), [](const TopoDS_Shell& shell) {
        return BRepCheck_Analyzer(shell).IsValid();
    });
}

void fill_output_report(
    const TopoDS_Shape& output,
    const std::vector<TopoDS_Shell>& shells,
    ShapeRepairData& report)
{
    const TopologyCounts counts = topology_counts(output);
    report.output_face_count = counts.faces;
    report.output_edge_count = counts.edges;
    report.component_count = shells.size();
    report.boundary_edge_count = counts.boundary_edges;
    report.non_manifold_edge_count = counts.non_manifold_edges;
    report.max_output_tolerance = max_shape_tolerance(output);
}

void add_relation(
    std::uint32_t input_index,
    const TopoDS_Shape& candidate,
    TopAbs_ShapeEnum type,
    const ShapeMap& outputs,
    rust::Vec<std::uint32_t>& relations,
    std::set<std::pair<std::uint32_t, std::uint32_t>>& seen)
{
    if (candidate.IsNull()) return;
    ShapeMap candidates;
    if (candidate.ShapeType() == type) candidates.Add(candidate);
    else TopExp::MapShapes(candidate, type, candidates);
    for (int index = 1; index <= candidates.Extent(); ++index) {
        const int output_index = outputs.FindIndex(candidates(index));
        if (output_index == 0) continue;
        const auto relation = std::make_pair(input_index, static_cast<std::uint32_t>(output_index - 1));
        if (!seen.insert(relation).second) continue;
        relations.push_back(relation.first);
        relations.push_back(relation.second);
    }
}

bool sampled_edge_gap(const TopoDS_Edge& left, const TopoDS_Edge& right, double& gap) {
    BRepAdaptor_Curve first(left);
    BRepAdaptor_Curve second(right);
    const double first_start = first.FirstParameter();
    const double first_end = first.LastParameter();
    const double second_start = second.FirstParameter();
    const double second_end = second.LastParameter();
    if (!std::isfinite(first_start) || !std::isfinite(first_end)
        || !std::isfinite(second_start) || !std::isfinite(second_end)) return false;

    const gp_Pnt first_a = first.Value(first_start);
    const gp_Pnt first_b = first.Value(first_end);
    const gp_Pnt second_a = second.Value(second_start);
    const gp_Pnt second_b = second.Value(second_end);
    const bool reverse = first_a.Distance(second_b) + first_b.Distance(second_a)
        < first_a.Distance(second_a) + first_b.Distance(second_b);

    constexpr int SAMPLE_COUNT = 17;
    gap = 0.0;
    for (int sample = 0; sample < SAMPLE_COUNT; ++sample) {
        const double fraction = static_cast<double>(sample) / static_cast<double>(SAMPLE_COUNT - 1);
        const double first_parameter = first_start + fraction * (first_end - first_start);
        const double second_fraction = reverse ? 1.0 - fraction : fraction;
        const double second_parameter = second_start + second_fraction * (second_end - second_start);
        gap = std::max(gap, first.Value(first_parameter).Distance(second.Value(second_parameter)));
    }
    return true;
}

void fill_seam_gap(const BRepBuilderAPI_Sewing& sewing, ShapeRepairData& report) {
    bool detected = false;
    double maximum = 0.0;
    for (int index = 1; index <= sewing.NbContigousEdges(); ++index) {
        std::vector<TopoDS_Edge> boundaries;
        const auto& couple = sewing.ContigousEdgeCouple(index);
        for (NCollection_List<TopoDS_Shape>::Iterator iterator(couple); iterator.More(); iterator.Next()) {
            if (iterator.Value().ShapeType() == TopAbs_EDGE) boundaries.push_back(TopoDS::Edge(iterator.Value()));
        }
        for (std::size_t left = 0; left < boundaries.size(); ++left) {
            for (std::size_t right = left + 1; right < boundaries.size(); ++right) {
                double gap = 0.0;
                if (!sampled_edge_gap(boundaries[left], boundaries[right], gap)) continue;
                detected = true;
                maximum = std::max(maximum, gap);
            }
        }
    }
    if (detected) report.max_detected_seam_gap = maximum;
}

void fill_sewing_history(
    const std::vector<TopoDS_Face>& input_faces,
    const TopoDS_Shape& input_shape,
    const TopoDS_Shape& output,
    const BRepBuilderAPI_Sewing& sewing,
    ShapeRepairData& report)
{
    ShapeMap output_faces;
    ShapeMap output_edges;
    ShapeMap input_edges;
    TopExp::MapShapes(output, TopAbs_FACE, output_faces);
    TopExp::MapShapes(output, TopAbs_EDGE, output_edges);
    TopExp::MapShapes(input_shape, TopAbs_EDGE, input_edges);
    std::set<std::pair<std::uint32_t, std::uint32_t>> face_seen;
    std::set<std::pair<std::uint32_t, std::uint32_t>> edge_seen;

    for (std::size_t index = 0; index < input_faces.size(); ++index) {
        const TopoDS_Shape replacement = sewing.IsModified(input_faces[index])
            ? sewing.Modified(input_faces[index])
            : TopoDS_Shape(input_faces[index]);
        add_relation(static_cast<std::uint32_t>(index), replacement, TopAbs_FACE, output_faces, report.face_history, face_seen);
    }
    for (int index = 1; index <= input_edges.Extent(); ++index) {
        const TopoDS_Shape& edge = input_edges(index);
        const TopoDS_Shape replacement = sewing.IsModifiedSubShape(edge)
            ? sewing.ModifiedSubShape(edge)
            : edge;
        add_relation(static_cast<std::uint32_t>(index - 1), replacement, TopAbs_EDGE, output_edges, report.edge_history, edge_seen);
    }
    for (int index = 1; index <= sewing.NbContigousEdges(); ++index) {
        const TopoDS_Edge& output_edge = sewing.ContigousEdge(index);
        const auto& couple = sewing.ContigousEdgeCouple(index);
        for (NCollection_List<TopoDS_Shape>::Iterator iterator(couple); iterator.More(); iterator.Next()) {
            const int input_index = input_edges.FindIndex(iterator.Value());
            if (input_index == 0) continue;
            add_relation(static_cast<std::uint32_t>(input_index - 1), output_edge, TopAbs_EDGE, output_edges, report.edge_history, edge_seen);
        }
    }
}

void add_context_history(
    std::uint32_t input_index,
    const TopoDS_Shape& working_shape,
    TopAbs_ShapeEnum type,
    const ShapeMap& outputs,
    const occ::handle<ShapeBuild_ReShape>& context,
    rust::Vec<std::uint32_t>& relations,
    std::set<std::pair<std::uint32_t, std::uint32_t>>& seen)
{
    const std::size_t initial_size = relations.size();
    const auto history = context->History();
    const auto& modified = history->Modified(working_shape);
    for (NCollection_List<TopoDS_Shape>::Iterator iterator(modified); iterator.More(); iterator.Next()) {
        add_relation(input_index, iterator.Value(), type, outputs, relations, seen);
    }
    const auto& generated = history->Generated(working_shape);
    for (NCollection_List<TopoDS_Shape>::Iterator iterator(generated); iterator.More(); iterator.Next()) {
        add_relation(input_index, iterator.Value(), type, outputs, relations, seen);
    }
    if (relations.size() == initial_size) {
        add_relation(input_index, context->ValueLeaf(working_shape), type, outputs, relations, seen);
    }
}

void fill_healing_history(
    const TopoDS_Shape& input,
    const TopoDS_Shape& output,
    const BRepBuilderAPI_Copy& copier,
    const occ::handle<ShapeBuild_ReShape>& context,
    ShapeRepairData& report)
{
    ShapeMap input_faces;
    ShapeMap input_edges;
    ShapeMap output_faces;
    ShapeMap output_edges;
    TopExp::MapShapes(input, TopAbs_FACE, input_faces);
    TopExp::MapShapes(input, TopAbs_EDGE, input_edges);
    TopExp::MapShapes(output, TopAbs_FACE, output_faces);
    TopExp::MapShapes(output, TopAbs_EDGE, output_edges);
    std::set<std::pair<std::uint32_t, std::uint32_t>> face_seen;
    std::set<std::pair<std::uint32_t, std::uint32_t>> edge_seen;

    for (int index = 1; index <= input_faces.Extent(); ++index) {
        const TopoDS_Shape working = copier.ModifiedShape(input_faces(index));
        const std::size_t before = report.face_history.size();
        add_context_history(static_cast<std::uint32_t>(index - 1), working, TopAbs_FACE, output_faces, context, report.face_history, face_seen);
        if (report.face_history.size() == before) ++report.deleted_face_count;
    }
    for (int index = 1; index <= input_edges.Extent(); ++index) {
        const TopoDS_Shape working = copier.ModifiedShape(input_edges(index));
        add_context_history(static_cast<std::uint32_t>(index - 1), working, TopAbs_EDGE, output_edges, context, report.edge_history, edge_seen);
    }
}

std::unique_ptr<TopoDS_Shape> finalize_result(
    const std::vector<TopoDS_Shell>& shells,
    ShapeRepairData& report)
{
    if (shells.empty()) {
        report.status = REPAIR_NO_OUTPUT;
        return nullptr;
    }
    if (report.max_output_tolerance > report.maximum_tolerance) {
        report.status = REPAIR_TOLERANCE_EXCEEDED;
        return nullptr;
    }
    if (report.non_manifold_edge_count != 0) {
        report.status = REPAIR_NON_MANIFOLD;
        return nullptr;
    }
    if (!shells_are_valid(shells)) {
        report.status = REPAIR_INVALID_TOPOLOGY;
        return nullptr;
    }
    if (shells.size() != 1) {
        report.status = REPAIR_MULTIPLE_COMPONENTS;
        return nullptr;
    }
    report.status = REPAIR_SUCCESS;
    return std::make_unique<TopoDS_Shape>(shells.front());
}

} // namespace

std::unique_ptr<TopoDS_Shape> sew_faces_with_report(
    const std::vector<TopoDS_Face>& faces,
    double tolerance,
    double maximum_tolerance,
    ShapeRepairData& report)
{
    reset_report(report, tolerance, maximum_tolerance);
    if (!valid_tolerances(tolerance, maximum_tolerance)) {
        report.status = REPAIR_INVALID_TOLERANCE;
        return nullptr;
    }
    if (faces.empty()) {
        report.status = REPAIR_EMPTY_INPUT;
        return nullptr;
    }

    try {
        const TopoDS_Compound input = face_compound(faces);
        const TopologyCounts input_counts = topology_counts(input);
        report.input_face_count = faces.size();
        report.input_edge_count = input_counts.edges;
        report.max_input_tolerance = max_shape_tolerance(input);
        if (report.max_input_tolerance > maximum_tolerance) {
            report.status = REPAIR_TOLERANCE_EXCEEDED;
            return nullptr;
        }

        BRepBuilderAPI_Sewing sewing(tolerance, true, true, true, true);
        sewing.SetMinTolerance(std::min(tolerance, Precision::Confusion()));
        if (maximum_tolerance < std::numeric_limits<double>::max()) sewing.SetMaxTolerance(maximum_tolerance);
        sewing.SetLocalTolerancesMode(false);
        sewing.SetNonManifoldMode(true);
        sewing.SetSameParameterMode(true);
        for (const auto& face : faces) sewing.Add(face);
        sewing.Perform();
        report.effective_tolerance = sewing.Tolerance();
        report.sewing_free_edge_count = static_cast<std::size_t>(sewing.NbFreeEdges());
        report.sewing_multiple_edge_count = static_cast<std::size_t>(sewing.NbMultipleEdges());
        report.sewn_edge_count = static_cast<std::size_t>(sewing.NbContigousEdges());
        report.degenerated_shape_count = static_cast<std::size_t>(sewing.NbDegeneratedShapes());
        report.deleted_face_count = static_cast<std::size_t>(sewing.NbDeletedFaces());
        report.changed = sewing.NbContigousEdges() != 0 || sewing.NbDeletedFaces() != 0;
        fill_seam_gap(sewing, report);

        const TopoDS_Shape& sewn = sewing.SewedShape();
        if (sewn.IsNull()) {
            report.status = REPAIR_NO_OUTPUT;
            return nullptr;
        }
        const std::vector<TopoDS_Shell> shells = normalize_shells(sewn);
        if (shells.empty()) {
            report.status = REPAIR_NO_OUTPUT;
            return nullptr;
        }
        const TopoDS_Shape output = compound_shells(shells);
        fill_output_report(output, shells, report);
        fill_sewing_history(faces, input, output, sewing, report);
        return finalize_result(shells, report);
    } catch (const Standard_Failure&) {
        report.status = REPAIR_KERNEL_FAILURE;
        return nullptr;
    }
}

std::unique_ptr<TopoDS_Shape> heal_shell_with_report(
    const TopoDS_Shape& shape,
    double tolerance,
    double maximum_tolerance,
    ShapeRepairData& report)
{
    reset_report(report, tolerance, maximum_tolerance);
    if (!valid_tolerances(tolerance, maximum_tolerance)) {
        report.status = REPAIR_INVALID_TOLERANCE;
        return nullptr;
    }
    if (shape.IsNull() || shape.ShapeType() != TopAbs_SHELL) {
        report.status = REPAIR_INVALID_TOPOLOGY;
        return nullptr;
    }

    try {
        const TopologyCounts input_counts = topology_counts(shape);
        report.input_face_count = input_counts.faces;
        report.input_edge_count = input_counts.edges;
        report.max_input_tolerance = max_shape_tolerance(shape);
        if (report.max_input_tolerance > maximum_tolerance) {
            report.status = REPAIR_TOLERANCE_EXCEEDED;
            return nullptr;
        }

        BRepBuilderAPI_Copy copier(shape, true, false);
        const TopoDS_Shape working = copier.Shape();
        if (working.IsNull()) {
            report.status = REPAIR_NO_OUTPUT;
            return nullptr;
        }
        const occ::handle<ShapeBuild_ReShape> context = new ShapeBuild_ReShape();
        ShapeFix_Shape fixer(working);
        fixer.SetContext(context);
        fixer.SetPrecision(tolerance);
        fixer.SetMinTolerance(std::min(tolerance, Precision::Confusion()));
        fixer.SetMaxTolerance(maximum_tolerance);
        fixer.FixSolidMode() = 0;
        fixer.FixFreeShellMode() = 1;
        fixer.FixFreeFaceMode() = 1;
        fixer.FixFreeWireMode() = 1;
        fixer.FixSameParameterMode() = 1;
        fixer.FixVertexPositionMode() = 0;
        fixer.FixVertexTolMode() = 1;
        fixer.Perform();
        report.effective_tolerance = fixer.Precision();
        report.changed = fixer.Status(ShapeExtend_DONE1) || fixer.Status(ShapeExtend_DONE2)
            || fixer.Status(ShapeExtend_DONE3) || fixer.Status(ShapeExtend_DONE4)
            || fixer.Status(ShapeExtend_DONE5) || fixer.Status(ShapeExtend_DONE6);

        const TopoDS_Shape healed = fixer.Shape();
        if (healed.IsNull()) {
            report.status = REPAIR_NO_OUTPUT;
            return nullptr;
        }
        const std::vector<TopoDS_Shell> shells = normalize_shells(healed);
        if (shells.empty()) {
            report.status = REPAIR_NO_OUTPUT;
            return nullptr;
        }
        const TopoDS_Shape output = compound_shells(shells);
        fill_output_report(output, shells, report);
        fill_healing_history(shape, output, copier, context, report);
        return finalize_result(shells, report);
    } catch (const Standard_Failure&) {
        report.status = REPAIR_KERNEL_FAILURE;
        return nullptr;
    }
}

} // namespace cadrum
