//! Geometry violations a deck board forbids repairing in place.
//!
//! Every other surface we generate has a defensible last resort for content
//! that will not fit: a web page can clip a scroll row at the viewport edge
//! and the reader scrolls it back into view. A projector board has no such
//! move — clipped content is simply gone, and nobody in the room knows it was
//! ever there. The deck contract (deck-system spec §3.1) orders the real
//! fixes as shorten the copy → change the page type → split the page, and
//! none of the three is a call a geometry pass can make on its own.
//!
//! So the pass reports instead of repairing. These echoes carry the measured
//! numbers that proved the violation, and the caller renders them into
//! whatever channel it has — `RepairSummary::note` on the finalize path
//! (deliberately NOT a `RepairRecord`, which means "one edit was applied"),
//! a log line on the per-subtask path.

/// One deck-board violation that was detected and deliberately not repaired.
#[derive(Debug, Clone, PartialEq)]
pub enum DeckEcho {
    /// A horizontal row whose children are wider than the board. On any other
    /// surface `fix_horizontal_overflow` spans the viewport and sets
    /// `clipContent`; on a board that would delete the tail of the row from
    /// the projection, so the row is left visibly too wide instead.
    HorizontalOverflow {
        /// Target row, when it carries an id.
        node_id: Option<String>,
        /// Target row's name, when it has one.
        node_name: Option<String>,
        /// Summed child widths plus gaps, in board pixels.
        content_width: f64,
        /// The row's inner width (its own width minus horizontal padding).
        available_width: f64,
    },
}

impl DeckEcho {
    /// One-line rendering for a note or a log line.
    pub fn line(&self) -> String {
        match self {
            DeckEcho::HorizontalOverflow {
                content_width,
                available_width,
                ..
            } => format!(
                "deck · {} · row content {}px exceeds the board's {}px — split the slide \
                 or shorten the row; not clipped (clipping hides it on the projector)",
                self.node_label(),
                round(*content_width),
                round(*available_width),
            ),
        }
    }

    /// `Name [id]`, `[id]`, `Name`, or `an unnamed row` — whichever the node
    /// actually has. A row a weak model emitted without an id is the common
    /// case here, and "unnamed" is more honest than an empty slot.
    fn node_label(&self) -> String {
        let (node_id, node_name) = match self {
            DeckEcho::HorizontalOverflow {
                node_id, node_name, ..
            } => (node_id.as_deref(), node_name.as_deref()),
        };
        let name = node_name.map(str::trim).filter(|name| !name.is_empty());
        match (name, node_id) {
            (Some(name), Some(id)) => format!("{name} [{id}]"),
            (Some(name), None) => name.to_string(),
            (None, Some(id)) => format!("[{id}]"),
            (None, None) => "an unnamed row".to_string(),
        }
    }
}

fn round(value: f64) -> i64 {
    value.round() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_line_names_the_row_and_both_measurements() {
        let echo = DeckEcho::HorizontalOverflow {
            node_id: Some("n12".into()),
            node_name: Some("KPI Row".into()),
            content_width: 2140.0,
            available_width: 1776.0,
        };
        let line = echo.line();
        assert!(line.contains("KPI Row [n12]"), "{line}");
        assert!(line.contains("2140px"), "{line}");
        assert!(line.contains("1776px"), "{line}");
    }

    #[test]
    fn an_anonymous_row_still_renders() {
        let echo = DeckEcho::HorizontalOverflow {
            node_id: None,
            node_name: None,
            content_width: 100.0,
            available_width: 50.0,
        };
        assert!(echo.line().contains("an unnamed row"));
    }
}
