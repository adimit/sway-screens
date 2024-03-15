use std::{
    collections::HashMap,
    fmt::{self, Debug},
    hash::BuildHasherDefault,
    path::Display,
};

use anyhow::Result;
use fxhash::{FxHashMap, FxHasher};
use hyprland::shared::HyprData;
use tracing::{debug, info, trace, warn};
use wayland_client::{
    backend::ObjectId, event_created_child, protocol::wl_registry, Dispatch, Proxy,
};
use wayland_protocols_wlr::output_management::v1::client::{
    zwlr_output_head_v1::ZwlrOutputHeadV1, zwlr_output_manager_v1::ZwlrOutputManagerV1,
    zwlr_output_mode_v1::ZwlrOutputModeV1,
};

fn parse_setup(arg: Vec<String>) -> Result<Vec<usize>> {
    if arg.is_empty() {
        return Ok(vec![]);
    }

    arg[0]
        .chars()
        .map(|c| {
            c.to_digit(10)
                .map(|i| i as usize)
                .ok_or(anyhow::anyhow!("char '{}' not a digit", c))
        })
        .collect::<Result<Vec<usize>>>()
}

#[derive(Debug)]
struct Position {
    x: i32,
    y: i32,
}

#[derive(Debug)]
struct Resolution {
    width: i32,
    height: i32,
}

#[derive(Debug)]
struct Output {
    name: String,
    description: String,
    current_resolution: Resolution,
    preferred_resolution: Resolution,
    position: Position,
}

#[derive(Debug)]
struct Mode {
    resolution: Resolution,
    refresh: i32,
}

#[derive(Debug)]
struct NewOutput {
    name: String,
    enabled: bool,
    description: String,
    current_mode: Option<Mode>,
    preferred_mode: Option<Mode>,
    modes: Vec<Mode>,
    position: Option<Position>,
}

trait Ipc {
    fn get_outputs(&mut self) -> Result<Vec<Output>>;
    fn activate_output(&mut self, output: &Output, new_position: Option<Position>) -> Result<()>;
    fn disable_output(&mut self, output: &Output) -> Result<()>;
}

#[derive(Debug)]
struct SwayIPC {
    connection: swayipc::Connection,
}

impl SwayIPC {
    fn new() -> Result<Self> {
        Ok(Self {
            connection: swayipc::Connection::new()?,
        })
    }

    fn get_preferred_resolution(output: &swayipc::Output) -> Resolution {
        output
            .modes
            .first()
            .map(|mode| Resolution {
                width: mode.width,
                height: mode.height,
            })
            .unwrap_or(Resolution {
                width: output.rect.width,
                height: output.rect.height,
            })
    }
}

impl Ipc for SwayIPC {
    fn get_outputs(&mut self) -> Result<Vec<Output>> {
        let mut outputs = self.connection.get_outputs()?;
        outputs.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(outputs
            .into_iter()
            .map(|output| {
                let preferred_resolution = Self::get_preferred_resolution(&output);
                Output {
                    name: output.name,
                    description: format!("{} {}", output.make, output.model),
                    current_resolution: Resolution {
                        width: output.rect.width,
                        height: output.rect.height,
                    },
                    preferred_resolution,
                    position: Position {
                        x: output.rect.x,
                        y: output.rect.y,
                    },
                }
            })
            .collect())
    }

    fn activate_output(&mut self, output: &Output, new_position: Option<Position>) -> Result<()> {
        let position = new_position.as_ref().unwrap_or(&output.position);
        self.connection.run_command(format!(
            "output {} enable pos {} {}",
            output.name, position.x, position.y
        ))?;
        Ok(())
    }

    fn disable_output(&mut self, output: &Output) -> Result<()> {
        self.connection
            .run_command(format!("output {} disable", output.name))?;
        Ok(())
    }
}

