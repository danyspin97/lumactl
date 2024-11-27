//! IPC socket server.
//! Based on <https://github.com/catacombing/catacomb/blob/master/src/ipc_server.rs>

use std::collections::HashSet;
use std::fs;
use std::io::{BufReader, BufWriter, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

use eyre::{bail, Result};
use eyre::{ensure, Context};
use lumaipc::{DisplayBrightness, IpcError, IpcRequest, IpcResponse};
use smithay_client_toolkit::reexports::client::QueueHandle;

use crate::socket::SocketSource;
use crate::Lumactld;

/// Create an IPC socket.
pub fn listen_on_ipc_socket(socket_path: &Path) -> Result<SocketSource> {
    // Try to delete the socket if it exists already.
    if socket_path.exists() {
        fs::remove_file(socket_path)?;
    }

    // Spawn unix socket event source.
    let listener = UnixListener::bind(socket_path)?;
    let socket = SocketSource::new(listener)?;
    Ok(socket)
}

/// Handle IPC socket messages.
pub fn handle_message(
    ustream: UnixStream,
    qh: QueueHandle<Lumactld>,
    lumactld: &mut Lumactld,
) -> Result<()> {
    const SIZE: usize = 4096;
    let mut buffer = [0; SIZE];

    // Read new content to buffer.
    let mut stream = BufReader::new(&ustream);
    let n = stream
        .read(&mut buffer)
        .context("error while reading line from IPC")?;
    // The message is empty
    if n == 0 {
        return Ok(());
    }
    ensure!(n != SIZE, "The message received was too big");

    // Read pending events on socket.
    let message: IpcRequest = serde_json::from_slice(&buffer[..n])
        .with_context(|| format!("error while deserializing message {:?}", &buffer[..n]))?;

    // Handle IPC events.
    let resp: Result<IpcResponse, IpcError> = match message {
        IpcRequest::Get { display } => {
            if let Some(display_name) = display {
                let display = lumactld
                    .displays
                    .iter_mut()
                    .find(|d| d.match_name(&display_name));
                match display {
                    Some(display) => match display.brightness() {
                        Ok((brightness, max_brightness)) => {
                            Ok(IpcResponse::DisplayBrightness(vec![DisplayBrightness {
                                name: display.info.name.as_ref().unwrap().clone(),
                                brightness,
                                max_brightness,
                            }]))
                        }
                        Err(err) => Err(IpcError::GetBrightnessError {
                            error: err.to_string(),
                        }),
                    },
                    None => Err(IpcError::DisplayNotFound {
                        display: display_name,
                    }),
                }
            } else {
                match lumactld
                    .displays
                    .iter_mut()
                    .map(|display| match display.brightness() {
                        Ok((brightness, max_brightness)) => Ok(DisplayBrightness {
                            name: display.info.name.as_ref().unwrap().clone(),
                            brightness,
                            max_brightness,
                        }),
                        Err(err) => Err(IpcError::GetBrightnessError {
                            error: err.to_string(),
                        }),
                    })
                    .collect::<Result<Vec<_>, IpcError>>()
                {
                    Ok(displays_brightness) => {
                        Ok(IpcResponse::DisplayBrightness(displays_brightness))
                    }
                    Err(err) => Err(err),
                }
            }
        }
        IpcRequest::Set {
            display,
            brightness,
        } => {
            if let Some(display_name) = display {
                let display = lumactld
                    .displays
                    .iter_mut()
                    .find(|d| d.match_name(&display_name));
                match display {
                    Some(display) => match display.set_brightness(&brightness) {
                        Ok(_) => Ok(IpcResponse::Ok),
                        Err(err) => Err(IpcError::SetBrightnessError {
                            error: err.to_string(),
                        }),
                    },
                    None => Err(IpcError::DisplayNotFound {
                        display: display_name,
                    }),
                }
            } else {
                match lumactld
                    .displays
                    .iter_mut()
                    .try_for_each(|display| -> Result<(), IpcError> {
                        match display.set_brightness(&brightness) {
                            Ok(_) => Ok(()),
                            Err(err) => Err(IpcError::SetBrightnessError {
                                error: err.to_string(),
                            }),
                        }
                    }) {
                    Ok(_) => Ok(IpcResponse::Ok),
                    Err(err) => Err(err),
                }
            }
        }
    };

    let mut stream = BufWriter::new(ustream);
    stream
        .write_all(&serde_json::to_vec(&resp).unwrap())
        .context("unable to write response to the IPC client")?;
    // .suggestion("Probably the client died, try running it again")?;

    Ok(())
}
