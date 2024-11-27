use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
};

use clap::{Parser, Subcommand};
use lumaipc::{socket_path, IpcError, IpcRequest, IpcResponse};

#[derive(Parser)]
#[command(name = "lumactl")]
#[command(about = "Control the brightness of the displays")]
#[command(version)]
#[command(propagate_version = true)]
struct Args {
    #[clap(subcommand)]
    cmd: Command,
}

#[derive(Debug, Subcommand, Clone)]
enum Command {
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

fn main() {
    let args = Args::parse();

    let mut percentage = false;
    let mut conn = UnixStream::connect(socket_path().unwrap()).unwrap();
    let req = match &args.cmd {
        Command::Get {
            display,
            percentage: p,
        } => {
            percentage = *p;
            IpcRequest::Get {
                display: display.clone(),
            }
        }
        Command::Set {
            display,
            brightness,
        } => IpcRequest::Set {
            display: display.clone(),
            brightness: brightness.clone(),
        },
    };

    conn.write_all(&serde_json::to_vec(&req).unwrap()).unwrap();
    let mut buf = String::new();
    conn.read_to_string(&mut buf).unwrap();
    let res: Result<IpcResponse, IpcError> =
        serde_json::from_str(&buf).expect("wpaperd to return a valid json");
    match res {
        Ok(resp) => match resp {
            IpcResponse::DisplayBrightness(displays) => {
                if displays.len() == 1 {
                    let display = displays.first().unwrap();
                    let br_string =
                        format_brightness(display.brightness, display.max_brightness, percentage);
                    println!("{}", br_string);
                } else {
                    for display in displays {
                        let br_string = format_brightness(
                            display.brightness,
                            display.max_brightness,
                            percentage,
                        );
                        println!("{}: {}", display.name, br_string);
                    }
                }
            }
            IpcResponse::Ok => {}
        },
        Err(err) => match err {
            IpcError::DisplayNotFound { display } => eprintln!("Display {} not found", display),
            IpcError::GetBrightnessError { error } => {
                eprintln!("Error getting brightness: {}", error)
            }
            IpcError::SetBrightnessError { error } => {
                eprintln!("Error setting brightness: {}", error)
            }
        },
    }
}

fn format_brightness(brightness: u8, max_brightness: u8, percentage: bool) -> String {
    if percentage {
        format!("{:.0}%", brightness as f32 / max_brightness as f32 * 100.0)
    } else {
        format!("{}/{}", brightness, max_brightness)
    }
}
