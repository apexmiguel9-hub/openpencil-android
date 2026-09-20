//! Guards for the Slides arm of [`build_fallback_plan`].
//!
//! Measured 2026-08-05 on a user's desktop run: two `planning stream error`
//! attempts on a deck request dropped straight through to the heuristic
//! fallback, which had no Slides branch — the user asked for a PPT and the
//! orchestrator built the generic 1200-wide three-section page. These tests
//! lock the three properties that make the fallback an actual deck.

use super::*;
use crate::plan_normalize::normalize;

fn deck_req(prompt: &str) -> DesignRequest {
    DesignRequest {
        prompt: prompt.into(),
        ..Default::default()
    }
}

#[test]
fn deck_fallback_uses_the_projector_artboard_not_the_desktop_page() {
    let plan = build_fallback_plan(&deck_req("帮我做一个季度汇报 PPT"));
    assert_eq!(plan.root_frame.width, 1920.0);
    assert_eq!(plan.root_frame.height, 1080.0);
    for st in &plan.subtasks {
        assert_eq!(
            (st.region.width, st.region.height),
            (1920.0, 1080.0),
            "every slide region is the full board: {}",
            st.id
        );
    }
}

#[test]
fn deck_fallback_tags_every_slide_with_its_own_screen() {
    let plan = build_fallback_plan(&deck_req("a pitch deck for our seed round"));
    let screens: Vec<&str> = plan
        .subtasks
        .iter()
        .map(|st| st.screen.as_deref().expect("every slide carries a screen"))
        .collect();
    let mut unique = screens.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(
        unique.len(),
        screens.len(),
        "duplicate screen labels collapse slides onto one board: {screens:?}"
    );
}

#[test]
fn deck_fallback_screens_survive_normalize_as_separate_roots() {
    // The `screen` tags are only worth anything if `plan_normalize` fans them
    // out — that is the pass which decides one root vs N.
    let mut plan = build_fallback_plan(&deck_req("做一份 7 页的产品发布演示文稿"));
    let req = deck_req("做一份 7 页的产品发布演示文稿");
    let info = normalize(&mut plan, &req);

    assert!(
        info.is_deck,
        "normalize must classify the fallback as a deck"
    );
    let roots: std::collections::BTreeSet<&str> = plan
        .subtasks
        .iter()
        .map(|st| st.parent_frame_id.as_deref().expect("parent assigned"))
        .collect();
    assert_eq!(
        roots.len(),
        7,
        "each slide needs its own root frame, got {roots:?}"
    );
}

#[test]
fn deck_fallback_honours_an_explicitly_requested_slide_count() {
    for (prompt, expected) in [
        ("帮我做一个 12 页的产品培训课件 PPT", 12),
        ("做一个3页的极简 keynote", 3),
        ("a 10-slide pitch deck", 10),
        ("build a deck, 9 slides, dark theme", 9),
    ] {
        let plan = build_fallback_plan(&deck_req(prompt));
        assert_eq!(
            plan.subtasks.len(),
            expected,
            "{prompt:?} asked for {expected} slides"
        );
    }
}

#[test]
fn deck_fallback_without_a_count_scales_with_the_prompt_instead_of_fixing_six() {
    let short = build_fallback_plan(&deck_req("做个 PPT"));
    let long = build_fallback_plan(&deck_req(&format!(
        "帮我做一套关于分布式系统的培训幻灯片，覆盖一致性、复制、分区容错、\
         共识算法、故障恢复和可观测性六个主题，每个主题都要有示意图和要点。{}",
        "补充说明。".repeat(20)
    )));
    assert!(
        long.subtasks.len() > short.subtasks.len(),
        "a richly described deck must plan more slides than a bare one: \
         {} vs {}",
        long.subtasks.len(),
        short.subtasks.len()
    );
}

#[test]
fn slide_count_parser_ignores_numbers_that_are_not_slide_counts() {
    for prompt in [
        "做一个 1920x1080 的 PPT",
        "2026 年度汇报 PPT",
        "a deck about our $500 pricing tier",
        "16:9 keynote",
    ] {
        assert_eq!(
            explicit_slide_count(prompt),
            None,
            "{prompt:?} carries no slide count"
        );
    }
    // …and still reads the real ones out of a noisy prompt.
    assert_eq!(
        explicit_slide_count("2026 年度汇报，1920x1080，一共 8 页"),
        Some(8)
    );
}

#[test]
fn slide_titles_stay_distinct_and_bracketed_at_every_count() {
    for count in 2..=30usize {
        let titles = fallback_slide_titles(count);
        assert_eq!(titles.len(), count, "count {count}");
        assert_eq!(titles.first().map(String::as_str), Some("Cover"));
        assert_eq!(titles.last().map(String::as_str), Some("Closing"));
        let mut unique = titles.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), count, "duplicate titles at count {count}");
    }
}

#[test]
fn non_deck_fallback_is_unchanged() {
    // Regression lock: the Slides branch must not intercept anything else.
    let plan = build_fallback_plan(&deck_req("a marketing landing page for a fintech product"));
    assert_eq!(plan.root_frame.id, "root");
    assert_eq!(plan.root_frame.width, 1200.0);
    assert!(plan.subtasks.iter().all(|st| st.screen.is_none()));
}
