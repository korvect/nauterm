//! libdecor owns the xdg role and its native GTK decoration. All callbacks run
//! on the Wayland thread and only enqueue events; GPUI callbacks run afterwards.
use super::window::WaylandWindowStatePtr;
use anyhow::{Context as _, bail};
use calloop::{Interest, LoopHandle, Mode, PostAction, RegistrationToken, generic::Generic};
use gpui::{Pixels, Size, WindowParams};
use std::{
    cell::{Cell, RefCell},
    ffi::{CStr, CString, c_char, c_int, c_void},
    os::fd::BorrowedFd,
    rc::Rc,
};
use wayland_backend::client::ObjectId;
use wayland_client::{Connection, Proxy, protocol::wl_surface::WlSurface};
use wayland_protocols::xdg::shell::client::{xdg_surface::XdgSurface, xdg_toplevel::XdgToplevel};
type Ptr = *mut c_void;
#[repr(C)]
struct ContextInterface {
    error: unsafe extern "C" fn(Ptr, c_int, *const c_char),
    reserved: [Option<unsafe extern "C" fn()>; 10],
}
#[repr(C)]
struct FrameInterface {
    configure: unsafe extern "C" fn(Ptr, Ptr, Ptr),
    close: unsafe extern "C" fn(Ptr, Ptr),
    commit: unsafe extern "C" fn(Ptr, Ptr),
    dismiss_popup: unsafe extern "C" fn(Ptr, *const c_char, Ptr),
    reserved: [Option<unsafe extern "C" fn()>; 10],
}
macro_rules! api {
    ($($name:ident: $ty:ty),* $(,)?) => {
        struct Api { library: Ptr, $($name: $ty,)* }
        impl Api {
            fn load() -> anyhow::Result<Rc<Self>> {
                // SAFETY: fixed soname and public C ABI symbols; the handle outlives all calls.
                unsafe {
                    let library = libc::dlopen(c"libdecor-0.so.0".as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL);
                    if library.is_null() { bail!("Native Wayland titlebars require libdecor-0 and its GTK plugin"); }
                    $(let symbol = libc::dlsym(library, concat!(stringify!($name), "\0").as_ptr().cast());
                      if symbol.is_null() { libc::dlclose(library); bail!("Missing libdecor symbol {}", stringify!($name)); }
                      let $name = std::mem::transmute::<Ptr, $ty>(symbol);)*
                    Ok(Rc::new(Self { library, $($name,)* }))
                }
            }
        }
        impl Drop for Api { fn drop(&mut self) { unsafe { libc::dlclose(self.library); } } }
    }
}
api! {
    libdecor_new: unsafe extern "C" fn(Ptr, *const ContextInterface) -> Ptr,
    libdecor_unref: unsafe extern "C" fn(Ptr),
    libdecor_get_fd: unsafe extern "C" fn(Ptr) -> c_int,
    libdecor_dispatch: unsafe extern "C" fn(Ptr, c_int) -> c_int,
    libdecor_decorate: unsafe extern "C" fn(Ptr, Ptr, *const FrameInterface, Ptr) -> Ptr,
    libdecor_frame_unref: unsafe extern "C" fn(Ptr),
    libdecor_frame_map: unsafe extern "C" fn(Ptr),
    libdecor_frame_get_xdg_surface: unsafe extern "C" fn(Ptr) -> Ptr,
    libdecor_frame_get_xdg_toplevel: unsafe extern "C" fn(Ptr) -> Ptr,
    libdecor_frame_set_title: unsafe extern "C" fn(Ptr, *const c_char),
    libdecor_frame_set_app_id: unsafe extern "C" fn(Ptr, *const c_char),
    libdecor_frame_set_min_content_size: unsafe extern "C" fn(Ptr, c_int, c_int),
    libdecor_frame_set_max_content_size: unsafe extern "C" fn(Ptr, c_int, c_int),
    libdecor_frame_unset_capabilities: unsafe extern "C" fn(Ptr, c_int),
    libdecor_configuration_get_content_size: unsafe extern "C" fn(Ptr, Ptr, *mut c_int, *mut c_int) -> bool,
    libdecor_configuration_get_window_state: unsafe extern "C" fn(Ptr, *mut c_int) -> bool,
    libdecor_state_new: unsafe extern "C" fn(c_int, c_int) -> Ptr,
    libdecor_state_free: unsafe extern "C" fn(Ptr),
    libdecor_frame_commit: unsafe extern "C" fn(Ptr, Ptr, Ptr),
}
#[derive(Clone, Copy)]
pub(super) enum Event {
    Configure { width: i32, height: i32, state: i32 },
    Close,
    Commit,
    DismissPopups,
}
struct CallbackData {
    api: Rc<Api>,
    width: Cell<i32>,
    height: Cell<i32>,
    state: Cell<i32>,
    events: RefCell<Vec<Event>>,
}
unsafe extern "C" fn error(_: Ptr, _: c_int, message: *const c_char) {
    if !message.is_null() {
        log::error!(
            "libdecor: {}",
            unsafe { CStr::from_ptr(message) }.to_string_lossy()
        );
    }
}
unsafe extern "C" fn configure(frame: Ptr, config: Ptr, user: Ptr) {
    // SAFETY: boxed CallbackData remains alive until after frame/context destruction.
    let data = unsafe { &*user.cast::<CallbackData>() };
    let (mut width, mut height, mut state) =
        (data.width.get(), data.height.get(), data.state.get());
    unsafe {
        (data.api.libdecor_configuration_get_content_size)(config, frame, &mut width, &mut height);
        (data.api.libdecor_configuration_get_window_state)(config, &mut state);
        width = width.max(1);
        height = height.max(1);
        let content = (data.api.libdecor_state_new)(width, height);
        if content.is_null() {
            return;
        }
        (data.api.libdecor_frame_commit)(frame, content, config);
        (data.api.libdecor_state_free)(content);
    }
    data.width.set(width);
    data.height.set(height);
    data.state.set(state);
    data.events.borrow_mut().push(Event::Configure {
        width,
        height,
        state,
    });
}
unsafe extern "C" fn close(_: Ptr, user: Ptr) {
    unsafe { &*user.cast::<CallbackData>() }
        .events
        .borrow_mut()
        .push(Event::Close);
}
unsafe extern "C" fn commit(_: Ptr, user: Ptr) {
    unsafe { &*user.cast::<CallbackData>() }
        .events
        .borrow_mut()
        .push(Event::Commit);
}
unsafe extern "C" fn dismiss_popup(_: Ptr, _: *const c_char, user: Ptr) {
    unsafe { &*user.cast::<CallbackData>() }
        .events
        .borrow_mut()
        .push(Event::DismissPopups);
}
static CONTEXT_INTERFACE: ContextInterface = ContextInterface {
    error,
    reserved: [None; 10],
};
static FRAME_INTERFACE: FrameInterface = FrameInterface {
    configure,
    close,
    commit,
    dismiss_popup,
    reserved: [None; 10],
};

