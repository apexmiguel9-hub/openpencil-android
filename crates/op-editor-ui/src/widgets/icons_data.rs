//! Lucide d-string constants — all `Icon::paths` data lives here so
//! `icons.rs` stays under the 800-line file ceiling. New icons:
//! add the d-string here, register a variant in `icons.rs::Icon` +
//! `paths()` + `from_name()`. Sourced verbatim from
//! https://github.com/lucide-icons/lucide (ISC).
//!
//! The table itself outgrew the ceiling too, so it is split across two
//! sibling data modules and glob-re-exported here. The lookup API is
//! unchanged — `icons.rs` still does `use super::icons_data::*;` and
//! names every constant directly.
//!
//! | File                      | Glyph range                    |
//! | ------------------------- | ------------------------------ |
//! | `icons_data/glyphs_a.rs`  | `CURSOR` … `ARROW_DOWN`        |
//! | `icons_data/glyphs_b.rs`  | `MAIL` … `SQUARES_EXCLUDE`     |

mod glyphs_a;
mod glyphs_b;

pub(super) use glyphs_a::*;
pub(super) use glyphs_b::*;
