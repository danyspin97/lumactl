mod ddc;
mod display_info;

use std::path::Path;

use clap::Parser;
use clap::Subcommand;
use ddc::ddc_brightness;
use ddc::get_ddc_display;
use ddc::set_ddc_brightness;
use display_info::DisplayInfo;
use eyre::bail;
use eyre::ensure;
use eyre::eyre;
use eyre::Context;
use eyre::ContextCompat;
use eyre::Result;
use log::warn;

const BACKLIGHT_PATHS: [&str; 4] = [
    "/sys/class/backlight/intel_backlight/",
    "/sys/class/backlight/amdgpu_bl0/",
    "/sys/class/backlight/radeon_bl0/",
    "/sys/class/backlight/acpi_video0/",
];

#[derive(Parser)]
#[command(name = "lumactl")]
#[command(about = "Control the brightness of the displays")]
#[command(version)]
#[command(propagate_version = true)]
struct Args {
    #[clap(subcommand)]
    cmd: Subcmd,
    #[clap(long, short, help = "Enable verbose logging")]
    verbose: bool,
}

#[derive(Debug, Subcommand, Clone)]
enum Subcmd {
    #[clap(about = "Get the brightness of one or all displays")]
    Get {
        #[clap(
            long,
            short,
            help = "The display to get the brightness of (all displays if not provided)"
        )]
        display: Option<String>,
        #[clap(long, short, help = "Output the brightness as a percentage")]
        percentage: bool,
    },
    #[clap(about = "Get the brightness of one or all displays")]
    Set {
        #[clap(
            long,
            short,
            help = "The display to set the brightness of (all displays if not provided)"
        )]
        display: Option<String>,
        #[clap(help = "The brightness to set")]
        brightness: String,
    },
}

/// Calculate the new brightness value based on the current brightness value
/// We need &mut self because Display::brightness will be called
fn calculate_new_brightness(current_brightness: (u8, u8), new_brightness: &str) -> Result<u8> {
    // If the brightness string start with a '-' it means relative decrease
    // If the brightness string start with a '+' it means relative increase
    // If the brightness string is a number it means absolute value
    // If the brightness ends with a '%' it means percentage
    // Apply brightness reletive increase/decrease with percentage as well

    let brightness = new_brightness.trim();
    ensure!(!brightness.is_empty(), "brightness cannot be empty");
    let first_char = brightness.chars().next().unwrap();
    let (br, max_br) = current_brightness;
    let mut new_br = if first_char == '+' || first_char == '-' {
        &brightness[1..]
    } else {
        brightness
    };
    ensure!(!new_br.is_empty(), "invalid brightness value");
    let percentage = if new_br.ends_with('%') {
        new_br = &new_br[..new_br.len() - 1];
        true
    } else {
        false
    };
    let new_br = new_br.parse::<u8>().context("invalid brightness value")?;
    // if the value provided is a percentage, calculate the absolute value with
    // new_br * max_br / 100
    let set_val = if percentage {
        (new_br as f32 * max_br as f32 / 100.0) as u8
    } else {
        new_br
    };
    let new_br = match first_char {
        '+' => {
            // We do not want to overflow the brightness value
            br.saturating_add(set_val)
        }
        '-' => br.saturating_sub(set_val),
        _ => set_val,
    };

    // Apply max allowed values
    Ok(new_br.min(max_br))
}

fn backlight_brightness() -> Result<(u8, u8)> {
    for path in BACKLIGHT_PATHS {
        let br_path = Path::new(path).join("brightness");
        if br_path.exists() {
            let br = if let Some(value) = parse_path(br_path) {
                value
            } else {
                continue;
            };
            let max_br_path = Path::new(path).join("max_brightness");
            if max_br_path.exists() {
                if let Some(max_br) = parse_path(max_br_path) {
                    return Ok((br, max_br));
                } else {
                    return Err(eyre!("Failed to read max_brightness for {}", path));
                }
            }
        }
    }

    bail!("failed to find a valid backlight path")
}