fn find_ipc() -> Result<Box<dyn Ipc>> {
    if let Ok(_) = std::env::var("SWAYSOCK") {
        Ok(Box::new(SwayIPC::new()?))
    } else if let Ok(_) = std::env::var("HYPRLAND_INSTANCE_SIGNATURE") {
        Ok(Box::new(HyprlandIpc::new()))
    } else {
        Err(anyhow::anyhow!("Couldn't find compositor. Make sure either SWAYSOCK or HYPRLAND_INSTANCE_SIGNATURE is set."))
    }
}

struct HyprlandIpc {}

impl HyprlandIpc {
    fn new() -> Self {
        HyprlandIpc {}
    }
}

impl Ipc for HyprlandIpc {
    fn get_outputs(&mut self) -> Result<Vec<Output>> {
        let monitors = hyprland::data::Monitors::get()?;
        Ok(monitors
            .into_iter()
            .map(|monitor| Output {
                name: monitor.name,
                description: monitor.description,
                current_resolution: Resolution {
                    height: monitor.height as i32,
                    width: monitor.width as i32,
                },
                preferred_resolution: Resolution {
                    height: monitor.height as i32,
                    width: monitor.width as i32,
                },
                position: Position {
                    x: monitor.x,
                    y: monitor.y,
                },
            })
            .collect())
    }

    fn activate_output(&mut self, output: &Output, new_position: Option<Position>) -> Result<()> {
        todo!()
    }

    fn disable_output(&mut self, output: &Output) -> Result<()> {
        todo!()
    }
}

#[derive(Debug)]
struct State {
    running: bool,
    outputs: HashMap<ObjectId, NewOutput, BuildHasherDefault<FxHasher>>,
    capabilities: Vec<String>,
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: <wl_registry::WlRegistry as wayland_client::Proxy>::Event,
        data: &(),
        conn: &wayland_client::Connection,
        qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            state.capabilities.push(interface.clone());
            if interface == "zwlr_output_manager_v1" {
                info!("Binding output events.");
                registry.bind::<ZwlrOutputManagerV1, _, _>(name, 1, qhandle, ());
            }
        }
    }
}

impl Dispatch<ZwlrOutputManagerV1, ()> for State {
    fn event(
        state: &mut Self,
        proxy: &ZwlrOutputManagerV1,
        event: <ZwlrOutputManagerV1 as Proxy>::Event,
        data: &(),
        conn: &wayland_client::Connection,
        qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        use wayland_protocols_wlr::output_management::v1::client::zwlr_output_manager_v1::Event;
        if let Event::Head { head } = event {
            info!("Output manager found head {:?}.", head);
            state.outputs.insert(
                head.id(),
                NewOutput {
                    name: "unknown".into(),
                    description: String::new(),
                    position: None,
                    modes: Vec::new(),
                    enabled: false,
                    current_mode: None,
                    preferred_mode: None,
                },
            );
        } else if let Event::Done { serial } = event {
            trace!("Output manager done. {}", serial);
            state.running = false;
        } else {
            warn!("Output manager ignored {:?}", event);
        }
    }
    event_created_child!(State, ZwlrOutputManagerV1, [
        EVT_HEAD_OPCODE=> (ZwlrOutputHeadV1, ()),
    ]);
}

impl Dispatch<ZwlrOutputHeadV1, ()> for State {
    fn event(
        state: &mut Self,
        proxy: &ZwlrOutputHeadV1,
        event: <ZwlrOutputHeadV1 as Proxy>::Event,
        data: &(),
        conn: &wayland_client::Connection,
        qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        use wayland_protocols_wlr::output_management::v1::client::zwlr_output_head_v1::Event;
        if let Event::Name { name } = event {
            let new_output = state.outputs.get_mut(&proxy.id()).map(|output| {
                output.name = name;
            });
            if new_output.is_none() {
                warn!("Unknow head {:?}", proxy.id());
            }
        } else if let Event::Enabled { enabled } = event {
            let new_output = state.outputs.get_mut(&proxy.id()).map(|output| {
                output.enabled = enabled == 1;
            });
            if new_output.is_none() {
                warn!("Unknow head {:?}", proxy.id());
            }
        } else if let Event::Description { description } = event {
            let new_output = state.outputs.get_mut(&proxy.id()).map(|output| {
                output.description = description;
            });
            if new_output.is_none() {
                warn!("Unknow head {:?}", proxy.id());
            }
        } else {
            debug!("Output head ignoring event {:?}", event);
        }
    }
    event_created_child!(State, ZwlrOutputManagerV1, [
        3 => (ZwlrOutputModeV1, ()),
    ]);
}

