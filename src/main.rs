use anyhow::Result;

use crate::{
    screens::{OutputManager, Position},
    setup::parse_setup,
    wlr::WlrOutputManager,
};

mod wlr {
    use crate::screens::{Mode, Output, OutputManager, Position};
    use anyhow::Result;
    use fxhash::FxHashMap;
    use tracing::{info, trace, warn};
    use wayland_client::backend::ObjectId;

    #[derive(Debug)]
    struct OutputQueryState {
        running: bool,
        outputs: FxHashMap<ObjectId, crate::screens::Output>,
        modes: FxHashMap<ObjectId, Mode>,
        output_to_modes: FxHashMap<ObjectId, Vec<ObjectId>>,
        outputs_current_mode: FxHashMap<ObjectId, ObjectId>,
        capabilities: Vec<String>,
        finalised_output: Vec<Output>,
    }

    impl OutputQueryState {
        fn finalise(&mut self) {
            self.running = false;
            self.finalised_output = self
                .outputs
                .iter()
                .map(|(id, output)| self.finalise_output(id, output))
                .collect();
        }

        fn finalise_output(&self, id: &ObjectId, output: &Output) -> Output {
            let modes = self.find_modes_for_output(id);
            Output {
                name: output.name.clone(),
                enabled: output.enabled,
                description: output.description.clone(),
                current_mode: self.find_current_mode(id),
                preferred_mode: modes.iter().find(|mode| mode.preferred).cloned(),
                modes,
                position: output.position,
                scale: output.scale,
            }
        }

        fn find_current_mode(&self, id: &ObjectId) -> Option<Mode> {
            self.outputs_current_mode
                .get(id)
                .and_then(|mode_id| self.modes.get(mode_id).cloned())
        }

        fn find_modes_for_output(&self, id: &ObjectId) -> Vec<Mode> {
            self.output_to_modes
                .get(id)
                .map(|modes| {
                    modes
                        .iter()
                        .filter_map(|mode_id| self.modes.get(mode_id))
                        .cloned()
                        .collect()
                })
                .unwrap_or_default()
        }
    }

    pub struct WlrOutputManager {
        connection: wayland_client::Connection,
    }

    impl WlrOutputManager {
        pub fn new() -> Result<Self> {
            Ok(Self {
                connection: wayland_client::Connection::connect_to_env()?,
            })
        }
    }

    impl OutputManager for WlrOutputManager {
        fn get_outputs(&self) -> Result<Vec<Output>> {
            let display = self.connection.display();
            let mut q = self.connection.new_event_queue::<OutputQueryState>();
            let qh = q.handle();
            let _registry = display.get_registry(&qh, ());

            let mut state = OutputQueryState {
                running: true,
                outputs: FxHashMap::default(),
                capabilities: Vec::new(),
                output_to_modes: FxHashMap::default(),
                modes: FxHashMap::default(),
                outputs_current_mode: FxHashMap::default(),
                finalised_output: Vec::new(),
            };
            while state.running {
                q.blocking_dispatch(&mut state)?;
            }

            trace!(
                "Server has following unused capabilities: {:?}",
                state.capabilities
            );

            info!("Found {} outputs.", state.finalised_output.len());

            Ok(state.finalised_output)
        }

        fn enable_output(&self, output: &Output, position: &Position) -> Result<()> {
            warn!(
                "NYI: Enabling output {} at position {:?}.",
                output, position
            );
            Ok(())
        }

        fn disable_output(&self, output: &Output) -> Result<()> {
            warn!("NYI: Disabling output {}", output);
            Ok(())
        }
    }

    mod registry {
        use tracing::info;
        use wayland_client::{protocol::wl_registry, Dispatch};
        use wayland_protocols_wlr::output_management::v1::client::zwlr_output_manager_v1::ZwlrOutputManagerV1;

