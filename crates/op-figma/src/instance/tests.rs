//! Instance-override resolution tests — drives each of the four
//! Strategy branches with a tiny synthetic symbol subtree and a few
//! override / derived entries, asserting that the resulting overridden
//! tree carries the expected fields. The fixtures mirror the shape of a
//! decoded Figma `.fig` (`guid` / `guidPath.guids` arrays / `size` /
//! `fontSize` / etc.).

use super::*;

pub(super) fn obj(pairs: Vec<(&str, FigValue)>) -> FigValue {
    FigValue::Object(pairs.into_iter().map(|(k, v)| (k.into(), v)).collect())
}

pub(super) fn guid(sid: u32, lid: u32) -> FigValue {
    obj(vec![
        ("sessionID", FigValue::Uint(sid)),
        ("localID", FigValue::Uint(lid)),
    ])
}

pub(super) fn size(x: f32, y: f32) -> FigValue {
    obj(vec![("x", FigValue::Float(x)), ("y", FigValue::Float(y))])
}

pub(super) fn guid_path(guids: Vec<FigValue>) -> FigValue {
    obj(vec![(
        "guidPath",
        obj(vec![("guids", FigValue::Array(guids))]),
    )])
}

pub(super) fn ov_with(guids: Vec<FigValue>, extra: Vec<(&str, FigValue)>) -> FigValue {
    let mut pairs = vec![(
        "guidPath".into(),
        obj(vec![("guids", FigValue::Array(guids))]),
    )];
    for (k, v) in extra {
        pairs.push((k.into(), v));
    }
    FigValue::Object(pairs)
}

pub(super) fn leaf(name: &str, sid: u32, lid: u32) -> TreeNode {
    TreeNode {
        figma: obj(vec![
            ("type", FigValue::Str("RECTANGLE".into())),
            ("guid", guid(sid, lid)),
            ("name", FigValue::Str(name.into())),
            ("size", size(100.0, 100.0)),
        ]),
        children: vec![],
    }
}

pub(super) fn symbol_root(children: Vec<TreeNode>) -> TreeNode {
    TreeNode {
        figma: obj(vec![
            ("type", FigValue::Str("SYMBOL".into())),
            ("guid", guid(0, 0)),
            ("size", size(100.0, 100.0)),
        ]),
        children,
    }
}

#[test]
fn fast_path_scales_subtree_when_no_overrides() {
    let sym = symbol_root(vec![leaf("a", 1, 10)]);
    let out = apply_instance_overrides(&sym, None, None, Some(FigVec2 { x: 200.0, y: 100.0 }));
    assert_eq!(out.len(), 1);
    let resized = FigVec2::from_value(out[0].figma.get("size").unwrap()).unwrap();
    // 100 × 2 in x, untouched y.
    assert!((resized.x - 200.0).abs() < 0.001);
}

#[test]
fn strategy_0_applies_override_via_direct_guid() {
    let sym = symbol_root(vec![leaf("hero", 1, 10)]);
    // Derived (single-segment) names the real GUID → Strategy 0 wins.
    let derived = vec![guid_path(vec![guid(1, 10)])];
    let mut over_entry = ov_with(
        vec![guid(1, 10)],
        vec![("name", FigValue::Str("Renamed".into()))],
    );
    // Sanity: build a FigValue::Object with the override fields.
    if let FigValue::Object(pairs) = &mut over_entry {
        pairs.push(("ignored_skip".into(), FigValue::Null));
    }
    let out = apply_instance_overrides(&sym, Some(&[over_entry]), Some(&derived), None);
    assert_eq!(out[0].figma.get_str("name"), Some("Renamed"));
}

