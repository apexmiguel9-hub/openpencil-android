//! Timeline + emission tests for the Hyperframes composition export.
//!
//! The slide MARKUP itself is not re-tested here — it is the structured
//! exporter's, and covered by its own suite. What is tested is
//! everything this module adds on top: which boards become scenes, how
//! long each one holds, that the windows tile the timeline exactly, and
//! that every number lands in the file in a spelling a browser and a
//! renderer both accept.

use super::*;
use op_editor_core::scene_template_catalog::TemplateScene;

fn deck_state(source: &str) -> EditorState {
    let doc = jian_ops_schema::load_str(source)
        .expect("fixture JSON parses")
        .value;
    let mut state = EditorState::from_document(doc);
    state.editor_ui.scenario = Some(TemplateScene::Slides);
    state
}

/// A deck of `boards`, each a 1920x1080 frame carrying `text`.
fn deck_of(boards: &[&str]) -> EditorState {
    let children: Vec<String> = boards
        .iter()
        .enumerate()
        .map(|(i, text)| {
            format!(
                r##"{{"type":"frame","id":"f{i}","name":"slide {i}","x":{x},"y":0,
                     "width":1920,"height":1080,"fill":[{{"type":"solid","color":"#ffffff"}}],
                     "children":[
                       {{"type":"text","id":"t{i}","x":100,"y":100,"width":1400,"height":200,
                        "content":"{text}","fontSize":48,
                        "fill":[{{"type":"solid","color":"#101828"}}]}}
                     ]}}"##,
                x = i * 2000,
            )
        })
        .collect();
    deck_state(&format!(
        r#"{{"version":"1.0.0","children":[{}]}}"#,
        children.join(",")
    ))
}

/// An attribute's seconds value back as ticks. Comparing timelines in
/// ticks keeps the assertions on integers: accumulating the parsed
/// decimals as floats is exactly the drift the exporter avoids, and a
/// test that reintroduced it could fail on arithmetic rather than on
/// the behaviour under test.
fn ticks_of(value: &str) -> u32 {
    (value.parse::<f64>().expect("attribute is a number") * f64::from(TICKS_PER_SECOND)).round()
        as u32
}

/// Every `(start, duration)` pair the composition declares, as written.
fn windows(html: &str) -> Vec<(String, String)> {
    html.split("class=\"clip hf-scene\"")
        .skip(1)
        .map(|scene| {
            let attr = |name: &str| {
                scene
                    .split(&format!("{name}=\""))
                    .nth(1)
                    .and_then(|rest| rest.split('"').next())
                    .unwrap_or_else(|| panic!("scene is missing {name}"))
                    .to_string()
            };
            (attr("data-start"), attr("data-duration"))
        })
        .collect()
}

#[test]
fn every_visible_board_becomes_exactly_one_scene() {
    let state = deck_of(&["one", "two", "three"]);

    let composition = deck_composition(&state).expect("deck builds");

    assert_eq!(composition.scenes, 3);
    assert_eq!(
        composition.html.matches("class=\"clip hf-scene\"").count(),
        3
    );
    assert_eq!(composition.html.matches("class=\"hf-slide\"").count(), 3);
    // Board order is the authored child order, same as the slideshow.
    let labels: Vec<&str> = composition
        .html
        .split("aria-label=\"")
        .skip(1)
        .filter_map(|rest| rest.split('"').next())
        .collect();
    assert_eq!(labels, vec!["slide 0", "slide 1", "slide 2"]);
}

#[test]
fn scene_windows_tile_the_timeline_with_no_gap_and_no_overlap() {
    // Three different text lengths, so the durations differ and a
    // hard-coded stride could not pass.
    let state = deck_of(&[
        "hi",
        "一二三四五六七八九十一二三四五六",
        &"word ".repeat(30),
    ]);

    let composition = deck_composition(&state).expect("deck builds");

    let windows = windows(&composition.html);
    assert_eq!(windows.len(), 3);
    let mut expected = 0;
    for (start, duration) in &windows {
        assert_eq!(
            ticks_of(start),
            expected,
            "scene must start exactly where the previous one ended: {windows:?}"
        );
        assert!(
            ticks_of(duration) > 0,
            "a scene with no duration never shows"
        );
        expected += ticks_of(duration);
    }
    assert_eq!(
        expected, composition.total_ticks,
        "the declared total must be the sum of the windows"
    );
    // The durations really did differ — otherwise this test would pass
    // on a constant-hold implementation.
    assert!(
        windows[0].1 != windows[1].1 && windows[1].1 != windows[2].1,
        "{windows:?}"
    );
}

