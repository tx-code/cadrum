#include "cadrum/src/occt/ffi.rs.h"

#include <BRepTools.hxx>
#include <BRepTools_WireExplorer.hxx>
#include <BRep_Tool.hxx>
#include <BRepClass3d.hxx>
#include <NCollection_IndexedMap.hxx>
#include <Standard_Failure.hxx>
#include <TopExp.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Edge.hxx>
#include <TopoDS_Face.hxx>
#include <TopoDS_Iterator.hxx>
#include <TopoDS_Shell.hxx>
#include <TopoDS_Solid.hxx>
#include <TopoDS_Vertex.hxx>
#include <TopoDS_Wire.hxx>
#include <TopTools_ShapeMapHasher.hxx>
#include <gp_Pnt.hxx>

#include <cstdint>
#include <limits>
#include <memory>
#include <utility>
#include <vector>

namespace cadrum {
namespace {

using ShapeMap = NCollection_IndexedMap<TopoDS_Shape, TopTools_ShapeMapHasher>;
constexpr std::uint32_t MISSING_INDEX = std::numeric_limits<std::uint32_t>::max();

std::uint64_t runtime_id(const TopoDS_Shape& shape) {
    return reinterpret_cast<std::uint64_t>(shape.TShape().get());
}

std::uint8_t orientation(const TopAbs_Orientation value) {
    switch (value) {
    case TopAbs_FORWARD: return 0;
    case TopAbs_REVERSED: return 1;
    case TopAbs_INTERNAL: return 2;
    case TopAbs_EXTERNAL: return 3;
    }
    return 0;
}

std::uint32_t shape_index(const ShapeMap& map, const TopoDS_Shape& shape) {
    const int index = map.FindIndex(shape);
    if (index <= 0) return MISSING_INDEX;
    return static_cast<std::uint32_t>(index - 1);
}

void append_ordered_bodies(
    const TopoDS_Shape& shape,
    std::vector<TopoDS_Shape>& result)
{
    if (shape.IsNull()) return;
    if (shape.ShapeType() == TopAbs_SOLID || shape.ShapeType() == TopAbs_SHELL) {
        result.push_back(shape);
        return;
    }
    if (shape.ShapeType() != TopAbs_COMPOUND
        && shape.ShapeType() != TopAbs_COMPSOLID) return;

    for (TopoDS_Iterator child(shape, true, true); child.More(); child.Next()) {
        append_ordered_bodies(child.Value(), result);
    }
}

bool complete_body_tree(const TopoDS_Shape& shape, bool& saw_body) {
    if (shape.IsNull()) return false;
    if (shape.ShapeType() == TopAbs_SOLID || shape.ShapeType() == TopAbs_SHELL) {
        saw_body = true;
        return true;
    }
    if (shape.ShapeType() != TopAbs_COMPOUND
        && shape.ShapeType() != TopAbs_COMPSOLID) return false;

    bool saw_child = false;
    for (TopoDS_Iterator child(shape, true, true); child.More(); child.Next()) {
        saw_child = true;
        if (!complete_body_tree(child.Value(), saw_body)) return false;
    }
    return saw_child;
}

void append_shell(
    const TopoDS_Shell& shell,
    std::uint8_t role,
    const ShapeMap& faces,
    ShapeTopologyData& result)
{
    TopologyShellData shell_data{};
    shell_data.runtime_id = runtime_id(shell);
    shell_data.role = role;
    shell_data.orientation = orientation(shell.Orientation());
    shell_data.is_closed = BRep_Tool::IsClosed(shell);

    for (TopoDS_Iterator child(shell, false, true); child.More(); child.Next()) {
        if (child.Value().ShapeType() != TopAbs_FACE) continue;
        const std::uint32_t face = shape_index(faces, child.Value());
        if (face == MISSING_INDEX) {
            result.success = false;
            return;
        }
        TopologyFaceUseData face_use{};
        face_use.face = face;
        face_use.orientation = orientation(child.Value().Orientation());
        shell_data.faces.push_back(face_use);
    }
    result.shells.push_back(std::move(shell_data));
}

} // namespace

std::unique_ptr<std::vector<TopoDS_Shape>> decompose_into_brep_bodies(
    const TopoDS_Shape& shape)
{
    auto result = std::make_unique<std::vector<TopoDS_Shape>>();
    append_ordered_bodies(shape, *result);
    return result;
}

bool step_body_types_are_complete(const TopoDS_Shape& shape) {
    bool saw_body = false;
    return complete_body_tree(shape, saw_body) && saw_body;
}

ShapeTopologyData shape_topology(const TopoDS_Shape& shape) {
    ShapeTopologyData result{};
    result.success = false;
    try {
        if (shape.IsNull()
            || (shape.ShapeType() != TopAbs_SOLID
                && shape.ShapeType() != TopAbs_SHELL)) return result;

        ShapeMap vertices;
        ShapeMap edges;
        ShapeMap faces;
        TopExp::MapShapes(shape, TopAbs_VERTEX, vertices);
        TopExp::MapShapes(shape, TopAbs_EDGE, edges);
        TopExp::MapShapes(shape, TopAbs_FACE, faces);
        if (static_cast<std::uint64_t>(vertices.Extent()) >= MISSING_INDEX
            || static_cast<std::uint64_t>(edges.Extent()) >= MISSING_INDEX
            || static_cast<std::uint64_t>(faces.Extent()) >= MISSING_INDEX) return result;

        for (int index = 1; index <= vertices.Extent(); ++index) {
            const TopoDS_Vertex vertex = TopoDS::Vertex(vertices(index));
            const gp_Pnt point = BRep_Tool::Pnt(vertex);
            TopologyVertexData data{};
            data.runtime_id = runtime_id(vertex);
            data.x = point.X();
            data.y = point.Y();
            data.z = point.Z();
            data.tolerance = BRep_Tool::Tolerance(vertex);
            result.vertices.push_back(data);
        }

        for (int index = 1; index <= edges.Extent(); ++index) {
            TopoDS_Edge edge = TopoDS::Edge(edges(index));
            edge.Orientation(TopAbs_FORWARD);
            TopoDS_Vertex first;
            TopoDS_Vertex last;
            TopExp::Vertices(edge, first, last, false);

            TopologyEdgeData data{};
            data.runtime_id = runtime_id(edge);
            data.start_vertex = first.IsNull() ? MISSING_INDEX : shape_index(vertices, first);
            data.end_vertex = last.IsNull() ? MISSING_INDEX : shape_index(vertices, last);
            if ((!first.IsNull() && data.start_vertex == MISSING_INDEX)
                || (!last.IsNull() && data.end_vertex == MISSING_INDEX)) return result;
            result.edges.push_back(std::move(data));
        }

        for (int face_index = 1; face_index <= faces.Extent(); ++face_index) {
            TopoDS_Face face = TopoDS::Face(faces(face_index));
            face.Orientation(TopAbs_FORWARD);
            const TopoDS_Wire outer = BRepTools::OuterWire(face);

            TopologyFaceData face_data{};
            face_data.runtime_id = runtime_id(face);
            for (TopoDS_Iterator child(face, false, true); child.More(); child.Next()) {
                if (child.Value().ShapeType() != TopAbs_WIRE) continue;
                const TopoDS_Wire wire = TopoDS::Wire(child.Value());
                TopologyLoopData loop{};
                loop.is_outer = !outer.IsNull() && wire.IsSame(outer);
                loop.orientation = orientation(wire.Orientation());

                std::size_t expected_edges = 0;
                for (TopoDS_Iterator edge_child(wire, false, true);
                     edge_child.More(); edge_child.Next()) {
                    if (edge_child.Value().ShapeType() == TopAbs_EDGE) ++expected_edges;
                }
                if (expected_edges >= MISSING_INDEX
                    || face_data.boundary_loops.size() >= MISSING_INDEX) return result;

                BRepTools_WireExplorer explorer(wire, face);
                while (explorer.More()) {
                    const TopoDS_Edge& edge = explorer.Current();
                    const std::uint32_t edge_index = shape_index(edges, edge);
                    if (edge_index == MISSING_INDEX) return result;

                    TopologyEdgeUseData edge_use{};
                    edge_use.edge = edge_index;
                    edge_use.orientation = orientation(explorer.Orientation());
                    if (loop.edges.size() >= MISSING_INDEX) return result;
                    const std::uint32_t use_index =
                        static_cast<std::uint32_t>(loop.edges.size());
                    loop.edges.push_back(edge_use);

                    TopologyEdgeIncidentData incident{};
                    incident.face = static_cast<std::uint32_t>(face_index - 1);
                    incident.boundary_loop =
                        static_cast<std::uint32_t>(face_data.boundary_loops.size());
                    incident.edge_use = use_index;
                    incident.orientation = edge_use.orientation;
                    result.edges[edge_index].incidents.push_back(incident);
                    explorer.Next();
                }
                if (loop.edges.size() != expected_edges) return result;
                face_data.boundary_loops.push_back(std::move(loop));
            }
            result.faces.push_back(std::move(face_data));
        }

        result.success = true;
        if (shape.ShapeType() == TopAbs_SHELL) {
            append_shell(TopoDS::Shell(shape), 0, faces, result);
        } else {
            const TopoDS_Shell outer =
                BRepClass3d::OuterShell(TopoDS::Solid(shape));
            for (TopoDS_Iterator child(shape, true, true);
                 child.More(); child.Next()) {
                if (child.Value().ShapeType() == TopAbs_SHELL) {
                    const TopoDS_Shell shell = TopoDS::Shell(child.Value());
                    append_shell(shell, !outer.IsNull() && shell.IsSame(outer) ? 1 : 2,
                                 faces, result);
                    if (!result.success) return result;
                }
            }
        }
        if (result.shells.empty()) result.success = false;
    } catch (const Standard_Failure&) {
        result.success = false;
    }
    return result;
}

} // namespace cadrum
