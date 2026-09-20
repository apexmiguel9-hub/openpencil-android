//! Adapter tests spine — the shared `load` helper plus the sibling
//! test modules. Split for the repo's 800-line file cap.

use super::*;

pub(super) fn load(src: &str) -> LoadedDoc {
    let r = jian_ops_schema::load_str(src).unwrap();
    pen_document_to_payload(&r.value)
}

#[path = "adapter_compat_tests.rs"]
mod adapter_compat_tests;
#[path = "adapter_geometry_tests.rs"]
mod adapter_geometry_tests;
#[path = "adapter_text_widget_tests.rs"]
mod adapter_text_widget_tests;