#[test]
fn strategy_0_takes_explicit_null_as_reset() {
    // TS copies an explicit null override (intentional reset).
    let sym = symbol_root(vec![leaf("hero", 1, 10)]);
    let derived = vec![guid_path(vec![guid(1, 10)])];
    let over_entry = ov_with(vec![guid(1, 10)], vec![("name", FigValue::Null)]);
    let out = apply_instance_overrides(&sym, Some(&[over_entry]), Some(&derived), None);
    // The original "hero" name was overwritten by Null — get_str now
    // returns None because FigValue::get treats Null as absent.
    assert_eq!(out[0].figma.get_str("name"), None);
}

#[test]
fn strategy_1_index_maps_when_count_matches() {
    // Two children, two single-segment derived entries with virtual
    // GUIDs that do NOT name real subtree nodes → direct_matches == 0,
    // len1.len == flat_symbol.len (which is 3: symbol root + 2 leaves).
    let sym = symbol_root(vec![leaf("a", 1, 10), leaf("b", 1, 11)]);
    // 3 derived entries to match flat_symbol.len() == 3.
    let derived = vec![
        guid_path(vec![guid(9, 100)]),
        guid_path(vec![guid(9, 101)]),
        guid_path(vec![guid(9, 102)]),
    ];
    // Override the second child via index 2 (positions 0/1/2 = symbol/a/b).
    let over_b = ov_with(
        vec![guid(9, 102)],
        vec![("name", FigValue::Str("B-Override".into()))],
    );
    let out = apply_instance_overrides(&sym, Some(&[over_b]), Some(&derived), None);
    // flat_symbol[0] = symbol_root (sid 0 lid 0)
    // flat_symbol[1] = leaf "a" (first child by localID = 10)
    // flat_symbol[2] = leaf "b" (lid 11)
    // index 2 → leaf "b" gets renamed.
    let names: Vec<Option<&str>> = out.iter().map(|c| c.figma.get_str("name")).collect();
    assert!(names.contains(&Some("B-Override")), "{names:?}");
    assert!(names.contains(&Some("a")), "{names:?}");
}

/// Ant Design's `newest version number` component has exactly two
/// nodes (SYMBOL root + TEXT) and two virtual entries. Figma allocated
/// the TEXT first, while Strategy 1's root-first index mapping did the
/// opposite and silently applied the text style to the discarded root.
#[test]
fn strategy_1_routes_unique_text_style_to_unique_text_node() {
    let mut text = leaf("4.23.3", 179_429, 170_443);
    text.figma.set("type", FigValue::Str("TEXT".into()));
    text.figma.set("size", size(51.0, 22.0));
    text.figma.set("fontSize", FigValue::Float(14.0));
    text.figma.set(
        "fontName",
        obj(vec![
            ("family", FigValue::Str("Menlo".into())),
            ("style", FigValue::Str("Regular".into())),
        ]),
    );
    let mut sym = symbol_root(vec![text]);
    sym.figma.set("size", size(51.0, 22.0));

    let derived = vec![
        guid_path(vec![guid(1_887, 2_339)]),
        guid_path(vec![guid(1_887, 2_340)]),
    ];
    let overrides = vec![
        ov_with(
            vec![guid(1_887, 2_339)],
            vec![
                ("fontSize", FigValue::Float(38.0)),
                (
                    "fontName",
                    obj(vec![
                        ("family", FigValue::Str("Roboto".into())),
                        ("style", FigValue::Str("Medium".into())),
                    ]),
                ),
                (
                    "lineHeight",
                    obj(vec![
                        ("value", FigValue::Float(46.0)),
                        ("units", FigValue::Str("PIXELS".into())),
                    ]),
                ),
            ],
        ),
        ov_with(vec![guid(1_887, 2_340)], vec![("size", size(108.0, 46.0))]),
    ];

    let out = apply_instance_overrides(
        &sym,
        Some(&overrides),
        Some(&derived),
        Some(FigVec2 { x: 108.0, y: 46.0 }),
    );
    let text = &out[0].figma;
    assert_eq!(text.get_f64("fontSize"), Some(38.0));
    assert_eq!(
        text.get("fontName").and_then(|font| font.get_str("family")),
        Some("Roboto")
    );
    assert_eq!(
        text.get("fontName").and_then(|font| font.get_str("style")),
        Some("Medium")
    );
    assert_eq!(
        text.get("lineHeight")
            .and_then(|line_height| line_height.get_f64("value")),
        Some(46.0)
    );
    let resolved_size = text
        .get("size")
        .and_then(FigVec2::from_value)
        .expect("text keeps the resolved instance size");
    assert!((resolved_size.x - 108.0).abs() < 0.001);
    assert!((resolved_size.y - 46.0).abs() < 0.001);
}

