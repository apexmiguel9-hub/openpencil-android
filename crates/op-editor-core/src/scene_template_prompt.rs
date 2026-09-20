//! The request text the Scene Template Center's generate row sends.
//!
//! The row takes a topic ("Q3 复盘"), not a prompt, so something has to say
//! what kind of document that topic becomes. That wrapping is not cosmetic:
//! the generation pipeline reads the design type straight off the prompt
//! (`op_orchestrator::detect_design_type`), and a bare topic classifies as a
//! landing page — the user would type a talk title into a slides entry point
//! and get a 1200-wide scrolling page back.
//!
//! So the template is a contract with the classifier, and the locale tables
//! that carry it keep an ASCII "PPT" token in every language for the same
//! reason: it is the one trigger word that survives translation. The
//! cross-locale guarantee is asserted in `op-orchestrator`, where the
//! classifier itself lives, so a translation that drops the token fails
//! there rather than shipping as a silently mis-sized deck.
//!
//! The web wrapper is the same contract read from the other side. A landing
//! page is what an unqualified topic *already* classifies as, so its wrapper
//! carries no trigger word to preserve — it only has to avoid tripping one of
//! the other branches, which is why it names the surface ("网页 / landing
//! page") rather than borrowing deck or component vocabulary.

use crate::scene_template_catalog::TemplateScene;
use crate::Locale;

/// The i18n key carrying the wrapper, with a `{{topic}}` placeholder.
pub const SLIDES_PROMPT_KEY: &str = "sceneTemplate.generate.promptTemplate";

/// Its web-page counterpart.
pub const WEB_PROMPT_KEY: &str = "sceneTemplate.generate.webPromptTemplate";

/// Used when the key is missing from a locale table (runtime lookup already
/// falls back through English, so this only fires for a catalogue gap).
const SLIDES_PROMPT_FALLBACK: &str = "为以下主题制作一份演示文稿（PPT）：{{topic}}";
const WEB_PROMPT_FALLBACK: &str = "为以下主题设计一个多区块的网页落地页：{{topic}}";

/// Wrap a raw topic into a request the pipeline reads as a deck.
///
/// Whitespace-only input returns `None`: there is no topic to ask about, and
/// a wrapper around nothing would still classify as slides and generate a
/// deck about the empty string.
pub fn slides_generate_prompt(locale: Locale, topic: &str) -> Option<String> {
    wrap(locale, topic, SLIDES_PROMPT_KEY, SLIDES_PROMPT_FALLBACK)
}

/// Wrap a raw topic into a request the pipeline reads as a landing page.
pub fn web_generate_prompt(locale: Locale, topic: &str) -> Option<String> {
    wrap(locale, topic, WEB_PROMPT_KEY, WEB_PROMPT_FALLBACK)
}

/// The wrapper for whichever scene the card belongs to.
///
/// `None` for a scene the pipeline cannot build, so the one function answers
/// both "is there a prompt" and "should this card offer generation at all"
/// — the alternative is a caller that picks a wrapper by scene and a
/// separate gate that decides whether to show the button, which is two
/// places to keep a new scene in step with.
pub fn scene_generate_prompt(scene: TemplateScene, locale: Locale, topic: &str) -> Option<String> {
    match scene {
        TemplateScene::Slides => slides_generate_prompt(locale, topic),
        TemplateScene::Web => web_generate_prompt(locale, topic),
        _ => None,
    }
}

fn wrap(locale: Locale, topic: &str, key: &'static str, fallback: &'static str) -> Option<String> {
    let topic = topic.trim();
    if topic.is_empty() {
        return None;
    }
    let translated = op_i18n::translate_with(locale, key, &[("topic", topic)]);
    if translated == key {
        return Some(fallback.replace("{{topic}}", topic));
    }
    Some(translated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_locale_wraps_the_topic_verbatim() {
        for locale in Locale::ALL {
            for prompt in [
                slides_generate_prompt(locale, "  Q3 复盘  ").expect("a topic wraps"),
                web_generate_prompt(locale, "  Q3 复盘  ").expect("a topic wraps"),
            ] {
                assert!(
                    prompt.contains("Q3 复盘"),
                    "{locale:?} dropped the topic: {prompt}"
                );
                assert!(
                    !prompt.contains("{{topic}}"),
                    "{locale:?} left the placeholder unsubstituted: {prompt}"
                );
            }
        }
    }

    #[test]
    fn a_blank_topic_is_not_a_request() {
        assert_eq!(slides_generate_prompt(Locale::ZhCn, "   \t "), None);
        assert_eq!(slides_generate_prompt(Locale::EnUs, ""), None);
        assert_eq!(web_generate_prompt(Locale::ZhCn, "   \t "), None);
        assert_eq!(web_generate_prompt(Locale::EnUs, ""), None);
    }

    /// Every scene that says it supports generation must have a wrapper, and
    /// no scene that says it doesn't may have one. The two facts live in
    /// different files, and a new scene is the moment they can drift.
    #[test]
    fn a_wrapper_exists_for_exactly_the_scenes_that_offer_generation() {
        for scene in TemplateScene::ALL {
            assert_eq!(
                scene_generate_prompt(scene, Locale::ZhCn, "Q3 复盘").is_some(),
                scene.supports_generation(),
                "scene `{}` disagrees with its wrapper",
                scene.as_str()
            );
        }
    }
}
