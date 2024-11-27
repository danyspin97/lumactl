mod ipc_server;
mod socket;

use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use clap::Parser;
use ddc_hi::Ddc;
use eyre::bail;
use eyre::ensure;
use eyre::eyre;
use eyre::Context;
use eyre::Result;
use ipc_server::handle_message;
use ipc_server::listen_on_ipc_socket;
use log::error;
use log::warn;
use smithay_client_toolkit::output::OutputInfo;
use smithay_client_toolkit::reexports::calloop;
use smithay_client_toolkit::reexports::calloop_wayland_source::WaylandSource;
use smithay_client_toolkit::{
    delegate_output, delegate_registry,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
};
use wayland_client::{globals::registry_queue_init, protocol::wl_output, Connection, QueueHandle};

use lumaipc::socket_path;

const BACKLIGHT_PATHS: [&str; 4] = [
    "/sys/class/backlight/intel_backlight/",
    "/sys/class/backlight/amdgpu_bl0/",
    "/sys/class/backlight/radeon_bl0/",
    "/sys/class/backlight/acpi_video0/",
];

#[derive(Parser)]
#[command(name = "lumad")]
#[command(about = "Helper daemon to control the brightness of the displays")]
#[command(version)]
#[command(propagate_version = true)]
struct Args {
    #[clap(
        short,
        long,
        help = "Detach from the terminal and run in the background"
    )]
    daemon: bool,
}

struct Display {
    info: OutputInfo,
    ddc: Option<ddc_hi::Display>,
}

impl Display {
    fn brightness(&mut self) -> Result<(u8, u8)> {
        match &mut self.ddc {
            Some(ddc) => ddc_brightness(ddc),
            None => backlight_brightness(),
        }
    }

    /// Match the display name against the display's model name, id or description
    fn match_name(&self, display_name: &str) -> bool {
        self.info
            .name
            .as_ref()
            .is_some_and(|name| name.contains(display_name))
            || self.info.model.contains(display_name)
            || self
                .info
                .description
                .as_ref()
                .is_some_and(|desc| desc.contains(display_name))
    }

    fn set_brightness(&mut self, brightness: &str) -> Result<()> {
        let new_br = self.calculate_new_brightness(brightness)?;
        match &mut self.ddc {
            Some(ddc) => set_ddc_brightness(ddc, new_br),
            None => set_backlight_brightness(new_br),
        }
    }

    /// Calculate the new brightness value based on the current brightness value
    /// We need &mut self because Display::brightness will be called
    fn calculate_new_brightness(&mut self, brightness: &str) -> Result<u8> {
        // If the brightness string start with a '-' it means relative decrease
        // If the brightness string start with a '+' it means relative increase
        // If the brightness string is a number it means absolute value
        // If the brightness ends with a '%' it means percentage
        // Apply brightness reletive increase/decrease with percentage as well

        let brightness = brightness.trim();
        ensure!(!brightness.is_empty(), "brightness cannot be empty");
        let first_char = brightness.chars().next().unwrap();
        let (br, max_br) = self.brightness().context("unable to get brightness")?;
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
}

fn set_ddc_brightness(ddc: &mut ddc_hi::Display, new_br: u8) -> Result<()> {
    ddc.handle
        .set_vcp_feature(0x10, new_br.into())
        .map_err(eyre::Error::msg)
        .context("failed to set brightness")
}

fn ddc_brightness(ddc: &mut ddc_hi::Display) -> Result<(u8, u8)> {
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

    // We initialize the logger for the purpose of debugging.
    // Set `RUST_LOG=debug` to see extra debug information.
    env_logger::init();

    // Try to connect to the Wayland server.
    let conn = Connection::connect_to_env()?;

    // Now create an event queue and a handle to the queue so we can create objects.
    let (globals, mut event_queue) = registry_queue_init(&conn).unwrap();
    let qh = event_queue.handle();

    // Initialize the registry handling so other parts of Smithay's client toolkit may bind
    // globals.
    let registry_state = RegistryState::new(&globals);

    // Initialize the delegate we will use for outputs.
    let output_delegate = OutputState::new(&globals, &qh);

    // Set up application state.
    //
    // This is where you will store your delegates and any data you wish to access/mutate while the
    // application is running.
    let mut lumactld = Lumactld::new(registry_state, output_delegate);

    // `OutputState::new()` binds the output globals found in `registry_queue_init()`.
    //
    // After the globals are bound, we need to dispatch again so that events may be sent to the newly
    // created objects.
    event_queue.roundtrip(&mut lumactld)?;

    lumactld.reload_displays();

    let mut event_loop = calloop::EventLoop::<Lumactld>::try_new()?;

    WaylandSource::new(conn.clone(), event_queue)
        .insert(event_loop.handle())
        .map_err(|e| eyre!("insterting the wayland source into the event loop: {e}"))?;

    let socket = listen_on_ipc_socket(&socket_path()?).context("spawning the ipc socket")?;
    // Add source to calloop loop.
    event_loop
        .handle()
        .insert_source(socket, |stream, _, lumactl| {
            if let Err(err) = handle_message(stream, lumactl) {
                error!("{:?}", err);
            }
        })?;

    let (ctrlc_ping, ctrl_ping_source) = calloop::ping::make_ping()?;

    let should_exit = Arc::new(AtomicBool::new(false));
    let should_exit_clone = should_exit.clone();
    // Handle SIGINT, SIGTERM, and SIGHUP, so that the application can stop nicely
    ctrlc::set_handler(move || {
        // Just wake up the event loop. The actual exit will be handled by the main loop
        // The event loop callback will set should_exit to true
        ctrlc_ping.ping();
    })
    .expect("Error setting Ctrl-C handler");
    event_loop
        .handle()
        .insert_source(ctrl_ping_source, move |_, _, _| {
            should_exit_clone.store(true, Ordering::Release);
        })
        .map_err(|e| eyre!("inserting the filelist event listener in the event loop: {e}"))?;

    lumactld.output_changed = false;
    loop {
        if should_exit.load(Ordering::Acquire) {
            break Ok(());
        }

        if lumactld.output_changed {
            lumactld.reload_displays();
            lumactld.output_changed = false;
        }

        event_loop.dispatch(None, &mut lumactld)?;
    }
}

/// Application data.
///
/// This type is where the delegates for some parts of the protocol and any application specific data will
/// live.
struct Lumactld {
    registry_state: RegistryState,
    output_state: OutputState,
    displays: Vec<Display>,
    output_changed: bool,
}
impl Lumactld {
    fn new(registry_state: RegistryState, output_state: OutputState) -> Self {
        Self {
            registry_state,
            output_state,
            displays: Vec::new(),
            output_changed: false,
        }
    }

