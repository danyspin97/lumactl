use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use xdg::{BaseDirectories, BaseDirectoriesError};

#[derive(Serialize, Deserialize)]
pub enum IpcRequest {
    Get {
        display: Option<String>,
    },
    Set {
        display: Option<String>,
        brightness: String,
    },
}

#[derive(Serialize, Deserialize)]
pub struct DisplayBrightness {
    pub name: String,
    pub brightness: u8,
    pub max_brightness: u8,
}

#[derive(Serialize, Deserialize)]
pub enum IpcResponse {
    DisplayBrightness(Vec<DisplayBrightness>),
    Ok,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum IpcError {
    DisplayNotFound { display: String },
    GetBrightnessError { error: String },
    SetBrightnessError { error: String },
}

pub fn socket_path() -> Result<PathBuf, BaseDirectoriesError> {
    let xdg_dirs = BaseDirectories::with_prefix("lumactl")?;
    Ok(xdg_dirs.get_runtime_directory()?.join("lumactl.sock"))
}
