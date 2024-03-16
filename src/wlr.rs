use crate::screens::{Mode, Output, OutputManager, Position};
use anyhow::Result;
use fxhash::FxHashMap;
use tracing::{info, trace, warn};
use wayland_client::backend::ObjectId;

mod output_head;
mod output_manager;
mod output_mode;
mod registry;

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
