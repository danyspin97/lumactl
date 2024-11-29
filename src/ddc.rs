use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use ddc::Edid;
use ddc_hi::Backend;
use ddc_hi::Ddc;
use ddc_hi::DisplayInfo;
use ddc_hi::Handle;
use ddc_i2c::I2cDdc;
use eyre::Context;
use eyre::Result;
use i2c_linux::I2c;

use crate::calculate_new_brightness;

pub fn get_ddc_display(name: &str) -> Option<ddc_hi::Display> {
    const SYS_DRM_ROOT: &str = "/sys/class/drm/";
    if let Some(i2c_device) = fs::read_dir(SYS_DRM_ROOT)
        .unwrap()
        // Filter the right drm device for the display
        .filter_map(|entry| entry.ok())
        .find_map(|entry| {
            let file_name = entry.file_name();
            let file_name = file_name.to_string_lossy();
            if file_name.starts_with("card") && file_name.ends_with(name) {
                // Try all the available i2c devices
                for index in 1..=20 {
                    let i2c_device = format!("i2c-{index}");
                    let path = entry.path().join(&i2c_device);
                    if path.exists() {
                        return Some(i2c_device);
                    }
                }

                None
            } else {
                None
            }
        })
    {
        let i2c_dev = Path::new("/dev").join(i2c_device);
        let mut ddc = I2cDdc::new(I2c::from_path(i2c_dev).unwrap());
        let id = ddc
            .inner_ref()
            .inner_ref()
            .metadata()
            .map(|meta| meta.rdev())
            .unwrap_or_default();
        let mut edid = vec![0u8; 0x100];
        ddc.read_edid(0, &mut edid)
            .map_err(|e| format!("failed to read EDID for i2c-{}: {}", id, e))
            .unwrap();
        let display_info = DisplayInfo::from_edid(Backend::I2cDevice, id.to_string(), edid)
            .map_err(|e| format!("failed to parse EDID for i2c-{}: {}", id, e))
            .unwrap();
        Some(ddc_hi::Display::new(Handle::I2cDevice(ddc), display_info))
    } else {
        None
    }
}

pub fn ddc_brightness(ddc: &mut ddc_hi::Display) -> Result<(u8, u8)> {
    ddc.handle
        .get_vcp_feature(0x10)
        .map(|val| {
            (
                val.value().try_into().unwrap_or(0),
                val.maximum().try_into().unwrap_or(100),
            )
        })
        .map_err(eyre::Error::msg)
}
pub fn set_ddc_brightness(ddc: &mut ddc_hi::Display, brightness: &str) -> Result<()> {
    let current_brightness = ddc_brightness(ddc)?;
    let final_brightness = calculate_new_brightness(current_brightness, brightness)?;
    set_ddc_brightness_impl(ddc, final_brightness)?;
    Ok(())
}

pub fn set_ddc_brightness_impl(ddc: &mut ddc_hi::Display, new_br: u8) -> Result<()> {
    ddc.handle
        .set_vcp_feature(0x10, new_br.into())
        .map_err(eyre::Error::msg)
        .context("failed to set brightness")
}