#[test]
fn the_root_declares_the_canvas_and_the_whole_length() {
    let state = deck_of(&["one", "two"]);

    let composition = deck_composition(&state).expect("deck builds");

    assert!(
        composition
            .html
            .contains("data-composition-id=\"slide-0\" data-no-timeline data-start=\"0\""),
        "{}",
        composition.html
    );
    assert!(
        composition
            .html
            .contains("data-width=\"1920\" data-height=\"1080\""),
        "{}",
        composition.html
    );
    let total = seconds(composition.total_ticks);
    assert!(
        composition
            .html
            .contains(&format!("data-duration=\"{total}\" data-fps=")),
        "root duration missing: {}",
        composition.html
    );
    assert_eq!(composition.width, 1920.0);
    assert_eq!(composition.height, 1080.0);
}

#[test]
fn a_nearly_empty_slide_still_holds_the_readable_floor() {
    let state = deck_of(&["hi"]);

    let composition = deck_composition(&state).expect("deck builds");

    assert_eq!(composition.total_ticks, 30, "floor is {MIN_SECONDS}s");
    assert!(composition.html.contains("data-duration=\"3\""));
}

#[test]
fn a_wall_of_text_is_capped_rather_than_stretched() {
    let state = deck_of(&[&"字".repeat(2000)]);

    let composition = deck_composition(&state).expect("deck builds");

    assert_eq!(composition.total_ticks, 100, "ceiling is {MAX_SECONDS}s");
    assert!(composition.html.contains("data-duration=\"10\""));
}

#[test]
fn hold_time_between_the_bounds_follows_the_reading_formula() {
    // 40 CJK characters: 1.5 + 40/8 = 6.5s.
    let cjk = deck_of(&[&"字".repeat(40)]);
    assert_eq!(deck_composition(&cjk).expect("builds").total_ticks, 65);

    // 16 Latin words at 3 units each = 48 units: 1.5 + 48/8 = 7.5s.
    let latin = deck_of(&[&"word ".repeat(16)]);
    assert_eq!(deck_composition(&latin).expect("builds").total_ticks, 75);
}

#[test]
fn text_the_author_hid_is_not_paid_for_in_hold_time() {
    let visible = deck_state(
        r##"{"version":"1.0.0","children":[
            {"type":"frame","id":"f1","name":"s","x":0,"y":0,"width":1920,"height":1080,
             "fill":[{"type":"solid","color":"#ffffff"}],"children":[
               {"type":"text","id":"t1","x":10,"y":10,"width":800,"height":80,
                "content":"一二三四五六七八九十一二三四五六一二三四五六七八九十一二三四五六",
                "fontSize":48,"fill":[{"type":"solid","color":"#101828"}]}
             ]}
        ]}"##,
    );
    let hidden = deck_state(
        r##"{"version":"1.0.0","children":[
            {"type":"frame","id":"f1","name":"s","x":0,"y":0,"width":1920,"height":1080,
             "fill":[{"type":"solid","color":"#ffffff"}],"children":[
               {"type":"text","id":"t1","x":10,"y":10,"width":800,"height":80,"visible":false,
                "content":"一二三四五六七八九十一二三四五六一二三四五六七八九十一二三四五六",
                "fontSize":48,"fill":[{"type":"solid","color":"#101828"}]}
             ]}
        ]}"##,
    );

    let with_text = deck_composition(&visible).expect("builds").total_ticks;
    let without = deck_composition(&hidden).expect("builds").total_ticks;

    assert_eq!(with_text, 55, "32 CJK chars: 1.5 + 32/8 = 5.5s");
    assert_eq!(without, 30, "hidden text reads as an empty slide");
}

#[test]
fn a_hidden_board_takes_no_time_on_the_timeline_at_all() {
    let state = deck_state(
        r##"{"version":"1.0.0","children":[
            {"type":"frame","id":"f1","name":"one","x":0,"y":0,"width":1920,"height":1080,
             "fill":[{"type":"solid","color":"#ff0000"}]},
            {"type":"frame","id":"f2","name":"skipped","x":2000,"y":0,"width":1920,"height":1080,
             "visible":false,"fill":[{"type":"solid","color":"#00ff00"}]},
            {"type":"frame","id":"f3","name":"two","x":4000,"y":0,"width":1920,"height":1080,
             "fill":[{"type":"solid","color":"#0000ff"}]}
        ]}"##,
    );

    let composition = deck_composition(&state).expect("deck builds");

    assert_eq!(composition.scenes, 2);
    assert!(!composition.html.contains("aria-label=\"skipped\""));
    // The second scene starts when the FIRST one ends, not after a
    // slot left open by the hidden board.
    assert_eq!(
        windows(&composition.html),
        vec![
            ("0".to_string(), "3".to_string()),
            ("3".to_string(), "3".to_string())
        ]
    );
    assert_eq!(composition.total_ticks, 60);
}