#[test]
fn strategy_2_resolves_via_virtual_guid_dfs() {
    // Symbol with one child. Virtual GUID base = (sessionID 7, firstLID 50).
    // Derived has TWO single-segment entries (virtual). flat_symbol has 2
    // entries (symbol + child); count matches → Strategy 1 wins. To
    // force Strategy 2, add an extra child so counts diverge.
    let sym = symbol_root(vec![leaf("a", 1, 10), leaf("b", 1, 11), leaf("c", 1, 12)]);
    // 2 derived entries → not equal to flat_symbol.len() (4), and the
    // virtual GUIDs are not real subtree nodes → direct_matches=0, so
    // Strategy 2 fires.
    let derived = vec![guid_path(vec![guid(7, 50)]), guid_path(vec![guid(7, 51)])];
    // The override targets virtual GUID 7:51 which the "full" walk
    // (starting at children) maps to the second visited node — leaf "b"
    // (localID 11 → second after sort).
    let over = ov_with(
        vec![guid(7, 51)],
        vec![("name", FigValue::Str("B-via-virtual".into()))],
    );
    let out = apply_instance_overrides(&sym, Some(&[over]), Some(&derived), None);
    let names: Vec<Option<&str>> = out.iter().map(|c| c.figma.get_str("name")).collect();
    // Children sort by localID: a(10), b(11), c(12). full_idx starts at
    // 0 on "a" → virtual 7:50. idx 1 on "b" → 7:51. So "b" is renamed.
    assert!(names.contains(&Some("B-via-virtual")), "{names:?}");
}

#[test]
fn strategy_2_skips_overrides_that_target_root_in_root_walk() {
    // When the root-anchored walk maps a virtual GUID to the symbol
    // root itself, that override must not flow into the children. We
    // can't directly observe a no-op on the symbol root via the
    // returned children — but we CAN see that the children remain
    // unchanged.
    let sym = symbol_root(vec![leaf("a", 1, 10), leaf("b", 1, 11)]);
    let derived = vec![guid_path(vec![guid(8, 60)])];
    // Virtual GUID 8:60 anchored at the ROOT walk: rootIdx=0 names the
    // symbol_root itself → override is filtered out.
    let over = ov_with(
        vec![guid(8, 60)],
        vec![("name", FigValue::Str("ShouldNotShow".into()))],
    );
    let out = apply_instance_overrides(&sym, Some(&[over]), Some(&derived), None);
    let names: Vec<Option<&str>> = out.iter().map(|c| c.figma.get_str("name")).collect();
    assert!(!names.contains(&Some("ShouldNotShow")), "{names:?}");
}

#[test]
fn strategy_3_fallback_index_maps_when_no_virtual_base() {
    // Force Strategy 3 by providing only multi-segment derived entries
    // (len1_derived is empty, so virtual_guid_base returns None;
    // direct_matches == 0; len1==0 so use_direct is true via the empty
    // branch). Actually — when len1_derived is empty we fall into
    // Strategy 0's "len1_derived.is_empty()" branch (use_direct = true)
    // and the for-loop is empty, but the single-segment override pickup
    // still runs.
    //
    // So this test instead verifies the empty-len1 + single-segment
    // override pickup, which is part of Strategy 0's body.
    let sym = symbol_root(vec![leaf("a", 1, 10)]);
    let over_real = ov_with(
        vec![guid(1, 10)],
        vec![("name", FigValue::Str("Direct".into()))],
    );
    let out = apply_instance_overrides(&sym, Some(&[over_real]), None, None);
    assert_eq!(out[0].figma.get_str("name"), Some("Direct"));
}

