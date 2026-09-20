//! Per-section authoring guidelines for the layered `design_skeleton`
//! workflow — the long prose/role table carved off `batch_layered.rs`
//! to keep both files under the 800-line cap.

use serde_json::Value;

pub(super) fn generate_section_guidelines(
    name: &str,
    role: Option<&str>,
    content_width: i32,
    canvas_width: i32,
    style_guide: Option<&Value>,
) -> (String, Vec<&'static str>) {
    let name = name.to_lowercase();
    let is_mobile = canvas_width <= 500;
    let accent_color = style_guide
        .and_then(|guide| guide.get("palette"))
        .and_then(|palette| palette.get("accent"))
        .and_then(Value::as_str)
        .unwrap_or("#2563EB");

    if name.contains("nav") || role == Some("navbar") {
        return (
            format!(
                "Horizontal layout with 3 child groups: logo frame, nav-links frame, CTA button. \
Use justifyContent=\"space_between\", alignItems=\"center\". Logo as text (fontSize 18-20, \
fontWeight 700) or frame with icon. Nav links: horizontal frame with gap={}, each link as text \
node. CTA: button with accent fill [{{\"type\":\"solid\",\"color\":\"{}\"}}], white text. \
Content width: {}px.",
                if is_mobile { 16 } else { 32 },
                accent_color,
                content_width
            ),
            vec!["navbar", "nav-links", "nav-link", "button", "label", "icon"],
        );
    }

    if name.contains("hero") || role == Some("hero") {
        let layout_note = if is_mobile {
            "Stack vertically with gap 16-24. Center-align content."
        } else {
            "For desktop with phone mockup: two-column horizontal layout (left text, right phone). \
Without mockup: center-aligned vertical stack."
        };
        return (
            format!(
                "Large headline ({}px, fontWeight 700), subtitle (16-18px, secondary color), \
CTA button(s). {} Use gap=24 between elements. Headline text: textGrowth=\"fixed-width\" if >15 \
chars. Content width: {}px.",
                if is_mobile { "28-36" } else { "40-56" },
                layout_note,
                content_width
            ),
            vec![
                "hero",
                "heading",
                "subheading",
                "body-text",
                "button",
                "phone-mockup",
                "row",
            ],
        );
    }

    if name.contains("search") || name.contains("搜索") {
        return (
            format!(
                "Create a single compact search row, not a hero panel: one search-bar/form-input \
with search icon and placeholder plus an optional filter icon-button. Mobile height 48-56px, \
gap=10-12, no oversized tinted shell, no nested rounded background inside another large rounded \
background. Mobile top rhythm: place this primary module within 20-32px of the header/title group. \
Content width: {}px.",
                content_width
            ),
            vec![
                "row",
                "search-bar",
                "form-input",
                "input",
                "icon-button",
                "icon",
                "caption",
            ],
        );
    }

    if name.contains("feature") || name.contains("功能") {
        return (
            format!(
                "Section title (heading, 28-36px) + subtitle, then {} feature cards in a {} \
layout. Each card: frame with role=\"feature-card\", containing icon (path 20-24px), title \
(text 18-20px), description (text 14-16px). Cards in horizontal row: ALL must use \
width=\"fill_container\" + height=\"fill_container\". Use gap={} between cards. clipContent=true \
+ cornerRadius=12 on cards. Content width: {}px.",
                if is_mobile { "2-3" } else { "3-4" },
                if is_mobile { "vertical" } else { "horizontal" },
                if is_mobile { 16 } else { 24 },
                content_width
            ),
            vec![
                "section",
                "heading",
                "subheading",
                "feature-card",
                "feature-grid",
                "icon",
                "body-text",
            ],
        );
    }

    if name.contains("footer") || role == Some("footer") {
        return (
            format!(
                "{}: logo+tagline, navigation links grouped by category, social icons. Use muted \
text colors for secondary content. Add a divider (height=1, fill border color) above footer if \
needed. Bottom row: copyright text, small links. Content width: {}px.",
                if is_mobile {
                    "Vertical stack"
                } else {
                    "Horizontal layout with 3-4 column groups"
                },
                content_width
            ),
            vec![
                "footer",
                "row",
                "column",
                "nav-links",
                "label",
                "caption",
                "divider",
                "icon",
            ],
        );
    }

    if name.contains("cta") || name.contains("call to action") || role == Some("cta-section") {
        return (
            format!(
                "Centered content: bold headline (28-36px), short subtitle, prominent CTA button. \
Use accent background or gradient for visual distinction. Button: large (padding [16, 40]), \
contrasting color, cornerRadius 8-12. Content width: {}px.",
                content_width
            ),
            vec![
                "cta-section",
                "heading",
                "subheading",
                "button",
                "centered-content",
            ],
        );
    }

    if name.contains("testimonial") || name.contains("review") || name.contains("评价") {
        return (
            format!(
                "Section title + {} testimonial cards in {} layout. Each card: quote text \
(italic or normal, 14-16px), author name, author title/company. Optional: avatar (circle, 48px), \
star rating (5 star icons). Cards in horizontal: width=\"fill_container\" + \
height=\"fill_container\". Content width: {}px.",
                if is_mobile { "1-2" } else { "2-3" },
                if is_mobile { "vertical" } else { "horizontal" },
                content_width
            ),
            vec![
                "section",
                "card",
                "heading",
                "body-text",
                "caption",
                "avatar",
                "row",
            ],
        );
    }

    if name.contains("pricing") || name.contains("价格") || name.contains("plan") {
        return (
            format!(
                "Section title + {} pricing cards in {} layout. Each card: plan name, price \
(large text 36-48px), feature list (each item with check icon + text), CTA button. Highlight the \
recommended plan with accent border or fill. Cards in horizontal: width=\"fill_container\" + \
height=\"fill_container\". Content width: {}px.",
                if is_mobile { "1-2" } else { "2-3" },
                if is_mobile { "vertical" } else { "horizontal" },
                content_width
            ),
            vec![
                "section",
                "pricing-card",
                "heading",
                "label",
                "body-text",
                "button",
                "icon",
                "divider",
            ],
        );
    }

    if name.contains("stat") || name.contains("数据") || name.contains("metric") {
        return (
            format!(
                "{} layout. Each stat: large number (fontSize 36-48px, fontWeight 700), label \
text (14px, secondary color). Optional: icon or trend indicator. Cards: width=\"fill_container\" \
+ height=\"fill_container\". Content width: {}px.",
                if is_mobile {
                    "2x2 grid"
                } else {
                    "3-4 stat cards in horizontal"
                },
                content_width
            ),
            vec![
                "stats-section",
                "stat-card",
                "heading",
                "caption",
                "icon",
                "row",
            ],
        );
    }

    if name.contains("form")
        || name.contains("login")
        || name.contains("signup")
        || name.contains("register")
        || name.contains("表单")
        || name.contains("登录")
        || name.contains("注册")
    {
        return (
            format!(
                "Vertical layout with gap=16-20. ALL inputs MUST use width=\"fill_container\". \
Input fields: frame with role=\"form-input\", height=48, light bg, subtle border. Include \
placeholder text nodes inside inputs. Submit button: width=\"fill_container\", height=48, accent \
fill, white text. Keep form elements (inputs + submit button) together - do NOT split. Optional: \
social login buttons (horizontal frame, each width=\"fit_content\"). Content width: {}px.",
                content_width
            ),
            vec![
                "form-group",
                "form-input",
                "input",
                "button",
                "label",
                "caption",
                "divider",
                "icon",
            ],
        );
    }

    if name.contains("header") || name.contains("顶部") {
        return (
            format!(
                "Horizontal layout with justifyContent=\"space_between\", alignItems=\"center\". \
Left: back icon or menu icon. Center: title text. Right: action icon(s). Height: {}px. Content \
width: {}px.",
                if is_mobile { 56 } else { 64 },
                content_width
            ),
            vec!["row", "heading", "icon-button", "icon", "label"],
        );
    }

    if name.contains("sidebar") || name.contains("侧边栏") {
        return (
            format!(
                "Vertical layout with gap=4-8. Fixed width (240-280px). Items: horizontal frame \
with icon (20px) + text label, padding=[8,16], gap=12, alignItems=\"center\". Active item: accent \
fill or left border indicator. Group labels: uppercase caption text, letterSpacing=1-2. Content \
width: {}px.",
                content_width
            ),
            vec![
                "column", "nav-link", "icon", "label", "caption", "divider", "heading",
            ],
        );
    }

    (
        format!(
            "Vertical layout section. Content should be wrapped in a centered content frame if \
desktop. Use heading (28-36px) for section title, body-text (16px) for descriptions. All text >15 \
chars: textGrowth=\"fixed-width\" + width=\"fill_container\". Content width: {}px.",
            content_width
        ),
        vec![
            "section",
            "heading",
            "subheading",
            "body-text",
            "button",
            "row",
            "card",
        ],
    )
}
