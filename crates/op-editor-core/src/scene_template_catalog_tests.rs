use super::*;

#[test]
fn shipped_catalogue_parses_and_every_entry_resolves_its_document() {
    let catalogue = scene_template_catalogue();
    assert!(
        !catalogue.is_empty(),
        "the shipped catalogue must not be empty"
    );
    for template in catalogue {
        // `document()` panics on a missing document; calling it here is the
        // assertion. A card whose document is absent is worse than a missing
        // card: it looks clickable and does nothing.
        assert!(
            !template.document().is_empty(),
            "{} has an empty document",
            template.id
        );
        assert!(template.frames > 0);
        assert!(template.frame_width > 0 && template.frame_height > 0);
    }
}

#[test]
fn embedded_documents_are_valid_canonical_json_with_the_declared_frame_count() {
    // The declared `frames` drives what the card promises ("5 页"), so a
    // template edited without updating its entry must fail here rather than
    // mislead the user at the point of choosing.
    for template in scene_template_catalogue() {
        let parsed: serde_json::Value = serde_json::from_str(template.document())
            .unwrap_or_else(|error| panic!("{} is not valid JSON: {error}", template.id));
        let children = parsed
            .get("children")
            .and_then(serde_json::Value::as_array)
            .unwrap_or_else(|| panic!("{} has no top-level children", template.id));
        assert_eq!(
            children.len(),
            usize::from(template.frames),
            "{} declares {} frames but its document has {}",
            template.id,
            template.frames,
            children.len()
        );
    }
}

#[test]
fn scene_filter_and_id_lookup_agree_with_the_catalogue() {
    let all: Vec<&str> = scene_template_catalogue()
        .iter()
        .map(|t| t.id.as_str())
        .collect();
    let by_scene: Vec<&str> = TemplateScene::ALL
        .iter()
        .flat_map(|scene| scene_templates_for(*scene))
        .map(|t| t.id.as_str())
        .collect();
    assert_eq!(
        all.len(),
        by_scene.len(),
        "every template must belong to a known scene"
    );
    for id in &all {
        assert!(scene_template_by_id(id).is_some(), "{id} not found by id");
    }
    assert!(scene_template_by_id("no-such-template").is_none());
}

#[test]
fn search_matches_chinese_and_english_regardless_of_locale() {
    let tutorial = scene_template_by_id("screenshot-tutorial").expect("shipped");
    assert!(tutorial.matches_query(Locale::EnUs, "教程"));
    assert!(tutorial.matches_query(Locale::ZhCn, "tutorial"));
    assert!(
        tutorial.matches_query(Locale::EnUs, ""),
        "empty query matches all"
    );
    assert!(!tutorial.matches_query(Locale::EnUs, "spreadsheet"));
}

#[test]
fn a_template_without_an_embedded_document_is_rejected_at_parse_time() {
    let source = r#"
[[template]]
id = "not-shipped"
scene = "slides"
title_key = "k"
title_fallback = "t"
summary_key = "s"
summary_fallback = "s"
tags = ["a"]
frames = 1
frame_width = 1920
frame_height = 1080
"#;
    let error = parse_scene_template_catalogue(source).expect_err("must reject");
    assert!(
        error.message.contains("no embedded document"),
        "unexpected: {error}"
    );
}

#[test]
fn malformed_entries_report_the_offending_line() {
    let missing_field = r#"
[[template]]
id = "screenshot-tutorial"
scene = "tutorial"
"#;
    let error = parse_scene_template_catalogue(missing_field).expect_err("must reject");
    assert!(error.message.contains("missing `"), "unexpected: {error}");

    let unknown_scene = r#"
[[template]]
id = "screenshot-tutorial"
scene = "podcast"
"#;
    let error = parse_scene_template_catalogue(unknown_scene).expect_err("must reject");
    assert!(
        error.message.contains("unknown scene"),
        "unexpected: {error}"
    );

    let stray = "id = \"orphan\"\n";
    let error = parse_scene_template_catalogue(stray).expect_err("must reject");
    assert!(
        error.message.contains("before `[[template]]`"),
        "unexpected: {error}"
    );

    let unknown_field = r#"
[[template]]
id = "screenshot-tutorial"
colour = "red"
"#;
    let error = parse_scene_template_catalogue(unknown_field).expect_err("must reject");
    assert!(
        error.message.contains("unknown field"),
        "unexpected: {error}"
    );
}

