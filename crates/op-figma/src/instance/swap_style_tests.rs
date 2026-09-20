//! Component-swap regressions for stale fields inherited from the old SYMBOL.

use super::tests::{derived_with, guid, guid_path, leaf, obj, ov_with, size, symbol_root};
use super::*;

fn paint(label: &str) -> FigValue {
    obj(vec![
        ("type", FigValue::Str("SOLID".into())),
        ("label", FigValue::Str(label.into())),
    ])
}

fn old_layout_block() -> TreeNode {
    TreeNode {
        figma: obj(vec![
            ("type", FigValue::Str("INSTANCE".into())),
            ("guid", guid(1, 10)),
            ("size", size(32.0, 32.0)),
            ("fillPaints", FigValue::Array(vec![paint("old-fill")])),
            ("strokePaints", FigValue::Array(vec![paint("old-stroke")])),
            ("strokeWeight", FigValue::Float(1.0)),
            ("strokeJoin", FigValue::Str("MITER".into())),
            ("dashPattern", FigValue::Array(vec![FigValue::Float(2.0)])),
            ("cornerSmoothing", FigValue::Float(0.25)),
            ("effects", FigValue::Array(vec![paint("old-effect")])),
            ("opacity", FigValue::Float(0.5)),
            ("blendMode", FigValue::Str("MULTIPLY".into())),
            ("stackCounterAlignItems", FigValue::Str("CENTER".into())),
            ("symbolData", obj(vec![("symbolID", guid(2, 20))])),
        ]),
        children: vec![],
    }
}

fn apply_swap(extra: Vec<(&str, FigValue)>) -> TreeNode {
    let mut fields = vec![("overriddenSymbolID", guid(3, 30))];
    fields.extend(extra);
    let override_entry = ov_with(vec![guid(1, 10)], fields);
    apply_instance_overrides(
        &symbol_root(vec![old_layout_block()]),
        Some(&[override_entry]),
        None,
        None,
    )
    .remove(0)
}

#[test]
fn component_swap_clears_unoverridden_layout_block_style() {
    let swapped = apply_swap(vec![]);

    assert!(swapped.figma.get("fillPaints").is_none());
    assert!(swapped.figma.get("strokePaints").is_none());
    assert!(swapped.figma.get("strokeWeight").is_none());
    assert!(swapped.figma.get("strokeJoin").is_none());
    assert!(swapped.figma.get("dashPattern").is_none());
    assert!(swapped.figma.get("cornerSmoothing").is_none());
    assert!(swapped.figma.get("effects").is_none());
    assert!(swapped.figma.get("opacity").is_none());
    assert!(swapped.figma.get("blendMode").is_none());
    assert!(swapped.figma.get("stackCounterAlignItems").is_none());
    assert!(swapped.figma.get("overriddenSymbolID").is_some());
}

#[test]
fn component_swap_inherits_target_base_and_container_style() {
    let swapped = apply_swap(vec![]);
    let target = obj(vec![
        ("fillPaints", FigValue::Array(vec![paint("target-fill")])),
        ("effects", FigValue::Array(vec![paint("target-effect")])),
        ("opacity", FigValue::Float(0.75)),
        ("blendMode", FigValue::Str("SCREEN".into())),
        ("cornerSmoothing", FigValue::Float(0.5)),
        ("strokeJoin", FigValue::Str("ROUND".into())),
        (
            "dashPattern",
            FigValue::Array(vec![FigValue::Float(3.0), FigValue::Float(1.0)]),
        ),
    ]);

    let merged = merge_symbol_props(&swapped.figma, &target);
    let label = |key: &str| {
        merged
            .get_array(key)
            .and_then(|values| values.first())
            .and_then(|value| value.get_str("label"))
    };
    assert_eq!(label("fillPaints"), Some("target-fill"));
    assert_eq!(label("effects"), Some("target-effect"));
    assert_eq!(merged.get_f64("opacity"), Some(0.75));
    assert_eq!(merged.get_str("blendMode"), Some("SCREEN"));
    assert_eq!(merged.get_f64("cornerSmoothing"), Some(0.5));
    assert_eq!(merged.get_str("strokeJoin"), Some("ROUND"));
    assert_eq!(merged.get_array("dashPattern").map(<[_]>::len), Some(2));
}