impl Dispatch<ZwlrOutputModeV1, ()> for State {
    fn event(
        state: &mut Self,
        proxy: &ZwlrOutputModeV1,
        event: <ZwlrOutputModeV1 as Proxy>::Event,
        data: &(),
        conn: &wayland_client::Connection,
        qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        debug!("Mode ignoring event {:?}", event);
    }
}

type OutputHashMap = FxHashMap<ObjectId, NewOutput>;

impl fmt::Display for NewOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.enabled {
            write!(f, "*")?;
        } else {
            write!(f, " ")?;
        }
        write!(f, "{} ", self.name)?;
        write!(f, "[{}]", self.description)?;
        Ok(())
    }
}

fn main() -> Result<()> {
    let connection = wayland_client::Connection::connect_to_env()?;
    // get outputs from connection
    let display = connection.display();
    let mut q = connection.new_event_queue::<State>();
    let qh = q.handle();
    let _registry = display.get_registry(&qh, ());
    tracing::subscriber::set_global_default(
        tracing_subscriber::FmtSubscriber::builder()
            .with_max_level(tracing::Level::DEBUG)
            .finish(),
    )?;

    let mut state = State {
        running: true,
        outputs: OutputHashMap::default(),
        capabilities: Vec::new(),
    };
    while state.running {
        q.blocking_dispatch(&mut state)?;
    }

    trace!(
        "Server has following unused capabilities: {:?}",
        state.capabilities
    );

    info!("Found {} outputs.", state.outputs.len());
    for (i, (_id, output)) in state.outputs.iter().enumerate() {
        println!("{}: {}", i, output);
    }

    Ok(())
}

fn old_main() -> Result<()> {
    let args = std::env::args().collect::<Vec<String>>();
    let setup = parse_setup(args.into_iter().skip(1).collect())?;

    let mut ipc = find_ipc()?;
    let outputs = ipc.get_outputs()?;

    println!("Recognised screens:");
    for (i, output) in outputs.iter().enumerate() {
        println!(
            "{}: {} (current {}×{}) (preferred {}×{}) [{}]",
            i,
            output.name,
            output.current_resolution.width,
            output.current_resolution.height,
            output.preferred_resolution.width,
            output.preferred_resolution.height,
            output.description,
        );
    }

    if setup.len() > outputs.len() {
        return Err(anyhow::anyhow!(
            "Can't set {} outputs when we only have {}.",
            setup.len(),
            outputs.len()
        ));
    }

    if !setup.is_empty() {
        for (i, output) in outputs.iter().enumerate() {
            if !setup.contains(&i) {
                println!("Disabling {}.", output.name);
                ipc.disable_output(output)?;
            }
        }
    }

    let mut x: i32 = 0;
    for screen in setup.iter() {
        let output = &outputs[*screen];
        ipc.activate_output(output, Some(Position { x, y: 0 }))?;
        println!(
            "{}: Setting output {} with rect {}x{} to {}.",
            screen,
            output.name,
            output.preferred_resolution.width,
            output.preferred_resolution.height,
            x
        );
        x += output.preferred_resolution.width;
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse_setup() {
        let args = vec![String::from("012")];
        let result = parse_setup(args).unwrap();
        assert_eq!(result, &[0, 1, 2]);
    }
}
