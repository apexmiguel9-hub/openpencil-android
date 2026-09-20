//! The built-in semantic-palette fallback table.
//!
//! Generated from `pen-core/src/variables/default-palette-fallback.ts`;
//! kept in its own file so the resolver spine stays readable.

/// Built-in fallback for the 56 semantic-palette tokens — generated
/// from `pen-core/src/variables/default-palette-fallback.ts` (which
/// derives it from `semantic-palette.ts`). Light/dark colour tokens
/// key off the `Mode` theme axis; chart colours and numeric
/// typography / spacing / radius tokens are mode-independent.
pub(crate) enum FallbackValue {
    LightDark {
        light: &'static str,
        dark: &'static str,
    },
    Single(&'static str),
    Num(f64),
}

pub(crate) const DEFAULT_PALETTE_FALLBACK: &[(&str, FallbackValue)] = &[
    (
        "color-accent",
        FallbackValue::LightDark {
            light: "#2563EB",
            dark: "#60A5FA",
        },
    ),
    (
        "color-bg-deep",
        FallbackValue::LightDark {
            light: "#F8FAFC",
            dark: "#0F172A",
        },
    ),
    (
        "color-border",
        FallbackValue::LightDark {
            light: "#E2E8F0",
            dark: "#334155",
        },
    ),
    (
        "color-border-strong",
        FallbackValue::LightDark {
            light: "#CBD5E1",
            dark: "#475569",
        },
    ),
    ("color-chart-1", FallbackValue::Single("#3B82F6")),
    ("color-chart-2", FallbackValue::Single("#8B5CF6")),
    ("color-chart-3", FallbackValue::Single("#EC4899")),
    ("color-chart-4", FallbackValue::Single("#14B8A6")),
    ("color-chart-5", FallbackValue::Single("#F59E0B")),
    ("color-chart-6", FallbackValue::Single("#F97316")),
    (
        "color-danger-bg",
        FallbackValue::LightDark {
            light: "#FEE2E2",
            dark: "#7F1D1D",
        },
    ),
    (
        "color-danger-text",
        FallbackValue::LightDark {
            light: "#991B1B",
            dark: "#FECACA",
        },
    ),
    (
        "color-destructive",
        FallbackValue::LightDark {
            light: "#EF4444",
            dark: "#F87171",
        },
    ),
    (
        "color-info-bg",
        FallbackValue::LightDark {
            light: "#DBEAFE",
            dark: "#1E3A8A",
        },
    ),
    (
        "color-info-text",
        FallbackValue::LightDark {
            light: "#1E40AF",
            dark: "#BFDBFE",
        },
    ),
    (
        "color-scrim",
        FallbackValue::LightDark {
            light: "#00000080",
            dark: "#00000099",
        },
    ),
    (
        "color-success",
        FallbackValue::LightDark {
            light: "#10B981",
            dark: "#34D399",
        },
    ),
    (
        "color-success-bg",
        FallbackValue::LightDark {
            light: "#DCFCE7",
            dark: "#14532D",
        },
    ),
    (
        "color-success-text",
        FallbackValue::LightDark {
            light: "#166534",
            dark: "#BBF7D0",
        },
    ),
    (
        "color-surface",
        FallbackValue::LightDark {
            light: "#FFFFFF",
            dark: "#1E293B",
        },
    ),
    (
        "color-surface-2",
        FallbackValue::LightDark {
            light: "#F1F5F9",
            dark: "#334155",
        },
    ),
    (
        "color-surface-3",
        FallbackValue::LightDark {
            light: "#F3F4F6",
            dark: "#475569",
        },
    ),
    (
        "color-text-body",
        FallbackValue::LightDark {
            light: "#334155",
            dark: "#CBD5E1",
        },
    ),
    (
        "color-text-muted",
        FallbackValue::LightDark {
            light: "#64748B",
            dark: "#94A3B8",
        },
    ),
    (
        "color-text-primary",
        FallbackValue::LightDark {
            light: "#0F172A",
            dark: "#F1F5F9",
        },
    ),
    (
        "color-text-subtle",
        FallbackValue::LightDark {
            light: "#94A3B8",
            dark: "#64748B",
        },
    ),
    (
        "color-warning-bg",
        FallbackValue::LightDark {
            light: "#FEF3C7",
            dark: "#78350F",
        },
    ),
    (
        "color-warning-text",
        FallbackValue::LightDark {
            light: "#92400E",
            dark: "#FDE68A",
        },
    ),
    ("radius-lg", FallbackValue::Num(12f64)),
    ("radius-md", FallbackValue::Num(8f64)),
    ("radius-sm", FallbackValue::Num(4f64)),
    ("spacing-1", FallbackValue::Num(4f64)),
    ("spacing-2", FallbackValue::Num(8f64)),
    ("spacing-3", FallbackValue::Num(12f64)),
    ("spacing-4", FallbackValue::Num(16f64)),
    ("spacing-5", FallbackValue::Num(24f64)),
    ("type-body-line-height", FallbackValue::Num(1.5f64)),
    ("type-body-size", FallbackValue::Num(14f64)),
    ("type-body-weight", FallbackValue::Num(400f64)),
    ("type-caption-line-height", FallbackValue::Num(1.4f64)),
    ("type-caption-size", FallbackValue::Num(12f64)),
    ("type-caption-weight", FallbackValue::Num(400f64)),
    ("type-display-letter-spacing", FallbackValue::Num(-0.5f64)),
    ("type-display-line-height", FallbackValue::Num(1f64)),
    ("type-display-size", FallbackValue::Num(64f64)),
    ("type-display-weight", FallbackValue::Num(700f64)),
    ("type-h1-line-height", FallbackValue::Num(1.2f64)),
    ("type-h1-size", FallbackValue::Num(24f64)),
    ("type-h1-weight", FallbackValue::Num(600f64)),
    ("type-h2-line-height", FallbackValue::Num(1.25f64)),
    ("type-h2-size", FallbackValue::Num(20f64)),
    ("type-h2-weight", FallbackValue::Num(600f64)),
    ("type-h3-line-height", FallbackValue::Num(1.3f64)),
    ("type-h3-size", FallbackValue::Num(16f64)),
    ("type-h3-weight", FallbackValue::Num(600f64)),
    (
        "type-uppercase-label-letter-spacing",
        FallbackValue::Num(1.5f64),
    ),
];
