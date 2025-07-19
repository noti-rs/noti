use log::{debug, error};
use render::types::RectSize;
use std::{
    collections::HashMap,
    fs::File,
    hash::Hash,
    os::{
        fd::{AsFd, BorrowedFd},
        unix::fs::FileExt,
    },
};
use wayland_client::{
    delegate_noop,
    protocol::{wl_buffer::WlBuffer, wl_shm::WlShm, wl_shm_pool::WlShmPool},
    Connection, Dispatch, EventQueue,
};

use crate::dispatcher::Dispatcher;

/// The dual-buffer for storing and using for surface. It will help manage buffers, when one uses
/// in surface, the other must be used for writing data and flit them.
pub(super) struct DualSlotedBuffer<T> {
    buffers: [OwnedSlotedBuffer<T>; 2],
    current: usize,
}

impl<T> DualSlotedBuffer<T> {
    pub(super) fn init<P>(
        protocols: &P,
        wayland_connection: &Connection,
        rect_size: &RectSize<usize>,
    ) -> Self
    where
        P: AsRef<WlShm>,
    {
        Self {
            buffers: [
                OwnedSlotedBuffer::init(protocols, wayland_connection, rect_size),
                OwnedSlotedBuffer::init(protocols, wayland_connection, rect_size),
            ],
            current: 0,
        }
    }

    pub(super) fn flip(&mut self) {
        self.current = 1 - self.current;
    }

    pub(super) fn current(&self) -> &OwnedSlotedBuffer<T> {
        &self.buffers[self.current]
    }

    pub(super) fn current_mut(&mut self) -> &mut OwnedSlotedBuffer<T> {
        &mut self.buffers[self.current]
    }

    pub(super) fn other(&self) -> &OwnedSlotedBuffer<T> {
        &self.buffers[1 - self.current]
    }
}

impl<T> Dispatcher for DualSlotedBuffer<T> {
    type State = BufferState;

    fn get_event_queue_and_state(
        &mut self,
    ) -> Option<(&mut EventQueue<Self::State>, &mut Self::State)> {
        None
    }

    fn dispatch(&mut self) -> anyhow::Result<bool> {
        let mut is_dispatched = false;

        for buffer in &mut self.buffers {
            is_dispatched |= buffer.dispatch()?;
        }

        Ok(is_dispatched)
    }
}

/// The owner of sloted buffer in wayland. It handles the drop and the data will be released.
pub(super) struct OwnedSlotedBuffer<T> {
    event_queue: EventQueue<BufferState>,
    state: BufferState,
    sloted_buffer: SlotedBuffer<T>,
}

pub(super) struct BufferState {
    wl_shm_pool: WlShmPool,
    wl_buffer: WlBuffer,
    busy: bool,
}

impl<T> OwnedSlotedBuffer<T> {
    fn init<P>(protocols: &P, wayland_connection: &Connection, rect_size: &RectSize<usize>) -> Self
    where
        P: AsRef<WlShm>,
    {
        let event_queue = wayland_connection.new_event_queue();

        let size = rect_size.area() * 4;
        let mut sloted_buffer = SlotedBuffer::new();
        sloted_buffer.push_without_slot(&vec![0; size]);

        let wl_shm_pool = <P as AsRef<WlShm>>::as_ref(protocols).create_pool(
            sloted_buffer.buffer.as_fd(),
            size as i32,
            &event_queue.handle(),
            (),
        );

        let wl_buffer = wl_shm_pool.create_buffer(
            0,
            rect_size.width as i32,
            rect_size.height as i32,
            rect_size.width as i32 * 4,
            wayland_client::protocol::wl_shm::Format::Argb8888,
            &event_queue.handle(),
            (),
        );

        let state = BufferState {
            wl_shm_pool,
            wl_buffer,
            busy: true,
        };

        Self {
            event_queue,
            state,
            sloted_buffer,
        }
    }

    pub(super) fn build(&mut self, rect_size: &RectSize<usize>) {
        assert!(
            self.sloted_buffer.buffer.size() >= rect_size.area() * 4,
            "Buffer size must be greater or equal to window size. Buffer size: {}. Window area: {}.",
            self.sloted_buffer.buffer.size(),
            rect_size.area() * 4
        );

        //INFO: The Buffer size only growth and it guarantee that shm_pool never shrinks
        self.state
            .wl_shm_pool
            .resize(self.sloted_buffer.buffer.size() as i32);

        self.state.wl_buffer.destroy();
        self.state.wl_buffer = self.state.wl_shm_pool.create_buffer(
            0,
            rect_size.width as i32,
            rect_size.height as i32,
            rect_size.width as i32 * 4,
            wayland_client::protocol::wl_shm::Format::Argb8888,
            &self.event_queue.handle(),
            (),
        );
    }

