//! `@font-face` visibility.
//!
//! The importer never downloads or registers a web font: text keeps the CSS
//! `font-family` string it was given and the editor resolves it against the
//! fonts actually installed. That is invisible in the imported document — a
//! heading silently renders in a fallback face — so every declared family that
//! some imported text really asks for is reported here.

use crate::import_warning::ImportWarning;
use jian_ops_schema::node::text::TextContent;
use jian_ops_schema::node::PenNode;
use std::collections::BTreeSet;

use crate::css::declarations::Declaration;

/// One parsed `@font-face` block.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct WebFont {
    pub family: String,
    /// `src` URLs in declaration order. Kept for the warning's provenance and
    /// for a future download path; nothing consumes them today.
    pub sources: Vec<String>,
}

/// Extract the family and `src` URLs from an `@font-face` block's declarations.
pub(crate) fn from_declarations(declarations: &[Declaration]) -> Option<WebFont> {
    let family = declarations
        .iter()
        .rev()
        .find(|declaration| declaration.name == "font-family")
        .map(|declaration| unquote(&declaration.value))?;
    if family.is_empty() {
        return None;
    }
    let sources = declarations
        .iter()
        .filter(|declaration| declaration.name == "src")
        .flat_map(|declaration| source_urls(&declaration.value))
        .collect();
    Some(WebFont { family, sources })
}

/// `url("a.woff2") format("woff2"), local(X)` → `["a.woff2"]`.
///
/// The scan is quote-aware: a `)` inside a quoted URL — `url('a(1).woff2')` —
/// belongs to the string, not to the function, and must not end the token.
fn source_urls(value: &str) -> Vec<String> {
    let mut urls = Vec::new();
    let lower = value.to_ascii_lowercase();
    let mut at = 0usize;
    while let Some(found) = lower[at..].find("url(") {
        let start = at + found + "url(".len();
        let body = &value[start..];
        let leading = body.len() - body.trim_start().len();
        let quote = body
            .trim_start()
            .chars()
            .next()
            .filter(|character| *character == '"' || *character == '\'');
        // Past the closing quote for a quoted URL, from the start otherwise.
        let scan_from = match quote {
            Some(quote) => match body[leading + quote.len_utf8()..].find(quote) {
                Some(close) => leading + quote.len_utf8() + close + quote.len_utf8(),
                // Unterminated string: nothing sane is left to read.
                None => break,
            },
            None => 0,
        };
        let Some(end) = body[scan_from..].find(')').map(|end| scan_from + end) else {
            break;
        };
        let url = unquote(&body[..end]);
        if !url.is_empty() {
            urls.push(url);
        }
        at = start + end + 1;
    }
    urls
}

/// Report every declared family that imported text actually references.
///
/// Families nothing uses stay silent: a icon-font `@font-face` in a shared
/// stylesheet is not a degradation of this document. Neither is a `local()`-only
/// face: it asks for a font already installed on the machine, so there is
/// nothing to download and the editor resolves it exactly as the browser would.
///
/// Fallback entries later in a `font-family` list ARE reported. That is
/// deliberate: if the first choice is missing, text really does fall through to
/// a face this importer did not fetch either, and naming it is the point of the
/// warning.
pub(crate) fn warn_undownloaded(
    fonts: &[WebFont],
    nodes: &[PenNode],
    warnings: &mut Vec<ImportWarning>,
) {
    if fonts.is_empty() {
        return;
    }
    let mut used = BTreeSet::new();
    collect_used_families(nodes, &mut used);
    if used.is_empty() {
        return;
    }
    let mut reported = BTreeSet::new();
    for font in fonts {
        if font.sources.is_empty() {
            continue;
        }
        let key = font.family.to_ascii_lowercase();
        if !used.contains(&key) || !reported.insert(key) {
            continue;
        }
        warnings.push(ImportWarning::WebFontNotDownloaded {
            family: font.family.clone(),
        });
    }
}