#[test]
fn skipped_keys_never_flow_into_node() {
    let sym = symbol_root(vec![leaf("hero", 1, 10)]);
    let derived = vec![guid_path(vec![guid(1, 10)])];
    let over = ov_with(
        vec![guid(1, 10)],
        vec![
            // Real prop.
            ("name", FigValue::Str("Hello".into())),
            // Blacklisted prop must NOT overwrite the node.
            ("guidPath", FigValue::Str("not-a-path".into())),
            ("componentKey", FigValue::Str("XX".into())),
        ],
    );
    let out = apply_instance_overrides(&sym, Some(&[over]), Some(&derived), None);
    assert_eq!(out[0].figma.get_str("name"), Some("Hello"));
    // Blacklisted keys were NOT copied — the node's existing guidPath
    // shape is unchanged (none was present, so it stays absent).
    assert!(out[0].figma.get("componentKey").is_none());
}

#[test]
fn nested_overrides_forwarded_into_instance_subtree() {
    // Build a symbol that contains an INSTANCE child + two leaves,
    // with a multi-segment override targeting the instance subtree.
    let inst = TreeNode {
        figma: obj(vec![
            ("type", FigValue::Str("INSTANCE".into())),
            ("guid", guid(1, 20)),
            ("name", FigValue::Str("InstanceChild".into())),
        ]),
        children: vec![],
    };
    let sym = symbol_root(vec![inst.clone(), leaf("a", 1, 10)]);
    let derived = vec![guid_path(vec![guid(1, 20)])];
    // Multi-segment override — head = the instance's guid (1:20).
    let nested = ov_with(
        vec![guid(1, 20), guid(2, 30)],
        vec![("name", FigValue::Str("nested-name".into()))],
    );
    let out = apply_instance_overrides(&sym, Some(&[nested]), Some(&derived), None);
    // Find the INSTANCE child and verify symbolData.symbolOverrides
    // grew by 1 nested entry whose guidPath dropped the head segment.
    let inst_out = out
        .iter()
        .find(|c| c.figma.get_str("type") == Some("INSTANCE"))
        .expect("instance child present");
    let sym_data = inst_out
        .figma
        .get("symbolData")
        .expect("symbolData populated");
    let overrides = sym_data
        .get_array("symbolOverrides")
        .expect("symbolOverrides populated");
    assert_eq!(overrides.len(), 1);
    let first = &overrides[0];
    let guids = first
        .get("guidPath")
        .and_then(|p| p.get_array("guids"))
        .expect("nested guidPath present");
    // Head dropped — only the second guid remains.
    assert_eq!(guids.len(), 1);
    let g = FigGuid::from_value(&guids[0]).unwrap();
    assert_eq!((g.session_id, g.local_id), (2, 30));
}

