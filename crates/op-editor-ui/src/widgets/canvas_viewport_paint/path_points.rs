//! Doc-space → world-space path point projection and path flattening,
//! split out of `canvas_viewport_paint.rs` to keep that spine under the
//! repository's 800-line cap.

#[cfg(test)]
use crate::layout_scene::SceneNode;
use crate::Point2D;
#[cfg(test)]
use jian_scene::path_geometry::flatten_path_points;
const STACK_WORLD_PATH_POINTS: usize = 64;

// The large stack variant is intentional: hot path-overlay painting
// avoids heap allocation for the common small-polyline case.
#[allow(clippy::large_enum_variant)]
pub(crate) enum WorldPathPoints {
    Stack {
        points: [Point2D; STACK_WORLD_PATH_POINTS],
        len: usize,
    },
    Owned(Vec<Point2D>),
}

impl WorldPathPoints {
    pub(crate) fn as_slice(&self) -> &[Point2D] {
        match self {
            Self::Stack { points, len } => &points[..*len],
            Self::Owned(points) => points.as_slice(),
        }
    }
}

pub(super) fn doc_to_world_point(p: Point2D, viewport_origin: Point2D, zoom: f32) -> Point2D {
    Point2D::new(
        viewport_origin.x + p.x * zoom,
        viewport_origin.y + p.y * zoom,
    )
}

pub(crate) fn world_path_points(
    points: &[Point2D],
    viewport_origin: Point2D,
    zoom: f32,
) -> WorldPathPoints {
    if points.len() <= STACK_WORLD_PATH_POINTS {
        let mut stack = [Point2D::ZERO; STACK_WORLD_PATH_POINTS];
        for (idx, point) in points.iter().copied().enumerate() {
            stack[idx] = doc_to_world_point(point, viewport_origin, zoom);
        }
        return WorldPathPoints::Stack {
            points: stack,
            len: points.len(),
        };
    }
    WorldPathPoints::Owned(
        points
            .iter()
            .copied()
            .map(|p| doc_to_world_point(p, viewport_origin, zoom))
            .collect(),
    )
}

/// Flatten a Path scene node into a doc-space polyline — cubic
/// segments whose endpoints carry handles are tessellated; a
/// handle-free path falls back to the straight `points` polyline.
/// A closed path appends the last-anchor → first-anchor segment.
#[cfg(test)]
pub(crate) fn flatten_path(node: &SceneNode) -> Vec<Point2D> {
    use jian_scene::path_geometry::PathPoints;
    match flatten_path_points(node) {
        PathPoints::Borrowed(points) => points.to_vec(),
        PathPoints::Owned(points) => points,
    }
}