    pub(super) fn wl_buffer(&self) -> &WlBuffer {
        &self.state.wl_buffer
    }
}

impl<T> Drop for OwnedSlotedBuffer<T> {
    fn drop(&mut self) {
        self.state.wl_buffer.destroy();
        self.state.wl_shm_pool.destroy();

        if let Err(_err) = self.event_queue.roundtrip(&mut self.state) {
            error!("OwnedSlotedBuffer: Failed to sync during deinitialization.")
        };
    }
}

impl<T> std::ops::Deref for OwnedSlotedBuffer<T> {
    type Target = SlotedBuffer<T>;

    fn deref(&self) -> &Self::Target {
        &self.sloted_buffer
    }
}

impl<T> std::ops::DerefMut for OwnedSlotedBuffer<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.sloted_buffer
    }
}

impl<T> Dispatcher for OwnedSlotedBuffer<T> {
    type State = BufferState;

    fn get_event_queue_and_state(
        &mut self,
    ) -> Option<(&mut EventQueue<Self::State>, &mut Self::State)> {
        Some((&mut self.event_queue, &mut self.state))
    }
}

delegate_noop!(BufferState: ignore WlShmPool);

impl Dispatch<WlBuffer, ()> for BufferState {
    fn event(
        state: &mut Self,
        _proxy: &WlBuffer,
        event: <WlBuffer as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        if let wayland_client::protocol::wl_buffer::Event::Release = event {
            state.busy = false
        }
    }
}

/// The wrapper for Buffer which can deal with Slots. It allows to store data by key for reading
/// later. Also SlotedBuffer allows write data without key if it's useless to read later.
pub(super) struct SlotedBuffer<T> {
    pub(super) buffer: Buffer,
    slots: HashMap<T, Slot>,
}

struct Slot {
    offset: usize,
    len: usize,
}

impl<T> SlotedBuffer<T> {
    pub(super) fn new() -> Self {
        debug!("SlotedBuffer: Trying to create");
        let sb = Self {
            buffer: Buffer::new(),
            slots: HashMap::new(),
        };
        debug!("SlotedBuffer: Created!");
        sb
    }

    /// Clears the data of buffers and slots. But the size of buffer remains.
    pub(super) fn reset(&mut self) {
        self.buffer.reset();
        self.slots.clear();
        debug!("SlotedBuffer: Reset")
    }

    /// Pushes the data with creating a slot into buffer.
    pub(super) fn push(&mut self, key: T, data: &[u8])
    where
        T: Hash + Eq,
    {
        self.slots.insert(
            key,
            Slot {
                offset: self.buffer.filled_size(),
                len: data.len(),
            },
        );
        self.buffer.push(data);
        debug!("SlotedBuffer: Received data to create Slot")
    }

    /// Pushes the data wihtout creating a slot into buffer.
    pub(super) fn push_without_slot(&mut self, data: &[u8]) {
        self.buffer.push(data);
        debug!("SlotedBuffer: Received data to write without slot.")
    }

    /// Retrieves a copy of data using specific slot by key.
    pub(super) fn data_by_slot(&self, key: &T) -> Option<Vec<u8>>
    where
        T: Hash + Eq,
    {
        self.slots.get(key).and_then(|slot| {
            let mut buffer = vec![0; slot.len];

            if let Err(err) = self.buffer.file.read_at(&mut buffer, slot.offset as u64) {
                error!("SlotedBuffer: Failed to read slot from buffer. The error: {err}");
                return None;
            }

            Some(buffer)
        })
    }
}

pub(super) struct Buffer {
    file: File,
    cursor: u64,
    size: usize,
}

impl Buffer {
    fn new() -> Self {
        debug!("Buffer: Trying to create");
        Self {
            file: tempfile::tempfile().expect("The tempfile must be created"),
            cursor: 0,
            size: 0,
        }
    }

    fn reset(&mut self) {
        self.file
            .write_all_at(&vec![0; self.cursor as usize + 1], 0)
            .expect("Must be possibility to write into file");
        self.cursor = 0;
        debug!("Buffer: Reset");
    }

    fn push(&mut self, data: &[u8]) {
        self.file
            .write_all_at(data, self.cursor)
            .expect("Must be possibility to write into file");
        self.cursor += data.len() as u64;

        self.size = std::cmp::max(self.size, self.cursor as usize);

        debug!("Buffer: Received a data to write")
    }

    fn filled_size(&self) -> usize {
        self.cursor as usize
    }

    pub(super) fn size(&self) -> usize {
        self.size
    }

    pub(super) fn as_fd(&self) -> BorrowedFd<'_> {
        self.file.as_fd()
    }
}
