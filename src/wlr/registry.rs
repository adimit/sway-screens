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
