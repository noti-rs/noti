use anyhow::bail;
use async_channel::unbounded;
use config::Config;
use dbus::{
    actions::{Action, ClosingReason, Signal},
    notification::Notification,
    server::Server,
};
use derive_builder::Builder;
use dispatcher::Dispatcher;
use error::Error;
use log::{debug, error, info, warn};
use managers::{idle_manager::IdleManager, window_manager::WindowManager};
use scheduler::Scheduler;
use shared::{
    data::{Borrowed, Data},
    file_watcher::FileState,
};
use std::{rc::Rc, time::Duration};
use wayland_client::{
    delegate_noop,
    protocol::{
        wl_compositor::WlCompositor, wl_registry::WlRegistry, wl_seat::WlSeat, wl_shm::WlShm,
    },
    Connection, Dispatch,
};
use wayland_protocols::{
    ext::idle_notify::v1::client::ext_idle_notifier_v1::ExtIdleNotifierV1,
    wp::{
        cursor_shape::v1::client::wp_cursor_shape_manager_v1::WpCursorShapeManagerV1,
        presentation_time::client::wp_presentation::WpPresentation,
    },
};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::ZwlrLayerShellV1;

mod dispatcher;
mod error;
mod managers;
mod scheduler;

pub fn run(config: Config) -> anyhow::Result<()> {
    let mut config = Data::new(config);
    let (sender, receiver) = unbounded();

    let server = async_io::block_on(Server::init(sender))?;
    info!("Backend: Server initialized");

    let mut backend = Backend::init(&config)?;
    info!("Backend: initialized");

    let mut scheduler = Scheduler::new();
    info!("Scheduler: initialized");

    let mut partially_default_config = false;

    loop {
        while let Ok(action) = receiver.try_recv() {
            match action {
                Action::Show(notification) => {
                    backend.create_notification(notification);
                }
                Action::Close(Some(id)) => {
                    backend.close_notification(id);
                }
                Action::Schedule(notification) => {
                    debug!(
                        "Backend: Scheduled notification with id {} for time {}",
                        notification.data.id, notification.time
                    );
                    scheduler.add(notification);
                }
                Action::Close(None) => {
                    warn!("Backend: Received 'Close' action without an id. Ignored");
                }
                Action::CloseAll => {
                    warn!("Backend: Received unsupported 'CloseAll' action. Ignored");
                }
            }
        }

        scheduler
            .pop_due_notifications()
            .into_iter()
            .for_each(|scheduled| {
                let notification_id = scheduled.data.id;
                backend.create_notification(scheduled.data);
                debug!(
                    "Backend: Notification with id {} due for delivery",
                    notification_id
                );
            });

        backend.poll(config.borrow()).handle_error()?;

        match config.check_updates() {
            FileState::Updated => {
                partially_default_config = false;
                config.update();
                backend.update_config(config.borrow()).handle_error()?;
                info!("Renderer: Detected changes of config files and updated")
            }
            FileState::NotFound if !partially_default_config => {
                partially_default_config = true;
                config.update();
                backend.update_config(config.borrow()).handle_error()?;
                info!("The main or imported configuration file is not found, reverting this part to default values.");
            }
            FileState::NotFound | FileState::NothingChanged => (),
        };

        while let Some(signal) = backend.pop_signal() {
            //INFO: ignore this one because it always emits at server
            if let Signal::NotificationClosed {
                reason: ClosingReason::CallCloseNotification,
                ..
            } = &signal
            {
                continue;
            }
            debug_signal(&signal);
            async_io::block_on(server.emit_signal(signal))?;
        }

        // SAFETY: malloc_trim is very safe syscalls and fails only if there is not libc library
        // and can be turned off with remove feature.
        //
        // The main purpose of this allocator is return all memory to OS becasue the application
        // mostly in idle state and it doesn't need any memory at most of time.
        #[cfg(feature = "libc_alloc")]
        unsafe {
            libc::malloc_trim(0);
        }

        if !backend.window_manager.is_window_visible() {
            std::thread::sleep(Duration::from_millis(50));
            std::hint::spin_loop();
        }
    }
}

trait HandleError<T> {
    fn handle_error(self) -> anyhow::Result<T>;
}

impl<T: Default> HandleError<T> for Result<T, Error> {
    fn handle_error(self) -> anyhow::Result<T> {
        match self {
            Ok(val) => Ok(val),
            Err(err) => match err {
                Error::UnrenderedNotifications(_vec) => {
                    //TODO: handle unrenedered banners
                    Ok(Default::default())
                }
                Error::Fatal(error) => Err(error)?,
            },
        }
    }
}