/// A Supporter row carries authored defaults in its nested INSTANCE.
/// Newer rows put the person's name in a component text assignment on
/// the parent INSTANCE, while forwarding the image on the leaf pk.
#[test]
fn nested_override_forwarding_prefers_forwarded_fields_on_pk_collision() {
    let old_image = obj(vec![
        ("type", FigValue::Str("IMAGE".into())),
        ("image", obj(vec![("hash", FigValue::Bytes(vec![0x11; 4]))])),
    ]);
    let new_image = obj(vec![
        ("type", FigValue::Str("IMAGE".into())),
        ("image", obj(vec![("hash", FigValue::Bytes(vec![0x22; 4]))])),
    ]);
    let authored = ov_with(
        vec![guid(2, 30)],
        vec![
            (
                "textData",
                obj(vec![(
                    "characters",
                    FigValue::Str("Ant Design Part 01".into()),
                )]),
            ),
            ("fontSize", FigValue::Float(14.0)),
            ("fillPaints", FigValue::Array(vec![old_image])),
        ],
    );
    let inst = TreeNode {
        figma: obj(vec![
            ("type", FigValue::Str("INSTANCE".into())),
            ("guid", guid(1, 20)),
            (
                "symbolData",
                obj(vec![("symbolOverrides", FigValue::Array(vec![authored]))]),
            ),
        ]),
        children: vec![],
    };
    let sym = symbol_root(vec![inst]);
    let derived = vec![guid_path(vec![guid(1, 20)])];
    let text_assignment = ov_with(
        vec![guid(1, 20)],
        vec![(
            "componentPropAssignments",
            FigValue::Array(vec![obj(vec![(
                "value",
                obj(vec![(
                    "textValue",
                    obj(vec![("characters", FigValue::Str("Alex Tijero".into()))]),
                )]),
            )])]),
        )],
    );
    let forwarded = ov_with(
        vec![guid(1, 20), guid(2, 30)],
        vec![("fillPaints", FigValue::Array(vec![new_image]))],
    );

    let out = apply_instance_overrides(
        &sym,
        Some(&[text_assignment, forwarded]),
        Some(&derived),
        None,
    );
    let merged = out[0]
        .figma
        .get("symbolData")
        .and_then(|data| data.get_array("symbolOverrides"))
        .and_then(|entries| entries.first())
        .expect("nested override survives forwarding");
    assert_eq!(
        merged
            .get("textData")
            .and_then(|text| text.get_str("characters")),
        Some("Alex Tijero")
    );
    assert_eq!(merged.get_f64("fontSize"), Some(14.0));
    let hash = merged
        .get_array("fillPaints")
        .and_then(|paints| paints.first())
        .and_then(|paint| paint.get("image"))
        .and_then(|image| image.get("hash"));
    assert_eq!(hash, Some(&FigValue::Bytes(vec![0x22; 4])));
}

use crate::figma_types::FigGuid;

/// Sidebar-logo regression shape: a nested INSTANCE arrives with its
/// OWN authored derivedSymbolData (the icon's scale + fill targets).
/// Forwarded nested derived entries must MERGE with the authored
/// list — replacing it strips the icon's sizing data and its
/// overrides silently stop applying downstream.
#[test]
fn nested_derived_forwarding_preserves_authored_derived_data() {
    let mut inst_figma = obj(vec![
        ("type", FigValue::Str("INSTANCE".into())),
        ("guid", guid(1, 20)),
        ("name", FigValue::Str("InstanceChild".into())),
    ]);
    // Authored derived data of the nested instance itself.
    inst_figma.set(
        "derivedSymbolData",
        FigValue::Array(vec![derived_with(
            vec![guid(2, 30)],
            vec![("size", size(43.0, 43.0))],
        )]),
    );
    let inst = TreeNode {
        figma: inst_figma,
        children: vec![],
    };
    let sym = symbol_root(vec![inst, leaf("a", 1, 10)]);
    let derived = vec![
        guid_path(vec![guid(1, 20)]),
        // Multi-segment derived — forwarded into the instance. 2:30
        // duplicates the authored pk (field-merge, forwarded wins);
        // 2:31 is new (appended).
        derived_with(
            vec![guid(1, 20), guid(2, 30)],
            vec![("size", size(9.0, 9.0)), ("transform", obj(vec![]))],
        ),
        derived_with(
            vec![guid(1, 20), guid(2, 31)],
            vec![("size", size(10.0, 10.0))],
        ),
    ];
    let out = apply_instance_overrides(&sym, None, Some(&derived), None);
    let inst_out = out
        .iter()
        .find(|c| c.figma.get_str("type") == Some("INSTANCE"))
        .expect("instance child present");
    let entries = inst_out
        .figma
        .get_array("derivedSymbolData")
        .expect("derivedSymbolData present");
    let pks: Vec<String> = entries
        .iter()
        .filter_map(|e| {
            e.get("guidPath")
                .and_then(|p| p.get_array("guids"))
                .and_then(|g| g.first())
                .and_then(|g| FigGuid::from_value(g).map(|x| x.to_key()))
        })
        .collect();
    assert_eq!(
        pks,
        vec!["2:30".to_string(), "2:31".to_string()],
        "authored entry survives, duplicate pk merges, new pk appends"
    );
    let e30 = &entries[0];
    let sz = e30
        .get("size")
        .and_then(FigVec2::from_value)
        .expect("merged entry keeps a size");
    assert!(
        (sz.x - 9.0).abs() < 0.001,
        "outer forwarded field must win the merge, got {}",
        sz.x
    );
    assert!(
        e30.get("transform").is_some(),
        "forwarded-only field must be merged in"
    );
}

