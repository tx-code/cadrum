#pragma once

#include <Geom_BSplineSurface.hxx>

namespace cadrum {

struct BSplineSurfaceData;

Handle(Geom_BSplineSurface) make_bspline_surface(
    const BSplineSurfaceData& data);

} // namespace cadrum
