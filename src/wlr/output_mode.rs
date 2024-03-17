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
        use wayland_protocols_wlr::output_management::v1::client::zwlr_output_mode_v1::Event::*;
        match event {
            Size { width, height } => {
                let new_mode = state.modes.get_mut(&proxy.id()).map(|mode| {
                    mode.resolution = Resolution { width, height };
                });
                if new_mode.is_none() {
                    warn!("Unknown mode {:?}", proxy.id());
                }
            }
            Refresh { refresh } => {
                let new_mode = state.modes.get_mut(&proxy.id()).map(|mode| {
                    mode.refresh = refresh;
                });
                if new_mode.is_none() {
                    warn!("Unknown mode {:?}", proxy.id());
                }
            }

            _ => debug!("Mode ignoring event {:?}, {:?}", event, proxy.id()),
        }
    }
}
