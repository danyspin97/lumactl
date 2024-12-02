mod backlight;
mod brightness_control;
mod ddc;
mod display_info;

use brightness_control::BrightnessControl;
use clap::Parser;
use clap::Subcommand;
use ddc::ddc_brightness;
use ddc::get_ddc_display;
use ddc::set_ddc_brightness;
use display_info::DisplayInfo;
use eyre::bail;
use eyre::ensure;
use eyre::Context;
use eyre::ContextCompat;
use eyre::Result;

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

fn main() -> Result<()> {
    let args = Args::parse();

    match args.cmd {
        Subcmd::Get {
            display,
            percentage,
        } => {
            if let Some(display_name) = display {
                let mut br_ctl = BrightnessControl::get_from_name(&display_name)?;
                match br_ctl.brightness() {
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
                displays.into_iter().for_each(|display| {
                    let res = BrightnessControl::for_device(&display.name)
                        .with_context(|| {
                            format!("unable to find brightness control for {}", display.name)
                        })
                        .and_then(|br_ctl| {
                            br_ctl.and_then(|mut br_ctl| {
                                br_ctl.brightness().map(|(brightness, max_brightness)| {
                                    println!(
                                        "{}: {}",
                                        display.name,
                                        format_brightness(brightness, max_brightness, percentage)
                                    );
                                })
                            })
                        });

                    match res {
                        Ok(_) => {}
                        Err(err) => eprintln!("{err:?}"),
                    }
                });
            }
        }
        Subcmd::Set {
            display,
            brightness,
        } => {
            if let Some(display_name) = display {
                let mut br_ctl = BrightnessControl::get_from_name(&display_name)?;
                match br_ctl.set_brightness(brightness.as_str()) {
                    Ok(_) => {}
                    Err(err) => eprintln!("{err:?}"),
                }
            } else {
                let displays = DisplayInfo::get_displays()?;
                displays.into_iter().for_each(|display| {
                    let res = BrightnessControl::for_device(&display.name)
                        .with_context(|| {
                            format!("unable to find brightness control for {}", display.name)
                        })
                        .and_then(|br_ctl| {
                            br_ctl.and_then(|mut br_ctl| br_ctl.set_brightness(&brightness))
                        });

                    match res {
                        Ok(_) => {}
                        Err(err) => eprintln!("{err:?}"),
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
