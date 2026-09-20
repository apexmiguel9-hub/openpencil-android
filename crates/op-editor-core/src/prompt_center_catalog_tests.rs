use std::collections::HashSet;

use op_i18n::Locale;

use crate::prompt_center_catalog::{
    parse_prompt_catalogue, prompt_catalogue, PromptCategory, PromptDefinition,
};

fn has_tag(prompt: &PromptDefinition, tag: &str) -> bool {
    prompt.tags.iter().any(|candidate| candidate == tag)
}

#[test]
fn embedded_catalogue_has_all_fifty_nine_seed_prompts() {
    let prompts = prompt_catalogue();
    assert_eq!(prompts.len(), 59);

    let expected = [
        "gallery-wander",
        "gallery-forage",
        "gallery-still",
        "gallery-hearth",
        "gallery-meteo",
        "gallery-marginalia",
        "gallery-lingua",
        "gallery-daybreak",
        "gallery-verdant",
        "gallery-companion",
        "gallery-relic",
        "gallery-nocturne",
        "gallery-marquee",
        "gallery-ritual",
        "gallery-ember",
        "gallery-volt",
        "gallery-aloft",
        "gallery-gallery",
        "gallery-nightcap",
        "gallery-bloom",
        "freeform-wander",
        "freeform-forage",
        "freeform-still",
        "freeform-hearth",
        "freeform-meteo",
        "freeform-marginalia",
        "freeform-lingua",
        "freeform-daybreak",
        "freeform-verdant",
        "freeform-companion",
        "freeform-relic",
        "freeform-nocturne",
        "freeform-marquee",
        "freeform-ritual",
        "freeform-ember",
        "freeform-volt",
        "freeform-aloft",
        "freeform-gallery",
        "freeform-nightcap",
        "freeform-bloom",
        "extreme-weather",
        "extreme-now-playing",
        "extreme-daily-app",
        "extreme-calendar",
        "extreme-calm",
        "starter-travel-app",
        "starter-dashboard",
        "starter-coffee-shop",
        "starter-barbershop",
        "web-orbit",
        "web-atelier",
        "web-kilnform",
        "web-reefwright",
        "dashboard-pulse",
        "dashboard-sentinel",
        "component-data-grid",
        "component-form-lab",
        "modify-polish-current",
        "modify-complete-states",
    ];
    let actual: Vec<_> = prompts.iter().map(|prompt| prompt.id.as_str()).collect();
    assert_eq!(actual, expected);
    assert_eq!(
        actual.iter().copied().collect::<HashSet<_>>().len(),
        actual.len()
    );
}

#[test]
fn seed_groups_and_categories_match_the_source_contract() {
    let prompts = prompt_catalogue();
    assert_eq!(
        prompts
            .iter()
            .filter(|prompt| prompt.category == PromptCategory::MobileApp)
            .count(),
        45
    );
    assert_eq!(
        prompts
            .iter()
            .filter(|prompt| prompt.category == PromptCategory::Starter)
            .count(),
        4
    );
    for (category, count) in [
        // Two seed prompts each, plus the two web pages that ship as scene
        // templates as well — those carry their own designs, so the group
        // they belong to is the one place the count is not uniform.
        (PromptCategory::WebPage, 4),
        (PromptCategory::Dashboard, 2),
        (PromptCategory::Component, 2),
        (PromptCategory::Modify, 2),
    ] {
        assert_eq!(
            prompts
                .iter()
                .filter(|prompt| prompt.category == category)
                .count(),
            count,
            "{category:?}"
        );
    }
    assert_eq!(
        prompts
            .iter()
            .filter(|prompt| has_tag(prompt, "constrained"))
            .count(),
        20
    );
    assert_eq!(
        prompts
            .iter()
            .filter(|prompt| has_tag(prompt, "gallery") && has_tag(prompt, "freeform"))
            .count(),
        20
    );
    assert_eq!(
        prompts
            .iter()
            .filter(|prompt| has_tag(prompt, "extreme"))
            .count(),
        5
    );
    assert_eq!(
        prompts
            .iter()
            .filter(|prompt| has_tag(prompt, "quick-action"))
            .count(),
        4
    );
}