fn debug_signal(signal: &Signal) {
    match signal {
        Signal::ActionInvoked {
            notification_id,
            action_key,
        } => debug!("Action '{action_key}' was invoked for notification id {notification_id}"),
        Signal::NotificationClosed {
            notification_id,
            reason,
        } => debug!("Notification with id {notification_id} closed by {reason} reason"),
    }
}

/// Represents the main state of the `Noti` backend. It stores key objects related to drawing and
/// presentation.
pub(crate) struct Backend {
    wayland_connection: Connection,
    protocols: Protocols,
    window_manager: WindowManager,
    idle_manager: IdleManager,
    gpu_interface: GpuInterface,
}

/// Represents the list of bound Wayland protocols, usable for a wide range of tasks—from window
/// creation to managing complex states.
#[derive(Builder)]
struct Protocols {
    wl_compositor: WlCompositor,
    wl_shm: WlShm,
    wl_seat: WlSeat,
    ext_idle_notifier: ExtIdleNotifierV1,
    zwlr_layer_shell: ZwlrLayerShellV1,
    wp_presentation: WpPresentation,
    wp_cursor_shape_manager: WpCursorShapeManagerV1,
}

/// Represents a collection of objects related to the GPU.
struct GpuInterface {
    egl_state: EglState,
    skia_backend: SkiaBackend,
}

pub type EglInstance = khronos_egl::Instance<khronos_egl::Static>;
pub type EglDisplay = khronos_egl::Display;
pub type EglConfig = khronos_egl::Config;
pub type EglContext = khronos_egl::Context;

/// Represents the state for managing an EGL instance, including display, configuration, and context.
///
/// EGL provides a bridge between OpenGL (or OpenGL ES) and the native windowing system. In Wayland,
/// EGL allows rendering directly to GPU buffers, enabling hardware-accelerated drawing.
#[derive(Clone)]
struct EglState {
    instance: Rc<EglInstance>,
    display: EglDisplay,
    config: EglConfig,
    context: EglContext,
}

/// This application uses Skia as the primary module for drawing complex UI. To enable immediate
/// surface creation, the `GrContext` must be persisted for the lifetime of the application.
struct SkiaBackend {
    /// It's important to have DirectContext because it relies to current available surface.
    /// Without it the DirectContext won't be created and usable.
    no_surface: NoSurface,
    gr_context: skia_safe::gpu::DirectContext,
}

/// Wrapper for `NO_SURFACE` from EGL. This wrapper can be set as the current surface to indicate
/// to the GPU that no surface is active, effectively performing no rendering. It helps ensure
/// correct EGL surface management.
struct NoSurface(pub khronos_egl::Surface);

impl Backend {
    /// Initializes the backend by connecting to Wayland, setting up EGL, and creating a Skia context.
    pub(crate) fn init(config: &Config) -> anyhow::Result<Self> {
        let (wayland_connection, protocols) = Self::connect_to_wayland()?;
        let egl_state = Self::init_egl(&wayland_connection)?;
        let skia_backend = Self::init_skia_backend(&egl_state)?;

        Ok(Self {
            window_manager: WindowManager::init(config)?,
            idle_manager: IdleManager::init(&wayland_connection, &protocols, config)?,
            wayland_connection,
            protocols,
            gpu_interface: GpuInterface {
                egl_state,
                skia_backend,
            },
        })
    }

    /// Connects to Wayland and binds the required protocols. To prevent application deadlock during
    /// connection, a timeout is used.
    ///
    /// This method requires the presence of specific protocols; otherwise, the application will
    /// refuse to start, as it would not know how to interact with Wayland compositor.
    ///
    /// The required protocols are listed in the [Protocols] struct.
    fn connect_to_wayland() -> anyhow::Result<(Connection, Protocols)> {
        const DEFAULT_SETUP_TIME: Duration = Duration::from_secs(10);

        let wayland_connection = Connection::connect_to_env()?;
        let mut event_queue = wayland_connection.new_event_queue();
        let mut backend_state_builder = ProtocolsBuilder::create_empty();

        let display = wayland_connection.display();
        display.get_registry(&event_queue.handle(), ());
        let timer = std::time::Instant::now();

        while backend_state_builder.build().is_err() && timer.elapsed() < DEFAULT_SETUP_TIME {
            event_queue.blocking_dispatch(&mut backend_state_builder)?;
        }

        let protocols = match backend_state_builder.build() {
            Ok(state) => state,
            Err(_) => {
                error!("BackendManager: Failed to init due missing protocols of compositor.");
                bail!("Failed to init noti backend due missing protocols of compositor.")
            }
        };
        Ok((wayland_connection, protocols))
    }