        use super::OutputQueryState;
        impl Dispatch<wl_registry::WlRegistry, ()> for OutputQueryState {
            fn event(
                state: &mut Self,
                registry: &wl_registry::WlRegistry,
                event: <wl_registry::WlRegistry as wayland_client::Proxy>::Event,
                _data: &(),
                _conn: &wayland_client::Connection,
                qhandle: &wayland_client::QueueHandle<Self>,
            ) {
                if let wl_registry::Event::Global {
                    name, interface, ..
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
    }

    mod output_manager {
        use tracing::{info, trace, warn};
        use wayland_client::{event_created_child, Dispatch, Proxy};
        use wayland_protocols_wlr::output_management::v1::client::{
            zwlr_output_head_v1::ZwlrOutputHeadV1,
            zwlr_output_manager_v1::{ZwlrOutputManagerV1, EVT_HEAD_OPCODE},
        };

        use crate::screens::Output;

        use super::OutputQueryState;
        impl Dispatch<ZwlrOutputManagerV1, ()> for OutputQueryState {
            fn event(
                state: &mut Self,
                _proxy: &ZwlrOutputManagerV1,
                event: <ZwlrOutputManagerV1 as Proxy>::Event,
                _data: &(),
                _conn: &wayland_client::Connection,
                _qhandle: &wayland_client::QueueHandle<Self>,
            ) {
                use wayland_protocols_wlr::output_management::v1::client::zwlr_output_manager_v1::Event;
                if let Event::Head { head } = event {
                    info!("Output manager found head {:?}.", head);
                    state.outputs.insert(
                        head.id(),
                        Output {
                            name: "unknown".into(),
                            description: String::new(),
                            position: None,
                            modes: Vec::new(),
                            enabled: false,
                            current_mode: None,
                            preferred_mode: None,
                            scale: 1.0,
                        },
                    );
                    state.output_to_modes.insert(head.id(), Vec::new());
                } else if let Event::Done { serial } = event {
                    trace!("Output manager done. {}", serial);
                    state.finalise();
                } else {
                    warn!("Output manager ignored {:?}", event);
                }
            }

            event_created_child!(OutputQueryState, ZwlrOutputManagerV1, [
                EVT_HEAD_OPCODE => (ZwlrOutputHeadV1, ()),
            ]);
        }
    }

    mod output_head {
        use crate::screens::{Mode, Position, Resolution};

        use super::OutputQueryState;
        use tracing::{debug, warn};
        use wayland_client::{event_created_child, Dispatch, Proxy};
        use wayland_protocols_wlr::output_management::v1::client::{
            zwlr_output_head_v1::{ZwlrOutputHeadV1, EVT_MODE_OPCODE},
            zwlr_output_manager_v1::ZwlrOutputManagerV1,
            zwlr_output_mode_v1::ZwlrOutputModeV1,
        };

        impl Dispatch<ZwlrOutputHeadV1, ()> for OutputQueryState {
            fn event(
                state: &mut Self,
                proxy: &ZwlrOutputHeadV1,
                event: <ZwlrOutputHeadV1 as Proxy>::Event,
                _data: &(),
                _conn: &wayland_client::Connection,
                _qhandle: &wayland_client::QueueHandle<Self>,
            ) {
                use wayland_protocols_wlr::output_management::v1::client::zwlr_output_head_v1::Event;
                if let Event::Name { name } = event {
                    let new_output = state.outputs.get_mut(&proxy.id()).map(|output| {
                        output.name = name;
                    });
                    if new_output.is_none() {
                        warn!("Unknown head {:?}", proxy.id());
                    }
                } else if let Event::Enabled { enabled } = event {
                    let new_output = state.outputs.get_mut(&proxy.id()).map(|output| {
                        output.enabled = enabled == 1;
                    });
                    if new_output.is_none() {
                        warn!("Unknown head {:?}", proxy.id());
                    }
                } else if let Event::Description { description } = event {
                    let new_output = state.outputs.get_mut(&proxy.id()).map(|output| {
                        output.description = description;
                    });
                    if new_output.is_none() {
                        warn!("Unknown head {:?}", proxy.id());
                    }
                } else if let Event::Scale { scale } = event {
                    let new_output = state.outputs.get_mut(&proxy.id()).map(|output| {
                        output.scale = scale;
                    });
                    if new_output.is_none() {
                        warn!("Unknown head {:?}", proxy.id());
                    }
                } else if let Event::Position { x, y } = event {
                    let new_output = state.outputs.get_mut(&proxy.id()).map(|output| {
                        output.position = Some(Position { x, y });
                    });
                    if new_output.is_none() {
                        warn!("Unknown head {:?}", proxy.id());
                    }
                } else if let Event::Mode { mode } = event {
                    state.modes.insert(
                        mode.id(),
                        Mode {
                            resolution: Resolution {
                                width: 0,
                                height: 0,
                            },
                            refresh: 0,
                            preferred: false,
                        },
                    );
                    let new_mode = state.output_to_modes.get_mut(&proxy.id()).map(|modes| {
                        modes.push(mode.id());
                    });
                    if new_mode.is_none() {
                        warn!("Unknown head in mode assignment {:?}", proxy.id());
                    }
                } else if let Event::CurrentMode { mode } = event {
                    state.outputs_current_mode.insert(proxy.id(), mode.id());
                } else {
                    debug!("Output head ignoring event {:?}", event);
                }
            }
            event_created_child!(OutputQueryState, ZwlrOutputManagerV1, [
                EVT_MODE_OPCODE => (ZwlrOutputModeV1, ()),
            ]);
        }
    }

    mod output_mode {
        use tracing::{debug, warn};
        use wayland_client::{Dispatch, Proxy};
        use wayland_protocols_wlr::output_management::v1::client::zwlr_output_mode_v1::ZwlrOutputModeV1;

        use crate::screens::Resolution;

        use super::OutputQueryState;

        impl Dispatch<ZwlrOutputModeV1, ()> for OutputQueryState {
            fn event(
                state: &mut Self,
                proxy: &ZwlrOutputModeV1,
                event: <ZwlrOutputModeV1 as Proxy>::Event,
                _data: &(),
                _conn: &wayland_client::Connection,
                _qhandle: &wayland_client::QueueHandle<Self>,
            ) {
                use wayland_protocols_wlr::output_management::v1::client::zwlr_output_mode_v1::Event;
                if let Event::Size { width, height } = event {
                    let new_mode = state.modes.get_mut(&proxy.id()).map(|mode| {
                        mode.resolution = Resolution { width, height };
                    });
                    if new_mode.is_none() {
                        warn!("Unknown mode {:?}", proxy.id());
                    }
                } else if let Event::Refresh { refresh } = event {
                    let new_mode = state.modes.get_mut(&proxy.id()).map(|mode| {
                        mode.refresh = refresh;
                    });
                    if new_mode.is_none() {
                        warn!("Unknown mode {:?}", proxy.id());
                    }
                } else if let Event::Preferred = event {
                    let new_mode = state
                        .modes
                        .get_mut(&proxy.id())
                        .map(|mode| mode.preferred = true);
                    if new_mode.is_none() {
                        warn!("Unknown mode {:?}", proxy.id());
                    }
                } else {
                    debug!("Mode ignoring event {:?}, {:?}", event, proxy.id());
                }
            }
        }
    }
}

mod screens {
    use anyhow::Result;
    use std::fmt::{self, Debug};