#[test]
fn paired_galleries_share_twenty_title_keys() {
    let prompts = prompt_catalogue();
    for product in [
        "wander",
        "forage",
        "still",
        "hearth",
        "meteo",
        "marginalia",
        "lingua",
        "daybreak",
        "verdant",
        "companion",
        "relic",
        "nocturne",
        "marquee",
        "ritual",
        "ember",
        "volt",
        "aloft",
        "gallery",
        "nightcap",
        "bloom",
    ] {
        let constrained = prompts
            .iter()
            .find(|prompt| prompt.id == format!("gallery-{product}"))
            .unwrap();
        let freeform = prompts
            .iter()
            .find(|prompt| prompt.id == format!("freeform-{product}"))
            .unwrap();
        assert_eq!(constrained.title_key, freeform.title_key);
        assert_eq!(constrained.title_fallback, freeform.title_fallback);
        assert_ne!(constrained.body_zh, freeform.body_zh);
        assert_ne!(constrained.body_en, freeform.body_en);
    }

    let unique_title_keys: HashSet<_> = prompts.iter().map(|prompt| &prompt.title_key).collect();
    assert_eq!(unique_title_keys.len(), 39);
}

#[test]
fn screen_metadata_matches_the_four_multiscreen_pairs() {
    let prompts = prompt_catalogue();
    let multiscreen: Vec<_> = prompts
        .iter()
        .filter(|prompt| has_tag(prompt, "multi-screen"))
        .collect();
    assert_eq!(multiscreen.len(), 8);
    assert!(multiscreen
        .iter()
        .all(|prompt| prompt.screens.is_some_and(|count| count >= 2)));
    assert_eq!(
        multiscreen
            .iter()
            .filter(|prompt| prompt.screens == Some(2))
            .count(),
        2
    );
    assert_eq!(
        multiscreen
            .iter()
            .filter(|prompt| prompt.screens == Some(3))
            .count(),
        6
    );
    assert!(prompts
        .iter()
        .filter(|prompt| prompt.screens.is_some_and(|count| count >= 2))
        .all(|prompt| has_tag(prompt, "multi-screen")));

    for id in [
        "web-orbit",
        "web-atelier",
        "dashboard-pulse",
        "dashboard-sentinel",
        "component-data-grid",
        "component-form-lab",
    ] {
        assert_eq!(
            prompts
                .iter()
                .find(|prompt| prompt.id == id)
                .and_then(|prompt| prompt.screens),
            Some(1),
            "{id}"
        );
    }
    for id in ["modify-polish-current", "modify-complete-states"] {
        assert_eq!(
            prompts
                .iter()
                .find(|prompt| prompt.id == id)
                .and_then(|prompt| prompt.screens),
            None,
            "{id}"
        );
    }
}

#[test]
fn every_body_is_bilingual_content_without_markdown_labels() {
    for prompt in prompt_catalogue() {
        assert!(!prompt.body_zh.trim().is_empty(), "{}", prompt.id);
        assert!(!prompt.body_en.trim().is_empty(), "{}", prompt.id);
        assert!(!prompt.body_zh.contains("**中**:"), "{}", prompt.id);
        assert!(!prompt.body_en.contains("**EN**:"), "{}", prompt.id);
    }
}

#[test]
fn cjk_locales_select_chinese_and_all_other_locales_select_english() {
    let prompt = prompt_catalogue()
        .iter()
        .find(|prompt| prompt.id == "gallery-wander")
        .unwrap();
    for locale in Locale::ALL {
        let is_cjk = matches!(
            locale,
            Locale::ZhCn | Locale::ZhTw | Locale::Ja | Locale::Ko
        );
        assert_eq!(
            prompt.body_for_locale(locale),
            if is_cjk {
                prompt.body_zh.as_str()
            } else {
                prompt.body_en.as_str()
            },
            "{locale:?}"
        );
    }
}

