use eframe::Frame;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
use wayland_backend::client::{Backend, ObjectId};
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols::xdg::activation::v1::client::xdg_activation_v1::XdgActivationV1;

struct State;

impl Dispatch<WlRegistry, GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: <WlRegistry as Proxy>::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<XdgActivationV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &XdgActivationV1,
        _: <XdgActivationV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

pub fn is_wayland(frame: &Frame) -> bool {
    frame
        .display_handle()
        .is_ok_and(|handle| matches!(handle.as_raw(), RawDisplayHandle::Wayland(_)))
}

pub fn activate(frame: &Frame, token: &str) -> Result<(), String> {
    let display = match frame.display_handle().map_err(|e| e.to_string())?.as_raw() {
        RawDisplayHandle::Wayland(display) => display.display,
        _ => return Err("not a Wayland window".into()),
    };
    let surface = match frame.window_handle().map_err(|e| e.to_string())?.as_raw() {
        RawWindowHandle::Wayland(window) => window.surface,
        _ => return Err("not a Wayland window".into()),
    };
    // SAFETY: both pointers come from the live eframe window, which outlives this call.
    let backend = unsafe { Backend::from_foreign_display(display.as_ptr().cast()) };
    let conn = Connection::from_backend(backend);
    let (globals, queue) = registry_queue_init::<State>(&conn).map_err(|e| e.to_string())?;
    let activation: XdgActivationV1 = globals
        .bind(&queue.handle(), 1..=1, ())
        .map_err(|e| e.to_string())?;
    // SAFETY: see above.
    let surface_id = unsafe { ObjectId::from_ptr(WlSurface::interface(), surface.as_ptr().cast()) }
        .map_err(|e| e.to_string())?;
    let surface = WlSurface::from_id(&conn, surface_id).map_err(|e| e.to_string())?;
    activation.activate(token.to_string(), &surface);
    activation.destroy();
    conn.flush().map_err(|e| e.to_string())
}
