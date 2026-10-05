// <META - ROLE : Version-dispatched migrate chain; currently v1 only; higher version -> E_VERSION_TOO_NEW. | L1-30>

use crate::model::Project;
use crate::ScoreError;

pub fn migrate(bytes: &[u8]) -> Result<Project, ScoreError> {
    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|e| ScoreError {
        code: "E_FORMAT",
        message: format!("invalid JSON: {e}"),
    })?;
    let version = value
        .get("format_version")
        .and_then(|v| v.as_u64())
        .ok_or(ScoreError {
            code: "E_FORMAT",
            message: "missing format_version".to_string(),
        })?;
    if version > crate::model::FORMAT_VERSION_SUPPORTED as u64 {
        return Err(ScoreError {
            code: "E_VERSION_TOO_NEW",
            message: format!("format_version {version} is newer than supported"),
        });
    }
    // v1 -> v1: empty chain. Future versions dispatch here.
    let project: Project = serde_json::from_value(value).map_err(|e| ScoreError {
        code: "E_FORMAT",
        message: format!("model parse failed: {e}"),
    })?;
    Ok(project)
}