#[test]
fn wander_is_searchable_in_both_catalogue_languages() {
    let prompt: &'static PromptDefinition = prompt_catalogue()
        .iter()
        .find(|prompt| prompt.id == "gallery-wander")
        .unwrap();
    assert!(prompt.matches_query(Locale::ZhCn, "旅行"));
    assert!(prompt.matches_query(Locale::ZhCn, "TRAVEL"));
    assert!(prompt.matches_query(Locale::EnUs, "旅行"));
    assert!(prompt.matches_query(Locale::EnUs, "travel"));
    assert!(!prompt.matches_query(Locale::EnUs, "barbershop crm"));
}

#[test]
fn starter_entries_are_exactly_the_four_live_quick_actions() {
    let cases = [
        (
            "starter-travel-app",
            "ai.quickAction.travelApp",
            "ai.quickAction.travelAppPrompt",
        ),
        (
            "starter-dashboard",
            "ai.quickAction.dashboard",
            "ai.quickAction.dashboardPrompt",
        ),
        (
            "starter-coffee-shop",
            "ai.quickAction.coffeeShop",
            "ai.quickAction.coffeeShopPrompt",
        ),
        (
            "starter-barbershop",
            "ai.quickAction.barbershop",
            "ai.quickAction.barbershopPrompt",
        ),
    ];
    for (id, title_key, body_key) in cases {
        let prompt = prompt_catalogue()
            .iter()
            .find(|prompt| prompt.id == id)
            .unwrap();
        assert_eq!(prompt.title_key, title_key);
        assert_eq!(prompt.body_en, op_i18n::translate(Locale::EnUs, body_key));
        assert_eq!(prompt.body_zh, op_i18n::translate(Locale::ZhCn, body_key));
    }
}

#[test]
fn categories_round_trip_through_persisted_kebab_case() {
    for category in PromptCategory::ALL {
        let json = serde_json::to_string(&category).unwrap();
        assert_eq!(json, format!("\"{}\"", category.as_str()));
        assert_eq!(
            serde_json::from_str::<PromptCategory>(&json).unwrap(),
            category
        );
        assert_eq!(category.as_str().parse(), Ok(category));
    }
}

fn valid_entry(id: &str, extra: &str) -> String {
    format!(
        r#"
[[prompt]]
id = "{id}"
category = "mobile-app"
tags = ["test"]
title_key = "promptCenter.item.test.title"
title_fallback = "Test"
body_zh = '''测试'''
body_en = '''Test'''
{extra}
"#
    )
}

#[test]
fn strict_parser_rejects_unknown_duplicate_and_missing_fields() {
    let unknown = valid_entry("unknown-field", "surprise = \"no\"");
    assert!(parse_prompt_catalogue(&unknown)
        .unwrap_err()
        .message
        .contains("unknown field"));

    let duplicate = format!(
        "{}{}",
        valid_entry("duplicate", ""),
        valid_entry("duplicate", "")
    );
    assert!(parse_prompt_catalogue(&duplicate)
        .unwrap_err()
        .message
        .contains("duplicate prompt id"));

    let missing = r#"
[[prompt]]
id = "missing-body"
category = "mobile-app"
tags = ["test"]
title_key = "promptCenter.item.test.title"
title_fallback = "Test"
body_zh = '''测试'''
"#;
    assert!(parse_prompt_catalogue(missing)
        .unwrap_err()
        .message
        .contains("missing `body_en`"));
}

#[test]
fn strict_parser_rejects_unknown_categories_and_invalid_screens() {
    let unknown_category = valid_entry("unknown-category", "")
        .replace("category = \"mobile-app\"", "category = \"not-a-category\"");
    assert!(parse_prompt_catalogue(&unknown_category)
        .unwrap_err()
        .message
        .contains("unknown category"));

    let zero_screens = valid_entry("zero-screens", "screens = 0");
    assert!(parse_prompt_catalogue(&zero_screens)
        .unwrap_err()
        .message
        .contains("greater than zero"));

    let malformed_array = valid_entry("bad-tags", "").replace("tags = [\"test\"]", "tags = test");
    assert!(parse_prompt_catalogue(&malformed_array)
        .unwrap_err()
        .message
        .contains("string array"));
}
