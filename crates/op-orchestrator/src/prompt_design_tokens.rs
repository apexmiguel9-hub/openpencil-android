//! Explicit design-token extraction — literal radii / spacing / sizes the
//! user spelled out in the prompt, plus the instruction line they produce.

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(super) struct ExplicitDesignTokens {
    pub(super) radius: Option<f64>,
    pub(super) spacing: Option<f64>,
}

pub(super) fn find_keyword_positions(chars: &[char], keyword: &str) -> Vec<(usize, usize)> {
    let needle: Vec<char> = keyword.chars().collect();
    if needle.is_empty() || needle.len() > chars.len() {
        return Vec::new();
    }
    chars
        .windows(needle.len())
        .enumerate()
        .filter_map(|(idx, window)| {
            if window == needle.as_slice() {
                Some((idx, idx + needle.len()))
            } else {
                None
            }
        })
        .collect()
}

pub(super) fn keyword_distance(
    start: usize,
    end: usize,
    ranges: &[(usize, usize)],
) -> Option<usize> {
    ranges
        .iter()
        .map(|&(k_start, k_end)| {
            if end <= k_start {
                k_start - end
            } else {
                start.saturating_sub(k_end)
            }
        })
        .min()
}

pub(super) fn format_design_number(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{}", value as i64)
    } else {
        format!("{value:.1}")
    }
}

pub(super) fn extract_explicit_design_tokens(prompt: &str) -> ExplicitDesignTokens {
    let lower = prompt.to_lowercase();
    let chars: Vec<char> = lower.chars().collect();
    let radius_ranges: Vec<(usize, usize)> = [
        "圆角",
        "corner radius",
        "cornerradius",
        "border radius",
        "border-radius",
        "radius",
        "corner",
    ]
    .into_iter()
    .flat_map(|keyword| find_keyword_positions(&chars, keyword))
    .collect();
    let spacing_ranges: Vec<(usize, usize)> = ["间距", "spacing", "gap"]
        .into_iter()
        .flat_map(|keyword| find_keyword_positions(&chars, keyword))
        .collect();

    let mut out = ExplicitDesignTokens::default();
    let mut i = 0;
    while i < chars.len() {
        if !chars[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let start = i;
        i += 1;
        while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
            i += 1;
        }
        let end = i;
        let raw_value: String = chars[start..end].iter().collect();
        let Ok(value) = raw_value.parse::<f64>() else {
            continue;
        };
        if !(0.0..=128.0).contains(&value) {
            continue;
        }

        let radius_distance = keyword_distance(start, end, &radius_ranges);
        let spacing_distance = keyword_distance(start, end, &spacing_ranges);
        match (radius_distance, spacing_distance) {
            (Some(r), Some(s)) if r <= 12 || s <= 12 => {
                if r <= s {
                    out.radius.get_or_insert(value);
                } else {
                    out.spacing.get_or_insert(value);
                }
            }
            (Some(r), _) if r <= 12 => {
                out.radius.get_or_insert(value);
            }
            (_, Some(s)) if s <= 12 => {
                out.spacing.get_or_insert(value);
            }
            _ => {}
        }
    }
    out
}

pub(super) fn explicit_design_token_instruction(tokens: ExplicitDesignTokens) -> Option<String> {
    if tokens.radius.is_none() && tokens.spacing.is_none() {
        return None;
    }
    let mut parts = Vec::new();
    if let Some(radius) = tokens.radius {
        parts.push(format!(
            "cornerRadius must be {}px for ordinary rounded components",
            format_design_number(radius)
        ));
    }
    if let Some(spacing) = tokens.spacing {
        parts.push(format!(
            "layout gap/spacing must be {}px",
            format_design_number(spacing)
        ));
    }
    Some(format!(
        "EXPLICIT USER DESIGN TOKENS: {}. These values override style-guide radius/spacing ranges, role defaults, and generic mobile guidance. Do not use larger rounded-card/search ranges or mixed 16/20/24px internal gaps unless geometry strictly requires it.",
        parts.join("; ")
    ))
}