#[test]
fn redundant_same_symbol_override_keeps_legacy_resolved_style() {
    let override_entry = ov_with(vec![guid(1, 10)], vec![("overriddenSymbolID", guid(2, 20))]);
    let out = apply_instance_overrides(
        &symbol_root(vec![old_layout_block()]),
        Some(&[override_entry]),
        None,
        None,
    );

    let label = out[0]
        .figma
        .get_array("fillPaints")
        .and_then(|values| values.first())
        .and_then(|value| value.get_str("label"));
    assert_eq!(label, Some("old-fill"));
    assert_eq!(out[0].figma.get_f64("opacity"), Some(0.5));
}

#[test]
fn component_swap_preserves_fill_explicit_in_same_override() {
    let swapped = apply_swap(vec![(
        "fillPaints",
        FigValue::Array(vec![paint("new-fill")]),
    )]);

    let label = swapped
        .figma
        .get_array("fillPaints")
        .and_then(|paints| paints.first())
        .and_then(|paint| paint.get_str("label"));
    assert_eq!(label, Some("new-fill"));
    assert!(swapped.figma.get("strokePaints").is_none());
    assert!(swapped.figma.get("stackCounterAlignItems").is_none());
}

#[test]
fn support_row_outer_derived_geometry_overrides_nested_defaults() {
    let nested = TreeNode {
        figma: obj(vec![
            ("type", FigValue::Str("INSTANCE".into())),
            ("guid", guid(1, 20)),
            (
                "derivedSymbolData",
                FigValue::Array(vec![
                    derived_with(
                        vec![guid(2, 30)],
                        vec![
                            ("size", size(648.004, 45.0)),
                            ("authoredMarker", FigValue::Str("keep".into())),
                        ],
                    ),
                    derived_with(vec![guid(2, 31)], vec![("size", size(600.004, 45.0))]),
                ]),
            ),
        ]),
        children: vec![],
    };
    let outer = vec![
        guid_path(vec![guid(1, 20)]),
        derived_with(
            vec![guid(1, 20), guid(2, 30)],
            vec![("size", size(456.0, 50.0))],
        ),
        derived_with(
            vec![guid(1, 20), guid(2, 31)],
            vec![("size", size(408.0, 50.0))],
        ),
        derived_with(
            vec![guid(1, 20), guid(2, 32)],
            vec![("size", size(24.0, 24.0))],
        ),
    ];

    let out = apply_instance_overrides(&symbol_root(vec![nested]), None, Some(&outer), None);
    let entries = out[0]
        .figma
        .get_array("derivedSymbolData")
        .expect("nested derived data survives forwarding");
    let widths: Vec<f64> = entries
        .iter()
        .map(|entry| {
            entry
                .get("size")
                .and_then(FigVec2::from_value)
                .expect("derived entry has a size")
                .x
        })
        .collect();

    assert_eq!(widths, vec![456.0, 408.0, 24.0]);
    assert_eq!(entries[0].get_str("authoredMarker"), Some("keep"));
}

#[test]
fn swap_filter_drops_equal_sized_entries_from_known_base_component() {
    let mut base = symbol_root(vec![TreeNode {
        figma: obj(vec![
            ("type", FigValue::Str("TEXT".into())),
            ("guid", guid(4125, 724)),
            ("size", size(32.0, 32.0)),
        ]),
        children: Vec::new(),
    }]);
    base.figma.set("guid", guid(4125, 415));

    let mut target = symbol_root(vec![
        TreeNode {
            figma: obj(vec![
                ("type", FigValue::Str("INSTANCE".into())),
                ("guid", guid(40658, 996930)),
                ("size", size(32.0, 32.0)),
            ]),
            children: Vec::new(),
        },
        TreeNode {
            figma: obj(vec![
                ("type", FigValue::Str("INSTANCE".into())),
                ("guid", guid(40658, 1000837)),
                ("size", size(32.0, 32.0)),
            ]),
            children: Vec::new(),
        },
    ]);
    target.figma.set("guid", guid(4125, 3));
    target.figma.set("size", size(64.0, 32.0));

    let derived = vec![
        derived_with(vec![guid(4125, 415)], vec![("size", size(32.0, 32.0))]),
        derived_with(vec![guid(4125, 724)], vec![("size", size(32.0, 32.0))]),
        derived_with(vec![guid(40658, 996930)], vec![("size", size(32.0, 32.0))]),
        derived_with(vec![guid(40658, 1000837)], vec![("size", size(32.0, 32.0))]),
    ];
    let filtered = filter_swap_stale_derived(
        &derived,
        Some(&base),
        &target,
        Some(FigVec2 { x: 64.0, y: 32.0 }),
    );
    let keys = filtered
        .iter()
        .filter_map(guid_path_key)
        .collect::<Vec<_>>();
    assert_eq!(keys, ["40658:996930", "40658:1000837"]);
}

