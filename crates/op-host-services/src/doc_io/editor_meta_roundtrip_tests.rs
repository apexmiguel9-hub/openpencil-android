use super::{load_editor_state, load_editor_state_from_source, save_to_path};
use op_editor_core::EditorState;
use std::path::PathBuf;

fn temp_op_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "openpencil-preserve-geometry-{}-{}.op",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ))
}

#[test]
fn authored_geometry_mode_survives_save_load_and_source_round_trips() {
    let document = jian_ops_schema::load_str(
        r#"{
          "version":"1.0.0",
          "children":[{
            "type":"frame","id":"root","x":10,"y":20,
            "width":120,"height":140,"layout":"vertical",
            "children":[
              {"type":"rectangle","id":"bottom","x":8,"y":70,"width":40,"height":20},
              {"type":"rectangle","id":"top","x":8,"y":10,"width":40,"height":20}
            ]
          }]
        }"#,
    )
    .expect("fixture parses")
    .value;
    let mut state = EditorState::from_document(document);
    state.editor_ui.preserve_authored_geometry = true;
    let path = temp_op_path();

    save_to_path(&state, &path).expect("save succeeds");
    let source = std::fs::read_to_string(&path).expect("saved source");
    let from_path =
        load_editor_state(&path, op_editor_core::Locale::EnUs).expect("path load succeeds");
    let from_source = load_editor_state_from_source(&source, op_editor_core::Locale::EnUs)
        .expect("source load succeeds");

    for reopened in [&from_path, &from_source] {
        assert!(reopened.editor_ui.preserve_authored_geometry);
        let scene = op_pen_loader::editor_state_to_active_page_layout_scene(reopened);
        let page = scene.active_page().expect("active page");
        assert_eq!(page.find("bottom").expect("bottom").bounds.origin.y, 90.0);
        assert_eq!(page.find("top").expect("top").bounds.origin.y, 30.0);
    }
    let saved: serde_json::Value = serde_json::from_str(&source).expect("saved JSON");
    assert_eq!(saved["editorMeta"]["preserveAuthoredGeometry"], true);

    let _ = std::fs::remove_file(path);
}

#[test]
fn scenario_survives_save_load_and_ignores_an_unknown_tag() {
    use op_editor_core::scene_template_catalog::TemplateScene;

    let mut state = EditorState::from_document(
        jian_ops_schema::load_str(
            r#"{"version":"1.0.0","children":[{"type":"frame","id":"slide-1","width":1920,"height":1080}]}"#,
        )
        .expect("fixture parses")
        .value,
    );
    state.editor_ui.scenario = Some(TemplateScene::Slides);
    let path = temp_op_path();

    save_to_path(&state, &path).expect("save succeeds");
    let source = std::fs::read_to_string(&path).expect("saved source");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&source).expect("saved JSON")["editorMeta"]
            ["scenario"],
        "slides",
        "the tag is written as the kebab-case scene name"
    );

    for reopened in [
        load_editor_state(&path, op_editor_core::Locale::EnUs).expect("path load succeeds"),
        load_editor_state_from_source(&source, op_editor_core::Locale::EnUs)
            .expect("source load succeeds"),
    ] {
        assert_eq!(reopened.editor_ui.scenario, Some(TemplateScene::Slides));
    }

    // A tag from a future build, a hand edit, or the wrong type must cost the
    // hint and nothing else — the document still opens.
    for garbage in [
        r#""interpretive-dance""#,
        r#"7"#,
        r#"null"#,
        r#"{"scene":"slides"}"#,
    ] {
        let state = load_editor_state_from_source(
            &format!(
                r#"{{"version":"1.0.0","editorMeta":{{"activePageIndex":0,"scenario":{garbage}}},"children":[]}}"#
            ),
            op_editor_core::Locale::EnUs,
        )
        .unwrap_or_else(|error| panic!("{garbage} must still load: {error}"));
        assert_eq!(state.editor_ui.scenario, None, "{garbage}");
    }

    let _ = std::fs::remove_file(path);
}

/// A document saved before the tag existed opens as an ordinary design.
#[test]
fn source_without_a_scenario_tag_loads_untagged() {
    let state = load_editor_state_from_source(
        r#"{"version":"1.0.0","editorMeta":{"activePageIndex":0},"children":[]}"#,
        op_editor_core::Locale::EnUs,
    )
    .expect("legacy metadata loads");

    assert_eq!(state.editor_ui.scenario, None);
}

#[test]
fn source_without_geometry_metadata_keeps_legacy_normal_layout() {
    let state = load_editor_state_from_source(
        r#"{"version":"1.0.0","editorMeta":{"activePageIndex":0},"children":[]}"#,
        op_editor_core::Locale::EnUs,
    )
    .expect("legacy metadata loads");

    assert!(!state.editor_ui.preserve_authored_geometry);
}

#[test]
fn empty_file_keeps_the_source_parser_error() {
    let path = temp_op_path();
    std::fs::write(&path, []).expect("write empty fixture");

    let path_error = load_editor_state(&path, op_editor_core::Locale::EnUs)
        .expect_err("an empty document is invalid");
    let source_error = load_editor_state_from_source("", op_editor_core::Locale::EnUs)
        .expect_err("the same empty source is invalid");

    assert_eq!(path_error, source_error);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn invalid_utf8_keeps_the_localized_load_error() {
    let path = temp_op_path();
    let bytes = vec![0xf0, 0x28, 0x8c, 0x28];
    std::fs::write(&path, &bytes).expect("write invalid UTF-8 fixture");

    let detail = std::str::from_utf8(&bytes)
        .expect_err("fixture is invalid UTF-8")
        .to_string();
    let expected = op_i18n::translate(op_editor_core::Locale::EnUs, "dialog.loadErrorInvalidUtf8")
        .replace("{{detail}}", &detail);
    let actual = load_editor_state(&path, op_editor_core::Locale::EnUs)
        .expect_err("invalid UTF-8 is rejected")
        .to_string();

    assert_eq!(actual, expected);
    let _ = std::fs::remove_file(&path);
}
