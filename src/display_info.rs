use std::process::Command;

use eyre::{Context, Result};

#[derive(serde::Deserialize)]
pub struct DisplayInfo {
    pub model: String,
    pub name: String,
    pub description: String,
}

impl DisplayInfo {
    pub fn get_displays() -> Result<Vec<Self>> {
        let outputs = String::from_utf8(
            Command::new("wmctl")
                .args(["list-outputs", "--json"])
                .output()?
                .stdout,
        )?;
        serde_json::from_str(&outputs).context("failed to parse wmctl output")
    }

    /// Match the display name against the display's model name, id or description
    pub fn match_name(&self, display_name: &str) -> bool {
        self.name.contains(display_name)
            || self.model.contains(display_name)
            || self.description.contains(display_name)
    }
}
