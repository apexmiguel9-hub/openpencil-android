//! TS-compatible style-guide MCP tools.

use std::collections::BTreeMap;

use op_ai_skills::style_guide::{
    select_style_guide, style_guide_registry, user_style_guides, Platform, SelectOptions,
    STYLE_GUIDE_TAGS,
};

use super::{McpTool, ToolErrorCode, ToolOutcome};

pub struct GetStyleGuideTags;

impl McpTool for GetStyleGuideTags {
    fn name(&self) -> &str {
        "get_style_guide_tags"
    }

    fn call(&self, _args: &BTreeMap<String, String>) -> ToolOutcome {
        let tags = match serde_json::to_string(STYLE_GUIDE_TAGS) {
            Ok(json) => json,
            Err(e) => {
                return ToolOutcome::Err(
                    ToolErrorCode::Internal,
                    format!("serialize style guide tags failed: {e}"),
                );
            }
        };
        let mut out = BTreeMap::new();
        out.insert("tags".into(), tags);
        out.insert("count".into(), STYLE_GUIDE_TAGS.len().to_string());
        ToolOutcome::Ok(out)
    }
}

pub struct GetStyleGuide;

impl McpTool for GetStyleGuide {
    fn name(&self) -> &str {
        "get_style_guide"
    }

    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        let tags = match parse_tags(args.get("tags").map(String::as_str)) {
            Ok(tags) => tags,
            Err(e) => return ToolOutcome::Err(ToolErrorCode::InvalidArgument, e.to_string()),
        };
        let platform = args
            .get("platform")
            .filter(|s| !s.is_empty())
            .map(|s| Platform::from_str(s));
        let opts = SelectOptions {
            tags,
            name: args.get("name").filter(|s| !s.is_empty()).cloned(),
            platform,
        };
        let Some(guide) = select_style_guide(style_guide_registry(), &opts) else {
            let mut out = BTreeMap::new();
            out.insert(
                "error".into(),
                "No matching style guide found. Try different tags or list available names.".into(),
            );
            return ToolOutcome::Ok(out);
        };
        let tags_json = match serde_json::to_string(&guide.tags) {
            Ok(json) => json,
            Err(e) => {
                return ToolOutcome::Err(
                    ToolErrorCode::Internal,
                    format!("serialize style guide tags failed: {e}"),
                );
            }
        };
        let mut out = BTreeMap::new();
        out.insert("name".into(), guide.name.clone());
        out.insert("tags".into(), tags_json);
        out.insert("platform".into(), guide.platform.as_str().into());
        out.insert("content".into(), guide.content.clone());
        ToolOutcome::Ok(out)
    }
}

/// The Asset Center's Styles tab, as a tool.
///
/// `get_style_guide` answers "give me *a* guide matching this", and only ever
/// from the shipped corpus. Neither half serves an agent choosing an asset: it
/// cannot see what exists, and the user's own imported `DESIGN.md` files —
/// the material most worth picking — were invisible to MCP entirely.
///
/// Imports come first, as they do in the panel: a list where your own material
/// sits below fifty shipped entries is a list you scroll past your own work in.
/// Ids keep the two apart, so an import that names itself after a shipped guide
/// cannot quietly take its place.
///
/// Passing `id` returns that one entry with its markdown, which is what closes
/// the loop for user guides — `get_style_guide` cannot reach them by name.
pub struct ListStyleGuides;

impl McpTool for ListStyleGuides {
    fn name(&self) -> &str {
        "list_style_guides"
    }

    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        let arg = |key: &str| {
            args.get(key)
                .map(|value| value.trim())
                .filter(|value| !value.is_empty())
        };
        let wanted_id = arg("id");
        let tag_filter = arg("tag");
        let platform_filter = arg("platform");

        let user_guides = user_style_guides();
        let mut entries: Vec<serde_json::Value> = Vec::new();

        for user in &user_guides {
            entries.push(serde_json::json!({
                "id": user.id,
                "name": user.guide.name,
                "platform": user.guide.platform.as_str(),
                "tags": user.guide.tags,
                "isUser": true,
                "swatches": user.swatches,
            }));
        }
        for guide in style_guide_registry() {
            entries.push(serde_json::json!({
                // A corpus guide is pinned by its bare name; that is its id.
                "id": guide.name,
                "name": guide.name,
                "platform": guide.platform.as_str(),
                "tags": guide.tags,
                "isUser": false,
            }));
        }

        if let Some(wanted_id) = wanted_id {
            let Some(mut entry) = entries
                .into_iter()
                .find(|entry| entry["id"].as_str() == Some(wanted_id))
            else {
                return ToolOutcome::Err(
                    ToolErrorCode::InvalidArgument,
                    format!("unknown style guide id {wanted_id:?}"),
                );
            };
            let content = user_guides
                .iter()
                .find(|user| user.id == wanted_id)
                .map(|user| user.guide.content.clone())
                .or_else(|| {
                    style_guide_registry()
                        .iter()
                        .find(|guide| guide.name == wanted_id)
                        .map(|guide| guide.content.clone())
                });
            if let Some(content) = content {
                entry["content"] = serde_json::Value::String(content);
            }
            return ToolOutcome::OkJson(serde_json::json!({ "guides": [entry] }).to_string());
        }

        entries.retain(|entry| {
            let tag_ok = tag_filter.is_none_or(|tag| {
                entry["tags"]
                    .as_array()
                    .is_some_and(|tags| tags.iter().any(|candidate| candidate == tag))
            });
            let platform_ok =
                platform_filter.is_none_or(|platform| entry["platform"].as_str() == Some(platform));
            tag_ok && platform_ok
        });
        let count = entries.len();
        ToolOutcome::OkJson(serde_json::json!({ "guides": entries, "count": count }).to_string())
    }
}

pub fn list_style_guides_snapshot() -> ListStyleGuides {
    ListStyleGuides
}

pub fn get_style_guide_tags_snapshot() -> GetStyleGuideTags {
    GetStyleGuideTags
}

pub fn get_style_guide_snapshot() -> GetStyleGuide {
    GetStyleGuide
}

/// The `tags` arg looked like a JSON array but did not deserialize into
/// one. One-variant enum: the comma-separated spelling cannot fail, so this
/// is the only way `parse_tags` refuses. `Display` reproduces the previous
/// message byte-for-byte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagsParseError {
    /// The serde error text.
    pub detail: String,
}

impl std::fmt::Display for TagsParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let detail = &self.detail;
        write!(f, "tags must be a string array: {detail}")
    }
}

impl std::error::Error for TagsParseError {}

fn parse_tags(raw: Option<&str>) -> Result<Vec<String>, TagsParseError> {
    let Some(raw) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(Vec::new());
    };
    if raw.starts_with('[') {
        return serde_json::from_str::<Vec<String>>(raw).map_err(|e| TagsParseError {
            detail: e.to_string(),
        });
    }
    Ok(raw
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect())
}