/// Every family name imported text asks for, lower-cased and unquoted.
///
/// Only `PenNode::Text` is walked because only `TextNode` carries a
/// `font_family` in the schema — `TextInput` / `TextArea` / `Select` have no
/// typography fields at all, so a form control's CSS family is dropped by the
/// mapper long before it could be reported here. Extend this walk when (and
/// only when) those variants grow the field.
fn collect_used_families(nodes: &[PenNode], used: &mut BTreeSet<String>) {
    for node in nodes {
        if let PenNode::Text(text) = node {
            push_family_list(text.font_family.as_deref(), used);
            if let TextContent::Styled(segments) = &text.content {
                for segment in segments {
                    push_family_list(segment.font_family.as_deref(), used);
                }
            }
        }
        collect_used_families(child_nodes(node), used);
    }
}

fn push_family_list(value: Option<&str>, used: &mut BTreeSet<String>) {
    let Some(value) = value else {
        return;
    };
    for family in value.split(',') {
        let family = unquote(family).to_ascii_lowercase();
        if !family.is_empty() {
            used.insert(family);
        }
    }
}

fn unquote(value: &str) -> String {
    value
        .trim()
        .trim_matches(|character| character == '"' || character == '\'')
        .trim()
        .to_string()
}

fn child_nodes(node: &PenNode) -> &[PenNode] {
    match node {
        PenNode::Frame(frame) => frame.children.as_deref().unwrap_or_default(),
        PenNode::Group(group) => group.children.as_deref().unwrap_or_default(),
        PenNode::Rectangle(rectangle) => rectangle.children.as_deref().unwrap_or_default(),
        PenNode::Ref(reference) => reference.children.as_deref().unwrap_or_default(),
        PenNode::Tabs(tabs) => tabs.children.as_deref().unwrap_or_default(),
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::css::declarations::parse_declarations;

    #[test]
    fn parses_family_and_source_urls() {
        let declarations = parse_declarations(
            "font-family:'Inter Var';font-style:normal;\
             src:url(/fonts/inter.woff2) format(\"woff2\"),local(\"Inter\"),url('b.woff')",
        );
        let font = from_declarations(&declarations).expect("web font");
        assert_eq!(font.family, "Inter Var");
        assert_eq!(font.sources, ["/fonts/inter.woff2", "b.woff"]);
    }

    #[test]
    fn a_block_without_a_family_is_not_a_web_font() {
        assert_eq!(
            from_declarations(&parse_declarations("src:url(a.woff)")),
            None
        );
    }

    #[test]
    fn a_parenthesised_url_survives_the_quote_aware_scan() {
        let declarations = parse_declarations(
            "font-family:X;src:url('a(1).woff2') format('woff2'),url(\"b(2).woff\"),url(c.woff)",
        );
        let font = from_declarations(&declarations).expect("web font");
        assert_eq!(font.sources, ["a(1).woff2", "b(2).woff", "c.woff"]);
    }

    /// A `local()`-only face asks for an installed font. Nothing is downloaded
    /// because nothing needs to be, so it is not a degradation.
    #[test]
    fn a_local_only_face_is_never_reported_as_undownloaded() {
        let fonts = vec![WebFont {
            family: "Inter".into(),
            sources: Vec::new(),
        }];
        let result = crate::import_html(
            "<style>@font-face{font-family:Inter;src:local(\"Inter\")}\
             p{font-family:Inter}</style><p>hi</p>",
            &crate::HtmlImportOptions::default(),
        );
        let mut diagnostics = Vec::new();
        warn_undownloaded(&fonts, &result.nodes, &mut diagnostics);
        let warnings = crate::render_warnings(&diagnostics);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(
            !result
                .warnings
                .iter()
                .any(|warning| warning.contains("@font-face")),
            "{:?}",
            result.warnings
        );
    }

    #[test]
    fn only_referenced_families_are_reported() {
        let fonts = vec![
            WebFont {
                family: "Inter".into(),
                sources: vec!["inter.woff2".into()],
            },
            WebFont {
                family: "Unused Icons".into(),
                sources: Vec::new(),
            },
        ];
        let result = crate::import_html(
            "<style>@font-face{font-family:Inter;src:url(inter.woff2)}\
             p{font-family:Inter,sans-serif}</style><p>hi</p>",
            &crate::HtmlImportOptions::default(),
        );
        let mut diagnostics = Vec::new();
        warn_undownloaded(&fonts, &result.nodes, &mut diagnostics);
        let warnings = crate::render_warnings(&diagnostics);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("'Inter'"), "{warnings:?}");
    }
}
