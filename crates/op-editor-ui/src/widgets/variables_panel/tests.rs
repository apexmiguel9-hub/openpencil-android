//! Unit tests for the variables panel. Split across sibling modules
//! under `tests/` to keep every file under the 800-line cap.
//!
//! | File                      | Coverage                              |
//! | ------------------------- | ------------------------------------- |
//! | `tests/support.rs`        | recording backend + seeded documents  |
//! | `tests/theme_variant.rs`  | axis tabs / menus / rename inputs     |
//! | `tests/rows_and_hits.rs`  | row resolution, metrics, hit-testing  |

mod rows_and_hits;
mod support;
mod theme_variant;