#[test]
fn a_deck_with_no_visible_board_refuses_to_build() {
    let state = deck_state(
        r##"{"version":"1.0.0","children":[
            {"type":"frame","id":"f1","name":"gone","x":0,"y":0,"width":1920,"height":1080,
             "visible":false,"fill":[{"type":"solid","color":"#ff0000"}]}
        ]}"##,
    );

    assert_eq!(deck_composition(&state), Err(ExportError::NothingToExport));
}

#[test]
fn every_emitted_number_is_spelled_the_way_css_reads_numbers() {
    // Sizes that a naive `{}` on an f32 would print as `1.0e-5` or with
    // a long mantissa, and a text length that lands the hold time on a
    // fraction.
    let state = deck_state(
        r##"{"version":"1.0.0","children":[
            {"type":"frame","id":"f1","name":"a","x":0,"y":0,"width":1920,"height":1080,
             "fill":[{"type":"solid","color":"#ffffff"}],"children":[
               {"type":"text","id":"t1","x":0.00001,"y":0,"width":1400,"height":200,
                "content":"一二三四五六七八","fontSize":48,
                "fill":[{"type":"solid","color":"#101828"}]}
             ]},
            {"type":"frame","id":"f2","name":"b","x":3000,"y":0,"width":2000,"height":1000,
             "fill":[{"type":"solid","color":"#ffffff"}]}
        ]}"##,
    );

    let composition = deck_composition(&state).expect("deck builds");
    let html = &composition.html;

    // A locale comma or an exponent in a length or a time is the way
    // this silently breaks: the browser drops the declaration and the
    // scene is mispositioned or never shown.
    for marker in [
        "data-start=\"",
        "data-duration=\"",
        "--hf-start:",
        "animation-delay:",
        "animation-duration:",
    ] {
        for rest in html.split(marker).skip(1) {
            let value = rest
                .split(['"', ';', 's'])
                .next()
                .expect("value is delimited");
            // The one delay that is not a literal is the rule that
            // forwards the inherited custom property; its VALUE is
            // checked through the `--hf-start:` marker above.
            if value.starts_with("var(") {
                continue;
            }
            assert!(
                value.parse::<f64>().is_ok(),
                "{marker}{value:?} is not a number: {html}"
            );
            assert!(
                value.chars().all(|c| c.is_ascii_digit() || c == '.'),
                "{marker}{value:?} must be plain decimal: {html}"
            );
        }
    }
    // The second board is 2000x1000 in a 1920x1080 frame, so the fit
    // maths really ran rather than being short-circuited by two equal
    // sizes: 1920/2000 = 0.96, centred vertically at (1080-960)/2.
    assert!(html.contains("transform:scale(0.96)"), "{html}");
    assert!(html.contains("top:60px"), "{html}");
}

#[test]
fn a_scene_is_visible_only_inside_its_own_window() {
    let state = deck_of(&["one", "two"]);

    let html = deck_composition(&state).expect("deck builds").html;

    // The stacking failure this guards: `fill-mode:both` would hold the
    // last keyframe forever and leave every scene on screen at the end.
    assert!(html.contains("animation-fill-mode:none"), "{html}");
    assert!(!html.contains("animation-fill-mode:both"), "{html}");
    assert!(html.contains(".hf-scene{position:absolute"), "{html}");
    assert!(html.contains("visibility:hidden"), "{html}");
    assert!(
        html.contains("@keyframes hf-hold{from{visibility:visible}"),
        "{html}"
    );
}

/// The three attributes `hyperframes lint` reports as errors when they
/// are missing. Each one is a silent, expensive failure rather than a
/// cosmetic nit, so they are asserted rather than left to a manual lint
/// run: no `clip` renders every slide stacked from frame one, and no
/// `data-no-timeline` adds a 45-second poll to every render.
#[test]
fn the_composition_carries_what_the_renderer_requires() {
    let state = deck_of(&["one", "two"]);

    let html = deck_composition(&state).expect("deck builds").html;

    assert_eq!(html.matches("class=\"clip hf-scene\"").count(), 2, "{html}");
    assert!(html.contains("data-no-timeline"), "{html}");
    assert!(html.contains(&format!("data-fps=\"{FPS}\"")), "{html}");
    // Stable per-scene handles for the renderer's studio tooling.
    assert!(html.contains("id=\"scene-1\""), "{html}");
    assert!(html.contains("id=\"scene-2\""), "{html}");
    // Exactly one element may carry a composition id, or the renderer
    // discovers two entry points for one deck.
    assert_eq!(html.matches("data-composition-id").count(), 1, "{html}");
}