    /// Initializes the EGL context and creates the required components for the application to work.
    ///
    /// This method assumes the application will operate in an RGBA8 color space and that Wayland
    /// provides a way to obtain a `WlEglSurface`, from which an EGL surface will be created.
    fn init_egl(wayland_connection: &Connection) -> anyhow::Result<EglState> {
        let egl_instance = khronos_egl::Instance::new(khronos_egl::Static);
        let egl_display = unsafe {
            egl_instance
                .get_display(wayland_connection.backend().display_ptr() as *mut std::ffi::c_void)
                .expect("The compositor must have the EGL display")
        };
        egl_instance.initialize(egl_display)?;
        egl_instance.bind_api(khronos_egl::OPENGL_ES_API)?;

        let attrib_list = [
            khronos_egl::RED_SIZE,
            8,
            khronos_egl::GREEN_SIZE,
            8,
            khronos_egl::BLUE_SIZE,
            8,
            khronos_egl::ALPHA_SIZE,
            8,
            khronos_egl::DEPTH_SIZE,
            0,
            khronos_egl::STENCIL_SIZE,
            8,
            khronos_egl::SURFACE_TYPE,
            khronos_egl::WINDOW_BIT,
            khronos_egl::RENDERABLE_TYPE,
            khronos_egl::OPENGL_ES2_BIT,
            khronos_egl::NONE,
        ];
        let egl_config = egl_instance
            .choose_first_config(egl_display, &attrib_list)?
            .expect("EGL must have suitable configuration matches to noti");

        let ctx_attributes_es3 = [khronos_egl::CONTEXT_CLIENT_VERSION, 3, khronos_egl::NONE];
        let ctx_attributes_es2 = [khronos_egl::CONTEXT_CLIENT_VERSION, 2, khronos_egl::NONE];

        let mut egl_context =
            egl_instance.create_context(egl_display, egl_config, None, &ctx_attributes_es3);
        if egl_context.is_err() {
            egl_context =
                egl_instance.create_context(egl_display, egl_config, None, &ctx_attributes_es2);
        }
        let egl_context = egl_context?;

        Ok(EglState {
            instance: Rc::new(egl_instance),
            display: egl_display,
            config: egl_config,
            context: egl_context,
        })
    }

    /// Creates a Skia context from the initialized [EglState], which is used to create Skia surfaces.
    fn init_skia_backend(egl_state: &EglState) -> anyhow::Result<SkiaBackend> {
        let no_surface = unsafe { khronos_egl::Surface::from_ptr(khronos_egl::NO_SURFACE) };

        egl_state.instance.make_current(
            egl_state.display,
            Some(no_surface),
            Some(no_surface),
            Some(egl_state.context),
        )?;

        let gl_interface = skia_safe::gpu::gl::Interface::new_load_with(|name| {
            egl_state.instance.get_proc_address(name).unwrap() as *const _
        })
        .unwrap();

        let gr_context = skia_safe::gpu::direct_contexts::make_gl(gl_interface, None).unwrap();

        Ok(SkiaBackend {
            no_surface: NoSurface(no_surface),
            gr_context,
        })
    }

    pub(crate) fn create_notification(&mut self, notification: Box<Notification>) {
        let id = notification.id;
        self.window_manager.create_notification(notification);
        debug!("Backend Manager: Received notification with id {id} to append queue");
    }

    pub(crate) fn close_notification(&mut self, notification_id: u32) {
        self.window_manager.close_notification(notification_id);
        debug!("Backend Manager: Received notification id {notification_id} to close");
    }

    /// Processes and dispatches a single iteration of backend events.
    ///
    /// Unlike a continuous `run` loop, this method performs only one cycle and must be called
    /// repeatedly, typically inside an application loop.
    ///
    /// It handles only events related to the backend.
    pub(crate) fn poll(&mut self, config: Data<Config, Borrowed>) -> Result<(), Error> {
        let Self {
            idle_manager,
            window_manager,
            ..
        } = self;

        if !idle_manager.is_idled() {
            if idle_manager.was_idled() {
                idle_manager.reset_idle_state();

                window_manager.reset_timeouts()?;
            }

            window_manager.show_window(
                &self.wayland_connection,
                &mut self.gpu_interface,
                &self.protocols,
                config.clone(),
            )?;

            window_manager.handle_close_notifications(config.clone())?;
            window_manager.remove_expired(config.clone())?;
            window_manager.handle_actions(config.clone())?;
        }

        window_manager.dispatch()?;
        idle_manager.dispatch()?;

        if window_manager.update_cache() {
            window_manager.update_by_config(config)?;
        }

        Ok(())
    }

    /// Takes a D-Bus signal from the backend that needs to be sent to the application.
    pub(crate) fn pop_signal(&mut self) -> Option<Signal> {
        self.window_manager.pop_signal()
    }