/// A scene with no templates paints a chip that filters the grid to empty.
///
/// The scene enum and the catalogue are edited separately — adding a variant
/// is a compiler-guided change, adding its templates is not — so nothing else
/// stops a chip from shipping ahead of the documents it filters for.
#[test]
fn every_scene_has_at_least_one_template_so_no_filter_chip_is_dead() {
    for scene in TemplateScene::ALL {
        assert!(
            scene_templates_for(scene).next().is_some(),
            "scene `{}` has no templates — its filter chip would show an empty grid",
            scene.as_str()
        );
    }
}

#[test]
fn every_scene_has_a_distinct_label_so_the_filter_row_is_unambiguous() {
    let labels: Vec<&str> = TemplateScene::ALL
        .iter()
        .map(|scene| scene.title_fallback())
        .collect();
    let unique: std::collections::HashSet<_> = labels.iter().collect();
    assert_eq!(
        unique.len(),
        labels.len(),
        "duplicate scene labels: {labels:?}"
    );
    for scene in TemplateScene::ALL {
        assert_eq!(scene.as_str().parse::<TemplateScene>(), Ok(scene));
    }
}

/// Multi-frame templates must lay their boards out, not stack them.
///
/// A frame with no `x`/`y` defaults to the origin, so a six-slide deck opened
/// as one visible board with five hidden underneath it (reported 2026-08-02).
/// Per-frame rendering never shows this — only opening the document does.
#[test]
fn a_multi_frame_template_positions_every_frame() {
    for template in scene_template_catalogue() {
        if template.frames < 2 {
            continue;
        }
        let parsed: serde_json::Value =
            serde_json::from_str(template.document()).expect("valid json");
        let children = parsed["children"].as_array().expect("children");

        let mut seen: Vec<(f64, f64)> = Vec::new();
        for (index, frame) in children.iter().enumerate() {
            let x = frame["x"].as_f64().unwrap_or_else(|| {
                panic!(
                    "{}: frame {index} has no x — it would stack at the origin",
                    template.id
                )
            });
            let y = frame["y"]
                .as_f64()
                .unwrap_or_else(|| panic!("{}: frame {index} has no y", template.id));
            assert!(
                !seen.contains(&(x, y)),
                "{}: frame {index} sits on top of an earlier one at ({x}, {y})",
                template.id
            );
            seen.push((x, y));
        }
    }
}

#[test]
fn every_shipped_template_has_a_document_route_under_the_shared_prefix() {
    // The route is how the browser identifies a template document, and it is
    // what the catalogue's load-time check now asks about — on wasm the bytes
    // are not in the binary, so checking for them there would reject the whole
    // catalogue at start-up.
    use std::collections::HashSet;

    use crate::scene_template_catalog::scene_template_document_route;

    let mut routes = HashSet::new();
    for template in scene_template_catalogue() {
        let route = scene_template_document_route(&template.id)
            .unwrap_or_else(|| panic!("{} has no document route", template.id));
        assert_eq!(
            route,
            format!(
                "{}scene_templates/{}.op",
                crate::web_assets::WEB_ASSET_ROUTE_PREFIX,
                template.id
            ),
            "{} route must match the staged bundle layout",
            template.id
        );
        assert!(routes.insert(route), "{} duplicates a route", template.id);
    }
    assert!(
        scene_template_document_route("no-such-template").is_none(),
        "an unknown id has no route, which is how a caller tells it apart from \
         a shipped template whose document has not been fetched yet"
    );
}