// ── Guessed-mapping confidence gate ───────────────────────────────

pub(super) fn sized_leaf(name: &str, lid: u32, w: f32, h: f32, x: f32) -> TreeNode {
    TreeNode {
        figma: obj(vec![
            ("type", FigValue::Str("RECTANGLE".into())),
            ("guid", guid(1, lid)),
            ("name", FigValue::Str(name.into())),
            ("size", size(w, h)),
            (
                "transform",
                obj(vec![
                    ("m00", FigValue::Float(1.0)),
                    ("m01", FigValue::Float(0.0)),
                    ("m02", FigValue::Float(x)),
                    ("m10", FigValue::Float(0.0)),
                    ("m11", FigValue::Float(1.0)),
                    ("m12", FigValue::Float(0.0)),
                ]),
            ),
        ]),
        children: vec![],
    }
}

pub(super) fn derived_with(guids: Vec<FigValue>, extra: Vec<(&str, FigValue)>) -> FigValue {
    ov_with(guids, extra)
}

pub(super) fn transform_m02(v: f32) -> (&'static str, FigValue) {
    (
        "transform",
        obj(vec![
            ("m00", FigValue::Float(1.0)),
            ("m01", FigValue::Float(0.0)),
            ("m02", FigValue::Float(v)),
            ("m10", FigValue::Float(0.0)),
            ("m11", FigValue::Float(1.0)),
            ("m12", FigValue::Float(0.0)),
        ]),
    )
}

/// Mirrors the real-world failure (Test.fig orderCard): Strategy 2's
/// walk-order guess maps derived entries onto the wrong nodes. The
/// give-away is derived sizes grossly contradicting the mapped nodes'
/// own sizes (321 px wide data landing on 76/47 px nodes). When ≥2
/// entries are that far off, the whole guessed mapping is untrusted
/// and NO derived transform/size may be applied.
#[test]
fn strategy_2_gross_size_mismatch_abandons_guessed_mapping() {
    let sym = symbol_root(vec![
        sized_leaf("iphone", 10, 65.0, 17.0, 0.0),
        sized_leaf("right", 11, 76.0, 15.0, 244.0),
        sized_leaf("pending", 12, 47.0, 15.0, 15.0),
    ]);
    // Strategy 2 fires: 3 single-segment virtual entries ≠ flat len (4),
    // virtual base (9, 50). walk_virtual maps 9:50→iphone, 9:51→right,
    // 9:52→pending.
    let derived = vec![
        derived_with(vec![guid(9, 50)], vec![transform_m02(245.0)]),
        derived_with(vec![guid(9, 51)], vec![("size", size(321.0, 17.0))]),
        derived_with(vec![guid(9, 52)], vec![("size", size(321.0, 19.0))]),
    ];
    let out = apply_instance_overrides(&sym, None, Some(&derived), None);
    // Gate trips (two grossly-mismatched sizes) → derived data is NOT
    // applied: "iphone" keeps its authored x=0 instead of jumping to 245.
    let iphone = out
        .iter()
        .find(|n| n.figma.get_str("name") == Some("iphone"))
        .expect("iphone child present");
    let m02 = iphone
        .figma
        .get("transform")
        .and_then(|t| t.get_f64("m02"))
        .unwrap_or(0.0);
    assert!(
        (m02 - 0.0).abs() < 0.001,
        "gated mapping must not move iphone to 245, got m02={m02}"
    );
    // And "right" keeps its authored 76×15 size.
    let right = out
        .iter()
        .find(|n| n.figma.get_str("name") == Some("right"))
        .expect("right child present");
    let sz = FigVec2::from_value(right.figma.get("size").unwrap()).unwrap();
    assert!((sz.x - 76.0).abs() < 0.001, "right width mangled: {}", sz.x);
}