pub(super) struct NativeDecoration {
    api: Rc<Api>,
    context: Ptr,
    frame: Ptr,
    data: Box<CallbackData>,
    registration: Option<(LoopHandle<'static, ()>, RegistrationToken)>,
}
impl NativeDecoration {
    pub fn new(
        surface: &WlSurface,
        params: &WindowParams,
    ) -> anyhow::Result<(Self, XdgSurface, XdgToplevel)> {
        let api = Api::load()?;
        let backend = surface
            .backend()
            .upgrade()
            .context("Wayland connection closed")?;
        let mut data = Box::new(CallbackData {
            api: api.clone(),
            width: Cell::new(f32::from(params.bounds.size.width) as i32),
            height: Cell::new(f32::from(params.bounds.size.height) as i32),
            state: Cell::new(0),
            events: RefCell::new(Vec::new()),
        });
        // SAFETY: all pointers originate from this live libwayland connection.
        let context =
            unsafe { (api.libdecor_new)(backend.display_ptr().cast(), &CONTEXT_INTERFACE) };
        if context.is_null() {
            bail!("Cannot create native Wayland decorations");
        }
        let frame = unsafe {
            (api.libdecor_decorate)(
                context,
                surface.id().as_ptr().cast(),
                &FRAME_INTERFACE,
                (&mut *data as *mut CallbackData).cast(),
            )
        };
        let decoration = Self {
            api,
            context,
            frame,
            data,
            registration: None,
        };
        if frame.is_null() {
            bail!("Cannot decorate Wayland window");
        }
        if let Some(title) = params
            .titlebar
            .as_ref()
            .and_then(|title| title.title.as_ref())
        {
            decoration.set_title(title);
        }
        if let Some(id) = params.app_id.as_ref() {
            decoration.set_app_id(id);
        }
        if let Some(min) = params.window_min_size {
            unsafe {
                (decoration.api.libdecor_frame_set_min_content_size)(
                    frame,
                    f32::from(min.width) as i32,
                    f32::from(min.height) as i32,
                );
            }
        }
        if !params.is_movable {
            unsafe {
                (decoration.api.libdecor_frame_unset_capabilities)(frame, 1);
            }
        }
        if !params.is_minimizable {
            unsafe {
                (decoration.api.libdecor_frame_unset_capabilities)(frame, 4);
            }
        }
        if !params.is_resizable {
            unsafe {
                (decoration.api.libdecor_frame_unset_capabilities)(frame, 2);
            }
        }
        unsafe {
            (decoration.api.libdecor_frame_map)(frame);
        }
        // Initial registry/plugin setup owns the xdg role. Bound this startup wait;
        // normal event dispatch is integrated with calloop, without polling timers.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        let (raw_surface, raw_toplevel) = loop {
            let pair = unsafe {
                (
                    (decoration.api.libdecor_frame_get_xdg_surface)(frame),
                    (decoration.api.libdecor_frame_get_xdg_toplevel)(frame),
                )
            };
            if !pair.0.is_null() && !pair.1.is_null() {
                break pair;
            }
            if std::time::Instant::now() >= deadline {
                bail!("Timed out initializing native Wayland titlebar");
            }
            if unsafe { (decoration.api.libdecor_dispatch)(context, 10) } < 0 {
                bail!("Native Wayland decoration initialization failed");
            }
        };
        let connection = Connection::from_backend(backend);
        let xdg_surface =
            unsafe { ObjectId::from_ptr(XdgSurface::interface(), raw_surface.cast()) }?;
        let toplevel =
            unsafe { ObjectId::from_ptr(XdgToplevel::interface(), raw_toplevel.cast()) }?;
        Ok((
            decoration,
            XdgSurface::from_id(&connection, xdg_surface)?,
            XdgToplevel::from_id(&connection, toplevel)?,
        ))
    }
    pub fn attach(
        &mut self,
        window: WaylandWindowStatePtr,
        handle: LoopHandle<'static, ()>,
    ) -> anyhow::Result<()> {
        let fd = unsafe { (self.api.libdecor_get_fd)(self.context) };
        if fd < 0 {
            bail!("libdecor did not provide an event descriptor");
        }
        // Own a duplicate: removing the source must never close libdecor's fd.
        let fd = unsafe { BorrowedFd::borrow_raw(fd) }.try_clone_to_owned()?;
        let weak = window.downgrade();
        let token = handle
            .insert_source(
                Generic::new(fd, Interest::READ, Mode::Level),
                move |_, _, _| {
                    if let Some(window) = weak() {
                        window.dispatch_native_decoration();
                    }
                    Ok(PostAction::Continue)
                },
            )
            .map_err(|error| anyhow::anyhow!("Cannot register libdecor events: {error}"))?;
        self.registration = Some((handle, token));
        Ok(())
    }
    pub fn dispatch(&self) -> Vec<Event> {
        let timing = super::window::window_timing_enabled().then(std::time::Instant::now);
        unsafe {
            (self.api.libdecor_dispatch)(self.context, 0);
        }
        let events = std::mem::take(&mut *self.data.events.borrow_mut());
        if let Some(started) = timing
            && (!events.is_empty() || started.elapsed().as_micros() > 2000)
        {
            eprintln!(
                "[window-timing] phase=libdecor events={} us={}",
                events.len(),
                started.elapsed().as_micros()
            );
        }
        events
    }
    pub fn set_title(&self, title: &str) {
        if let Ok(title) = CString::new(title) {
            unsafe {
                (self.api.libdecor_frame_set_title)(self.frame, title.as_ptr());
            }
        }
    }
    pub fn set_app_id(&self, id: &str) {
        if let Ok(id) = CString::new(id) {
            unsafe {
                (self.api.libdecor_frame_set_app_id)(self.frame, id.as_ptr());
            }
        }
    }
    pub fn set_max_size(&self, max: i32) {
        unsafe {
            (self.api.libdecor_frame_set_max_content_size)(self.frame, max, max);
        }
    }
    pub fn resize(&self, size: Size<Pixels>) {
        let (width, height) = (
            f32::from(size.width).max(1.) as i32,
            f32::from(size.height).max(1.) as i32,
        );
        self.data.width.set(width);
        self.data.height.set(height);
        unsafe {
            let content = (self.api.libdecor_state_new)(width, height);
            if !content.is_null() {
                (self.api.libdecor_frame_commit)(self.frame, content, std::ptr::null_mut());
                (self.api.libdecor_state_free)(content);
            }
        }
    }
}
impl Drop for NativeDecoration {
    fn drop(&mut self) {
        if let Some((handle, token)) = self.registration.take() {
            handle.remove(token);
        }
        unsafe {
            if !self.frame.is_null() {
                (self.api.libdecor_frame_unref)(self.frame);
            }
            (self.api.libdecor_unref)(self.context);
        }
    }
}