#[test]
fn each_scene_enters_with_a_short_fade_and_no_transition_between_cuts() {
    let state = deck_of(&["one", "two"]);

    let html = deck_composition(&state).expect("deck builds").html;

    assert!(
        html.contains("@keyframes hf-enter{from{opacity:0}to{opacity:1}}"),
        "{html}"
    );
    assert!(html.contains("animation-duration:0.3s"), "{html}");
    // The fade targets the board's CHILDREN. Moving it up onto the
    // slide box takes the board's own background with it and turns
    // every cut into a black frame — a real regression this caught
    // only once the composition was rendered to video.
    assert!(
        html.contains(".hf-slide > .n > *{animation-name:hf-enter"),
        "{html}"
    );
    assert!(
        html.contains(
            ".hf-slide{position:absolute;transform-origin:0 0;overflow:hidden;\
                       background:#fff}"
        ),
        "the slide box must hard-cut, carrying no animation of its own: {html}"
    );
    // Animation longhands do not inherit; the per-scene delay reaches
    // the children through this custom property or not at all.
    assert!(html.contains("--hf-start:3s"), "{html}");
    assert!(
        html.contains("animation-delay:var(--hf-start,0s)"),
        "{html}"
    );
}

#[test]
fn the_composition_reaches_for_nothing_outside_itself() {
    let state = deck_of(&["one"]);

    let html = deck_composition(&state).expect("deck builds").html;

    assert!(!html.contains("http://"), "{html}");
    assert!(!html.contains("https://"), "{html}");
    assert!(!html.contains("<link"), "{html}");
    assert!(!html.contains("<script"), "{html}");
}

#[test]
fn deck_names_are_escaped_into_the_title_the_label_and_the_id() {
    let state = deck_state(
        r##"{"version":"1.0.0","children":[
            {"type":"frame","id":"f1","name":"<script> & \"quotes\"","x":0,"y":0,
             "width":1920,"height":1080,"fill":[{"type":"solid","color":"#ff0000"}]}
        ]}"##,
    );

    let html = deck_composition(&state).expect("deck builds").html;

    assert!(
        html.contains("aria-label=\"&lt;script&gt; &amp; &quot;quotes&quot;\""),
        "{html}"
    );
    assert!(
        html.contains("<title>&lt;script&gt; &amp; &quot;quotes&quot;</title>"),
        "{html}"
    );
    assert!(
        html.contains("data-composition-id=\"script-quotes\""),
        "{html}"
    );
    assert!(!html.contains("<script> &"), "raw board name leaked");
}

#[test]
fn the_export_writes_the_composition_and_its_render_notes_side_by_side() {
    let state = deck_of(&["one", "two"]);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "openpencil-hyperframes-{}-{nanos}",
        std::process::id()
    ));

    let written = export_deck_hyperframes(&state, &dir).expect("deck exports");

    assert_eq!(written.composition_path, dir.join("index.html"));
    assert_eq!(written.render_notes_path, dir.join("RENDER.md"));
    let html = std::fs::read_to_string(&written.composition_path).expect("composition on disk");
    let notes = std::fs::read_to_string(&written.render_notes_path).expect("notes on disk");
    assert_eq!(html, written.composition.html);
    assert!(
        notes.contains("npx hyperframes render . --output deck.mp4"),
        "{notes}"
    );
    assert!(notes.contains("npx hyperframes preview ."), "{notes}");
    assert!(notes.contains("2 scene(s), 6s total"), "{notes}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn hold_times_round_to_whole_ticks_so_starts_can_be_summed_as_integers() {
    // 1.5 + 3/8 = 1.875s → clamped to the 3s floor.
    assert_eq!(hold_ticks(3.0), 30);
    // 1.5 + 41/8 = 6.625s → 6.6s, the nearest tick.
    assert_eq!(hold_ticks(41.0), 66);
    // 1.5 + 43/8 = 6.875s → 6.9s.
    assert_eq!(hold_ticks(43.0), 69);
}

#[test]
fn seconds_never_emits_a_comma_or_an_exponent() {
    assert_eq!(seconds(0), "0");
    assert_eq!(seconds(30), "3");
    assert_eq!(seconds(65), "6.5");
    assert_eq!(seconds(1), "0.1");
    assert_eq!(seconds(1005), "100.5");
}

#[test]
fn a_title_with_no_ascii_falls_back_to_a_usable_id() {
    assert_eq!(slug("Quarterly Review"), "quarterly-review");
    assert_eq!(slug("  spaced  out  "), "spaced-out");
    assert_eq!(slug("季度回顾"), "deck");
    assert_eq!(slug(""), "deck");
}

#[test]
fn latin_and_cjk_are_counted_on_one_scale() {
    assert_eq!(units_in(""), 0.0);
    assert_eq!(units_in("hello world"), 6.0);
    assert_eq!(units_in("你好"), 2.0);
    // Mixed: two CJK characters plus one Latin word.
    assert_eq!(units_in("你好 world"), 5.0);
    // A hyphen ends the run, so a compound counts as the two words a
    // reader actually reads.
    assert_eq!(units_in("well-known"), 6.0);
}