/// Control: when the guessed mapping's derived sizes agree with the
/// mapped nodes (within stretch tolerance), the mapping is trusted and
/// derived transforms still apply.
#[test]
fn strategy_2_consistent_sizes_keep_derived_application() {
    let sym = symbol_root(vec![
        sized_leaf("iphone", 10, 65.0, 17.0, 0.0),
        sized_leaf("right", 11, 76.0, 15.0, 244.0),
        sized_leaf("pending", 12, 47.0, 15.0, 15.0),
    ]);
    let derived = vec![
        derived_with(vec![guid(9, 50)], vec![transform_m02(5.0)]),
        derived_with(vec![guid(9, 51)], vec![("size", size(77.0, 15.0))]),
        derived_with(vec![guid(9, 52)], vec![("size", size(48.0, 15.0))]),
    ];
    let out = apply_instance_overrides(&sym, None, Some(&derived), None);
    let iphone = out
        .iter()
        .find(|n| n.figma.get_str("name") == Some("iphone"))
        .expect("iphone child present");
    let m02 = iphone
        .figma
        .get("transform")
        .and_then(|t| t.get_f64("m02"))
        .unwrap_or(-1.0);
    assert!(
        (m02 - 5.0).abs() < 0.001,
        "trusted mapping must apply derived transform, got m02={m02}"
    );
}

/// Strategy 1's positional guess maps the symbol ROOT too, but root
/// derived data never applies to the returned children — a severe
/// root-size entry must not count toward the gate. With the root
/// skipped only one severe child remains, which is below the gate
/// threshold, so the mapping stays trusted.
#[test]
fn gate_ignores_symbol_root_size_mismatch() {
    let sym = symbol_root(vec![
        sized_leaf("a", 10, 100.0, 100.0, 0.0),
        sized_leaf("b", 11, 100.0, 100.0, 0.0),
    ]);
    // 3 single-segment virtual entries == flat_symbol.len() (root+2)
    // → Strategy 1 positional mapping: entry0→root, entry1→a, entry2→b.
    let derived = vec![
        derived_with(vec![guid(9, 100)], vec![("size", size(400.0, 400.0))]),
        derived_with(vec![guid(9, 101)], vec![("size", size(400.0, 400.0))]),
        derived_with(vec![guid(9, 102)], vec![transform_m02(5.0)]),
    ];
    let out = apply_instance_overrides(&sym, None, Some(&derived), None);
    let b = out
        .iter()
        .find(|n| n.figma.get_str("name") == Some("b"))
        .expect("b child present");
    let m02 = b
        .figma
        .get("transform")
        .and_then(|t| t.get_f64("m02"))
        .unwrap_or(-1.0);
    assert!(
        (m02 - 5.0).abs() < 0.001,
        "root mismatch must not trip the gate; b should get m02=5, got {m02}"
    );
}

