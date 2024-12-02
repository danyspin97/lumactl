use std::path::Path;

use eyre::{Context, Result};

pub fn backlight_brightness(path: &Path) -> Result<(u8, u8)> {
    let br_path = Path::new(path).join("brightness");
    let br =
        parse_path(br_path).with_context(|| format!("failed to read brightness for {:?}", path))?;
    let max_br_path = Path::new(path).join("max_brightness");
    let max_br = parse_path(max_br_path)
        .with_context(|| format!("failed to read max_brightness for {:?}", path))?;
    Ok((br, max_br))
}

pub fn set_backlight_brightness(path: &Path, new_br: u8) -> Result<(), eyre::Error> {
    let br_path = Path::new(path).join("brightness");
    std::fs::write(&br_path, new_br.to_string()).context("failed to write brightness")
}

fn parse_path(path: std::path::PathBuf) -> Result<u8> {
    std::fs::read_to_string(&path)?
        .trim()
        .parse()
        .context("failed to parse brightness")
}
