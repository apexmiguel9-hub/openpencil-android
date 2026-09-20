//! Export embedded OpenPencil AI skills for external agent runtimes.

use std::fs;
use std::path::Path;

use serde_json::json;

use crate::cli_error::CliError;

const DEFAULT_SKILL_OUT_DIR: &str = ".claude/skills";

pub(crate) fn run_export(name: &str, out_dir: Option<&str>) -> Result<String, CliError> {
    let skill = op_ai_skills::get_skill_by_name(name)
        .ok_or_else(|| CliError::Usage(format!("unknown skill {name:?}")))?;
    let root = out_dir.unwrap_or(DEFAULT_SKILL_OUT_DIR);
    let dir = Path::new(root).join(&skill.meta.name);
    fs::create_dir_all(&dir)
        .map_err(|e| CliError::Io(format!("cannot create {}: {e}", dir.display())))?;

    let path = dir.join("SKILL.md");
    let contents = format!(
        "---\nname: {}\ndescription: {}\n---\n\n{}\n",
        skill.meta.name,
        skill.meta.description.trim(),
        skill.content.trim()
    );
    fs::write(&path, contents)
        .map_err(|e| CliError::Io(format!("cannot write {}: {e}", path.display())))?;

    Ok(json!({
        "ok": true,
        "path": path.to_string_lossy(),
    })
    .to_string())
}