    /// Borrows the user configuration and updates the parts that depend on it.
    pub(crate) fn update_config(&mut self, config: Data<Config, Borrowed>) -> Result<(), Error> {
        let Self {
            window_manager,
            idle_manager,
            ..
        } = self;

        idle_manager.update_by_config(&self.protocols, &config);
        window_manager.update_by_config(config)?;
        window_manager.reset_timeouts()?;
        Ok(())
    }
}

macro_rules! impl_as_ref {
    ($source_type:ty: $field:ident$(.$other_fields:ident)* => $derived_type:ty) => {
        impl AsRef<$derived_type> for $source_type {
            fn as_ref(&self) -> &$derived_type {
                &self.$field
                    $(.$other_fields)*
            }
        }
    };
}

macro_rules! impl_as_mut {
    ($source_type:ty: $field:ident$(.$other_fields:ident)* => $derived_type:ty) => {
        impl AsMut<$derived_type> for $source_type {
            fn as_mut(&mut self) -> &mut $derived_type {
                &mut self.$field
                    $(.$other_fields)*
            }
        }
    };
}

impl_as_ref!(GpuInterface: egl_state => EglState);
impl_as_ref!(GpuInterface: skia_backend.no_surface => NoSurface);
impl_as_ref!(GpuInterface: skia_backend.gr_context => skia_safe::gpu::DirectContext);
impl_as_mut!(GpuInterface: skia_backend.gr_context => skia_safe::gpu::DirectContext);

impl_as_ref!(Protocols: wl_compositor => WlCompositor);
impl_as_ref!(Protocols: wl_shm => WlShm);
impl_as_ref!(Protocols: wl_seat => WlSeat);
impl_as_ref!(Protocols: ext_idle_notifier => ExtIdleNotifierV1);
impl_as_ref!(Protocols: zwlr_layer_shell => ZwlrLayerShellV1);
impl_as_ref!(Protocols: wp_presentation => WpPresentation);
impl_as_ref!(Protocols: wp_cursor_shape_manager => WpCursorShapeManagerV1);

impl Dispatch<WlRegistry, ()> for ProtocolsBuilder {
    fn event(
        state: &mut Self,
        registry: &WlRegistry,
        event: <WlRegistry as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &wayland_client::Connection,
        qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        if let wayland_client::protocol::wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            match interface.as_ref() {
                "wl_compositor" => {
                    state.wl_compositor(registry.bind::<WlCompositor, _, _>(
                        name,
                        version,
                        qhandle,
                        (),
                    ));
                    debug!("Backend: Bound the wl_compositor");
                }
                "wl_shm" => {
                    state.wl_shm(registry.bind::<WlShm, _, _>(name, version, qhandle, ()));
                    debug!("Backend: Bound the wl_shm");
                }
                "wl_seat" => {
                    state.wl_seat(registry.bind::<WlSeat, _, _>(name, version, qhandle, ()));
                    debug!("Backend: Bound the wl_seat");
                }
                "zwlr_layer_shell_v1" => {
                    state.zwlr_layer_shell(registry.bind::<ZwlrLayerShellV1, _, _>(
                        name,
                        version,
                        qhandle,
                        (),
                    ));
                    debug!("Backend: Bound the zwlr_layer_shell_v1");
                }
                "ext_idle_notifier_v1" => {
                    state.ext_idle_notifier(registry.bind::<ExtIdleNotifierV1, _, _>(
                        name,
                        version,
                        qhandle,
                        (),
                    ));
                    debug!("Backend: Bound the ext_idle_notifier_v1");
                }
                "wp_presentation" => {
                    state.wp_presentation(registry.bind::<WpPresentation, _, _>(
                        name,
                        version,
                        qhandle,
                        (),
                    ));
                }
                "wp_cursor_shape_manager_v1" => {
                    state.wp_cursor_shape_manager(registry.bind::<WpCursorShapeManagerV1, _, _>(
                        name,
                        version,
                        qhandle,
                        (),
                    ));

                    debug!("Backend: Bound the wp_cursor_shape_manager_v1");
                }
                _ => (),
            }
        }
    }
}

delegate_noop!(ProtocolsBuilder: ignore WlCompositor);
delegate_noop!(ProtocolsBuilder: ignore ZwlrLayerShellV1);
delegate_noop!(ProtocolsBuilder: ignore WlShm);
delegate_noop!(ProtocolsBuilder: ignore WlSeat);
delegate_noop!(ProtocolsBuilder: ignore WpPresentation);
delegate_noop!(ProtocolsBuilder: ignore WpCursorShapeManagerV1);
delegate_noop!(ProtocolsBuilder: ignore ExtIdleNotifierV1);