    pub fn reload_displays(&mut self) {
        let mut ddc_displays = ddc_hi::Display::enumerate();
        if let Err(err) = ddc_displays
            .iter_mut()
            .try_for_each(|ddc_display| -> Result<()> {
                ddc_display
                    .update_capabilities()
                    .map_err(|err| eyre!(err))
                    .context("failed to update capabilities")
            })
        {
            warn!("Failed to enumerate DDC displays: {:?}", err);
        }

        // Our outputs have been initialized with data, we may access what outputs exist and information about
        // said outputs using the output delegate.
        self.displays = self
            .output_state
            .outputs()
            .filter_map(|output| -> Option<Display> {
                let info = self.output_state.info(&output);
                if let Some(info) = info {
                    let index = ddc_displays.iter().position(|d| match &d.info.model_name {
                        Some(model) => &info.model == model,
                        None => false,
                    });
                    let ddc = index.map(|index| ddc_displays.remove(index));
                    Some(Display {
                        info: info.clone(),
                        ddc,
                    })
                } else {
                    warn!("output has no info");
                    None
                }
            })
            .collect::<Vec<_>>();
    }
}

// In order to use OutputDelegate, we must implement this trait to indicate when something has happened to an
// output and to provide an instance of the output state to the delegate when dispatching events.
impl OutputHandler for Lumactld {
    // First we need to provide a way to access the delegate.
    //
    // This is needed because delegate implementations for handling events use the application data type in
    // their function signatures. This allows the implementation to access an instance of the type.
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    // Then there exist these functions that indicate the lifecycle of an output.
    // These will be called as appropriate by the delegate implementation.

    fn new_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
        self.output_changed = true;
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
        self.output_changed = true;
    }
}

// Now we need to say we are delegating the responsibility of output related events for our application data
// type to the requisite delegate.
delegate_output!(Lumactld);

// In order for our delegate to know of the existence of globals, we need to implement registry
// handling for the program. This trait will forward events to the RegistryHandler trait
// implementations.
delegate_registry!(Lumactld);

// In order for delegate_registry to work, our application data type needs to provide a way for the
// implementation to access the registry state.
//
// We also need to indicate which delegates will get told about globals being created. We specify
// the types of the delegates inside the array.
impl ProvidesRegistryState for Lumactld {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    registry_handlers! {
        // Here we specify that OutputState needs to receive events regarding the creation and destruction of
        // globals.
        OutputState,
    }
}
