use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use ddc::Edid;
use ddc_hi::Backend;
use ddc_hi::Ddc;
use ddc_hi::DisplayInfo;
use ddc_hi::Handle;
use ddc_i2c::I2cDdc;
use eyre::eyre;
use eyre::Context;
use eyre::ContextCompat;
use eyre::Result;
use i2c_linux::I2c;

use crate::calculate_new_brightness;

pub fn get_ddc_display(name: &str) -> Result<ddc_hi::Display> {
    let i2c_dev = Path::new("/dev").join(name);
    let mut ddc = I2cDdc::new(I2c::from_path(i2c_dev)?);
    let id = ddc
        .inner_ref()
        .inner_ref()
        .metadata()
        .map(|meta| meta.rdev())
        .unwrap_or_default();
    let mut edid = vec![0u8; 0x100];
    ddc.read_edid(0, &mut edid)
        .map_err(|e| eyre!("failed to read EDID for i2c-{}: {}", id, e))?;
    let display_info = DisplayInfo::from_edid(Backend::I2cDevice, id.to_string(), edid)
        .map_err(|e| eyre!("failed to parse EDID for i2c-{}: {}", id, e))?;
    Ok(ddc_hi::Display::new(Handle::I2cDevice(ddc), display_info))
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
pub fn set_ddc_brightness(ddc: &mut ddc_hi::Display, new_br: u8) -> Result<()> {
    ddc.handle
        .set_vcp_feature(0x10, new_br.into())
        .map_err(eyre::Error::msg)
        .context("failed to set brightness")
}