fn set_backlight_brightness(new_br: u8) -> Result<(), eyre::Error> {
    for path in BACKLIGHT_PATHS {
        let br_path = Path::new(path).join("brightness");
        if br_path.exists() {
            std::fs::write(&br_path, new_br.to_string()).context("failed to write brightness")?;
            return Ok(());
        }
    }
    bail!("failed to find a valid backlight path");
}

fn parse_path(path: std::path::PathBuf) -> Option<u8> {
    match std::fs::read_to_string(&path) {
        Ok(val) => match val.trim().parse::<u8>() {
            Ok(val) => return Some(val),
            Err(err) => warn!("Failed to parse {}: {}", path.display(), err),
        },
        Err(err) => warn!("Failed to read {}: {}", path.display(), err),
    }
    None
}

fn main() -> Result<()> {
    let args = Args::parse();

    match args.cmd {
        Subcmd::Get {
            display,
            percentage,
        } => {
            if let Some(display_name) = display {
                let mut ddc_display = if let Some(ddc_display) = ddc::get_ddc_display(&display_name)
                {
                    ddc_display
                } else {
                    // If we can't find the display by its name, try the model and description
                    let displays = DisplayInfo::get_displays()?;
                    let display = displays.iter().find(|d| d.match_name(&display_name));
                    match display {
                        Some(display) => {
                            let ddc_display = get_ddc_display(&display.name);
                            ddc_display
                                .with_context(|| format!("Display {} not found", display.name))?
                        }
                        None => bail!("Display not found: {}", display_name),
                    }
                };
                match ddc_brightness(&mut ddc_display) {
                    Ok((brightness, max_brightness)) => {
                        println!(
                            "{}",
                            format_brightness(brightness, max_brightness, percentage)
                        );
                    }
                    Err(err) => eprintln!("{err:?}"),
                }
            } else {
                let displays = DisplayInfo::get_displays()?;
                displays
                    .iter()
                    .for_each(|display| match get_ddc_display(&display.name) {
                        Some(mut ddc_display) => match ddc_brightness(&mut ddc_display) {
                            Ok((brightness, max_brightness)) => {
                                println!(
                                    "{}: {}",
                                    display.name,
                                    format_brightness(brightness, max_brightness, percentage)
                                );
                            }
                            Err(err) => eprintln!("{err:?}"),
                        },
                        None => todo!(),
                    });
            }
        }
        Subcmd::Set {
            display,
            brightness,
        } => {
            if let Some(display_name) = display {
                let mut ddc_display = if let Some(ddc_display) = ddc::get_ddc_display(&display_name)
                {
                    ddc_display
                } else {
                    // If we can't find the display by its name, try the model and description
                    let displays = DisplayInfo::get_displays()?;
                    let display = displays.iter().find(|d| d.match_name(&display_name));
                    match display {
                        Some(display) => {
                            let ddc_display = get_ddc_display(&display.name);
                            ddc_display
                                .with_context(|| format!("Display {} not found", display.name))?
                        }
                        None => bail!("Display not found: {}", display_name),
                    }
                };
                set_ddc_brightness(&mut ddc_display, &brightness)?;
            } else {
                let displays = DisplayInfo::get_displays()?;
                displays.iter().fold(true, |success, display| {
                    match get_ddc_display(&display.name) {
                        Some(mut ddc_display) => {
                            match set_ddc_brightness(&mut ddc_display, &brightness) {
                                Ok(_) => success,
                                Err(err) => {
                                    eprintln!("{err:?}");
                                    false
                                }
                            }
                        }
                        None => todo!(),
                    }
                });
            }
        }
    };

    Ok(())
}

fn format_brightness(brightness: u8, max_brightness: u8, percentage: bool) -> String {
    if percentage {
        format!("{:.0}%", brightness as f32 / max_brightness as f32 * 100.0)
    } else {
        format!("{}/{}", brightness, max_brightness)
    }
}