    #[derive(Debug, Clone, Copy)]
    pub struct Position {
        pub x: i32,
        pub y: i32,
    }

    #[derive(Debug, Clone, Copy)]
    pub struct Resolution {
        pub width: i32,
        pub height: i32,
    }

    #[derive(Debug, Copy, Clone)]
    pub struct Mode {
        pub resolution: Resolution,
        pub refresh: i32,
        pub preferred: bool,
    }

    #[derive(Debug)]
    pub struct Output {
        pub name: String,
        pub enabled: bool,
        pub description: String,
        pub current_mode: Option<Mode>,
        pub preferred_mode: Option<Mode>,
        pub modes: Vec<Mode>,
        pub position: Option<Position>,
        pub scale: f64,
    }

    impl fmt::Display for Output {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            use colored::Colorize;
            let indicator = {
                if self.enabled {
                    "⯀ ".bright_green()
                } else {
                    "⮽ ".red()
                }
            };
            write!(f, "{}", indicator)?;
            write!(f, "{}", self.name)?;
            if (self.scale - 1.0).abs() > f64::EPSILON {
                write!(f, " (×{:.2})", self.scale)?;
            }
            if let Some(current_mode) = &self.current_mode {
                write!(f, " {}", current_mode)?;
            }
            if let Some(position) = &self.position {
                if position.x != 0 || position.y != 0 {
                    write!(f, " +{},{}", position.x, position.y)?;
                }
            }
            write!(f, ", {} modes", self.modes.len())?;
            write!(f, " [{}]", self.description)?;
            Ok(())
        }
    }

    impl fmt::Display for Resolution {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}×{}", self.width, self.height)
        }
    }

    impl fmt::Display for Mode {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            use colored::Colorize;
            write!(f, "{}", self.resolution)?;
            if self.refresh != 0 {
                write!(f, "@{:.2}kHz", (self.refresh as f64 / 1000.0))?;
            }
            let heart = if self.preferred {
                "♥".green()
            } else {
                " ".clear()
            };
            write!(f, "{}", heart)?;
            Ok(())
        }
    }

    pub trait OutputManager {
        fn get_outputs(&self) -> Result<Vec<Output>>;
        fn enable_output(&self, output: &Output, position: &Position) -> Result<()>;
        fn disable_output(&self, output: &Output) -> Result<()>;
    }
}

fn main() -> Result<()> {
    tracing::subscriber::set_global_default(
        tracing_subscriber::FmtSubscriber::builder()
            .with_max_level(tracing::Level::WARN)
            .finish(),
    )?;

    let args = std::env::args().collect::<Vec<String>>();
    let setup = parse_setup(args.into_iter().skip(1).collect())?;

    let man = WlrOutputManager::new()?;
    let outputs = man.get_outputs()?;

    println!("Recognised screens:");
    for (i, output) in outputs.iter().enumerate() {
        println!("{}: {}", i, output);
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
                man.disable_output(output)?;
            }
        }
    }

    let mut x: i32 = 0;
    for screen in setup.iter() {
        let output = &outputs[*screen];
        if let Some(mode) = output.preferred_mode.or(output.current_mode) {
            x = mode.resolution.width;
        }
        man.enable_output(output, &Position { x, y: 0 })?;
    }

    Ok(())
}

mod setup {
    use anyhow::Result;
    pub fn parse_setup(arg: Vec<String>) -> Result<Vec<usize>> {
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
}
