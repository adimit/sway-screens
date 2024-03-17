use crate::screens::Resolution;

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
        use wayland_protocols_wlr::output_management::v1::client::zwlr_output_head_v1::Event::*;
        match event {
            Name { name } => {
                let new_output = state.outputs.get_mut(&proxy.id()).map(|output| {
                    output.name = name;
                });
                if new_output.is_none() {
                    warn!("Unknown head {:?}", proxy.id());
                }
            }
            Description { description } => {
                let new_output = state.outputs.get_mut(&proxy.id()).map(|output| {
                    output.description = description;
                });
                if new_output.is_none() {
                    warn!("Unknown head {:?}", proxy.id());
                }
            }
            Scale { scale } => {
                let new_output = state.outputs.get_mut(&proxy.id()).map(|output| {
                    output.scale = scale;
                });
                if new_output.is_none() {
                    warn!("Unknown head {:?}", proxy.id());
                }
            }
            Position { x, y } => {
                let new_output = state.outputs.get_mut(&proxy.id()).map(|output| {
                    output.position = Some(super::Position { x, y });
                });
                if new_output.is_none() {
                    warn!("Unknown head {:?}", proxy.id());
                }
            }
            Mode { mode } => {
                state.modes.insert(
                    mode.id(),
                    super::Mode {
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
            }
            _ => debug!("Output head ignoring event {:?}", event),
        };
    }

    event_created_child!(OutputQueryState, ZwlrOutputManagerV1, [
        EVT_MODE_OPCODE => (ZwlrOutputModeV1, ()),
    ]);
}
