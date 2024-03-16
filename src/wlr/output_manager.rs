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
