use std::{fs, path::{Path, PathBuf}};

use eyre::{bail, Result};

use crate::{
    backlight::{backlight_brightness, set_backlight_brightness},
    calculate_new_brightness,
    ddc::{ddc_brightness, get_ddc_display, set_ddc_brightness},
    display_info::DisplayInfo,
};

const SYS_DRM_ROOT: &str = "/sys/class/drm/";

pub enum BrightnessControl {
    Backlight(PathBuf),
    I2c(ddc_hi::Display),
}

impl BrightnessControl {
    /// Get the brightness control (either i2c or backlight) from the --display argument
    /// passed by the user, which might me the name, model or description
    pub fn get_from_name(display_arg: &str) -> Result<Self, eyre::Error> {
        let br_ctl = if let Some(br_ctl) = Self::for_device(&display_arg) {
            br_ctl
        } else {
            // If we can't find the display by its name, try the model and description
            let displays = DisplayInfo::get_displays()?;
            let display = displays.iter().find(|d| d.match_name(&display_arg));
            match display {
                Some(display) => {
                    let br_ctl = BrightnessControl::for_device(&display.name);
                    match br_ctl {
                        Some(br_ctl) => br_ctl,
                        None => bail!("Display {} not found", display.name),
                    }
                }
                None => bail!("Display {} not found", display_arg),
            }
        };
        br_ctl
    }

    pub fn for_device(name: &str) -> Option<Result<Self>> {
        fs::read_dir(SYS_DRM_ROOT)
            .unwrap()
            // Filter the right drm device for the display
            .filter_map(|entry| entry.ok())
            .find_map(|entry| {
                let file_name = entry.file_name();
                let file_name = file_name.to_string_lossy();
                if file_name.starts_with("card") && file_name.ends_with(name) {
                    // Try searching for the backlight first
                    if let Some(backlight) = fs::read_dir(entry.path())
                        .unwrap()
                        .filter_map(|entry| entry.ok())
                        .find_map(|entry| {
                            let file_name = entry.file_name();
                            let file_name = file_name.to_string_lossy();
                            ["amdgpu_bl", "intel_backlight", "acpi_video"]
                                .iter()
                                .find_map(|backlight| {
                                    if file_name.starts_with(backlight) {
                                        Some(entry.path())
                                    } else {
                                        None
                                    }
                                })
                        })
                    {
                        return Some(Ok(BrightnessControl::Backlight(backlight)));
                    }
                    // Try all the available i2c devices
                    for index in 1..=20 {
                        let i2c_device = format!("i2c-{index}");
                        let path = entry.path().join(&i2c_device);
                        if path.exists() {
                            let ddc_display = get_ddc_display(&i2c_device);
                            match ddc_display {
                                Ok(ddc_display) => {
                                    return Some(Ok(BrightnessControl::I2c(ddc_display)));
                                }
                                Err(err) => {
                                    return Some(Err(err));
                                }
                            }
                        }
                    }
                    None
                } else {
                    None
                }
            })
    }

    pub fn brightness(&mut self) -> Result<(u8, u8)> {
        match self {
            BrightnessControl::Backlight(backlight) => backlight_brightness(Path::new(backlight)),
            BrightnessControl::I2c(ref mut i2c_display) => ddc_brightness(i2c_display),
        }
    }

    pub(crate) fn set_brightness(&mut self, new_br: &str) -> Result<()> {
        let current_brightness = self.brightness()?;
        let final_brightness = calculate_new_brightness(current_brightness, new_br)?;

        match self {
            BrightnessControl::Backlight(backlight) => {
                set_backlight_brightness(Path::new(backlight), final_brightness)
            }
            BrightnessControl::I2c(ref mut i2c_display) => {
                set_ddc_brightness(i2c_display, final_brightness)
            }
        }
    }
}
