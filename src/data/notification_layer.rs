use smithay_client_toolkit::{
    output::OutputState,
    reexports::client::protocol::wl_pointer,
    registry::RegistryState,
    seat::SeatState,
    shell::wlr_layer::LayerSurface,
    shm::{slot::SlotPool, Shm},
};

pub(crate) struct NotificationLayer {
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    shm: Shm,

    close: bool,
    pool: SlotPool,
    width: u32,
    height: u32,
    layer: LayerSurface,
    pointer: Option<wl_pointer::WlPointer>,

    margin: Margin,
}

pub(crate) struct Margin {
    left: i32,
    right: i32,
    top: i32,
    bottom: i32,
}