/// Two severe entries among many consistent ones are legitimate
/// per-node stretches, not proof of a wrong guess — the gate needs a
/// severe MAJORITY. 2 severe of 5 comparable stays trusted.
#[test]
fn gate_requires_severe_majority() {
    let sym = symbol_root(vec![
        sized_leaf("a", 10, 100.0, 100.0, 0.0),
        sized_leaf("b", 11, 100.0, 100.0, 0.0),
        sized_leaf("c", 12, 100.0, 100.0, 0.0),
        sized_leaf("d", 13, 100.0, 100.0, 0.0),
        sized_leaf("e", 14, 100.0, 100.0, 0.0),
        sized_leaf("f", 15, 100.0, 100.0, 0.0),
    ]);
    // 6 virtual entries ≠ flat len (7) → Strategy 2. a+b severely
    // stretched (legit per-node resize), c/d/e near-authored, f gets a
    // transform. severe=2 of comparable=5 → minority → trusted.
    let derived = vec![
        derived_with(vec![guid(9, 50)], vec![("size", size(400.0, 400.0))]),
        derived_with(vec![guid(9, 51)], vec![("size", size(400.0, 400.0))]),
        derived_with(vec![guid(9, 52)], vec![("size", size(101.0, 100.0))]),
        derived_with(vec![guid(9, 53)], vec![("size", size(102.0, 100.0))]),
        derived_with(vec![guid(9, 54)], vec![("size", size(103.0, 100.0))]),
        derived_with(vec![guid(9, 55)], vec![transform_m02(7.0)]),
    ];
    let out = apply_instance_overrides(&sym, None, Some(&derived), None);
    let f = out
        .iter()
        .find(|n| n.figma.get_str("name") == Some("f"))
        .expect("f child present");
    let m02 = f
        .figma
        .get("transform")
        .and_then(|t| t.get_f64("m02"))
        .unwrap_or(-1.0);
    assert!(
        (m02 - 7.0).abs() < 0.001,
        "severe minority must not trip the gate; f should get m02=7, got {m02}"
    );
}

/// When the gate DOES trip, multi-segment entries whose head names a
/// real subtree node are mapping-independent and must still be
/// forwarded into that node's nested override slots.
#[test]
fn gate_fallback_keeps_real_guid_nested_forwarding() {
    // Forwarding only lands on INSTANCE nodes, so the head child must
    // be one.
    let mut nested_instance = sized_leaf("nested", 12, 50.0, 50.0, 0.0);
    nested_instance
        .figma
        .set("type", FigValue::Str("INSTANCE".into()));
    let sym = symbol_root(vec![
        sized_leaf("x", 10, 65.0, 17.0, 0.0),
        sized_leaf("y", 11, 76.0, 15.0, 244.0),
        nested_instance,
    ]);
    let derived = vec![
        // Two gross mismatches → gate trips (2 of 2 severe).
        derived_with(vec![guid(9, 50)], vec![("size", size(321.0, 17.0))]),
        derived_with(vec![guid(9, 51)], vec![("size", size(321.0, 19.0))]),
        // Real-GUID head (1:12) + nested segment — unambiguous.
        derived_with(
            vec![guid(1, 12), guid(11, 8523)],
            vec![("fontSize", FigValue::Float(20.0))],
        ),
    ];
    let out = apply_instance_overrides(&sym, None, Some(&derived), None);
    let nested = out
        .iter()
        .find(|n| n.figma.get_str("name") == Some("nested"))
        .expect("nested child present");
    let forwarded = nested
        .figma
        .get("derivedSymbolData")
        .and_then(|v| match v {
            FigValue::Array(a) => Some(a.len()),
            _ => None,
        })
        .unwrap_or(0);
    assert_eq!(
        forwarded, 1,
        "real-GUID-headed nested derived entry must survive the gate fallback"
    );
    // And the gate still protected the guessed mapping: x keeps x=0.
    let x = out
        .iter()
        .find(|n| n.figma.get_str("name") == Some("x"))
        .expect("x child present");
    let sz = FigVec2::from_value(x.figma.get("size").unwrap()).unwrap();
    assert!((sz.x - 65.0).abs() < 0.001, "x size mangled: {}", sz.x);
}