#[test]
fn exact_target_guids_are_not_remapped_by_mixed_stale_derived_entries() {
    let target_a = TreeNode {
        figma: obj(vec![
            ("type", FigValue::Str("INSTANCE".into())),
            ("guid", guid(47855, 93249)),
            ("name", FigValue::Str("A".into())),
            ("size", size(32.0, 32.0)),
        ]),
        children: Vec::new(),
    };
    let target_b = TreeNode {
        figma: obj(vec![
            ("type", FigValue::Str("INSTANCE".into())),
            ("guid", guid(47855, 93250)),
            ("name", FigValue::Str("B".into())),
            ("size", size(32.0, 32.0)),
        ]),
        children: Vec::new(),
    };
    let mut symbol = symbol_root(vec![target_a, target_b]);
    symbol.figma.set("guid", guid(47855, 93255));
    symbol.figma.set("size", size(64.0, 32.0));

    let derived = vec![
        derived_with(vec![guid(4125, 415)], vec![("size", size(32.0, 32.0))]),
        derived_with(vec![guid(4125, 724)], vec![("size", size(32.0, 32.0))]),
        derived_with(vec![guid(47855, 93249)], vec![("size", size(32.0, 32.0))]),
        derived_with(vec![guid(47855, 93250)], vec![("size", size(32.0, 32.0))]),
    ];
    let overrides = vec![
        ov_with(
            vec![guid(47855, 93249)],
            vec![("overriddenSymbolID", guid(4125, 158))],
        ),
        ov_with(
            vec![guid(47855, 93250)],
            vec![("overriddenSymbolID", guid(4125, 3))],
        ),
    ];
    let out = apply_instance_overrides(&symbol, Some(&overrides), Some(&derived), None);
    let target_of = |name: &str| {
        out.iter()
            .find(|node| node.figma.get_str("name") == Some(name))
            .and_then(|node| node.figma.get("overriddenSymbolID"))
            .and_then(crate::tree::guid_to_string)
    };
    assert_eq!(target_of("A").as_deref(), Some("4125:158"));
    assert_eq!(target_of("B").as_deref(), Some("4125:3"));
}

#[test]
fn strategy_1_pairs_remaining_entries_after_exact_identity_reservation() {
    let sym = symbol_root(vec![leaf("a", 1, 10), leaf("b", 1, 11)]);
    // The exact B entry is deliberately first. Removing only its eventual
    // positional pair would shift both virtual entries and lose virtual A.
    let derived = vec![
        guid_path(vec![guid(1, 11)]),
        guid_path(vec![guid(9, 100)]),
        guid_path(vec![guid(9, 101)]),
    ];
    let overrides = vec![
        ov_with(
            vec![guid(1, 11)],
            vec![("name", FigValue::Str("exact-b".into()))],
        ),
        ov_with(
            vec![guid(9, 101)],
            vec![("name", FigValue::Str("virtual-a".into()))],
        ),
    ];

    let out = apply_instance_overrides(&sym, Some(&overrides), Some(&derived), None);
    assert_eq!(out[0].figma.get_str("name"), Some("virtual-a"));
    assert_eq!(out[1].figma.get_str("name"), Some("exact-b"));
}

#[test]
fn strategy_3_pairs_remaining_entries_after_exact_identity_reservation() {
    let sym = symbol_root(vec![leaf("a", 1, 10), leaf("b", 1, 11), leaf("c", 1, 12)]);
    // A malformed first GUID makes virtual_guid_base return None. The exact C
    // identity must be removed from both streams before Strategy 3 zips them.
    let malformed = guid_path(vec![obj(vec![("localID", FigValue::Uint(99))])]);
    let derived = vec![
        malformed,
        guid_path(vec![guid(1, 12)]),
        guid_path(vec![guid(9, 100)]),
    ];
    let overrides = vec![
        ov_with(
            vec![guid(1, 12)],
            vec![("name", FigValue::Str("exact-c".into()))],
        ),
        ov_with(
            vec![guid(9, 100)],
            vec![("name", FigValue::Str("virtual-a".into()))],
        ),
    ];

    let out = apply_instance_overrides(&sym, Some(&overrides), Some(&derived), None);
    assert_eq!(out[0].figma.get_str("name"), Some("virtual-a"));
    assert_eq!(out[1].figma.get_str("name"), Some("b"));
    assert_eq!(out[2].figma.get_str("name"), Some("exact-c"));
}
