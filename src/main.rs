use std::{fs::File, os::fd::AsFd};

use tempfile::tempfile;
use wayland_client::{
    Connection, Dispatch, QueueHandle, WEnum, delegate_noop,
    protocol::{
        wl_buffer, wl_compositor, wl_keyboard, wl_registry,
        wl_seat::{self},
        wl_shm, wl_shm_pool, wl_surface,
    },
};
use wayland_protocols::xdg::shell::client::{
    xdg_surface,
    xdg_toplevel::{self, XdgToplevel},
    xdg_wm_base,
};

//Application State
//Quoting wayland_client documentation:
//"The core event dispatching logic provided by this crate is built around the EventQueue struct. In this paradigm, receiving and processing events is a two-step process:
//
//  - First, events are read from the Wayland socket. For each event, the backend figures out which EventQueue manages it, and enqueues the event in an internal buffer of that queue.
//  - Then, the EventQueue empties its internal buffer by sequentially invoking the appropriate Dispatch::event() method on the State value that was provided to it.
//
//The main goal of this structure is to make your State accessible without synchronization to most of your event-processing logic, to reduce the plumbing costs.
//
struct AppState {
    running: bool,
    base_surface: Option<wl_surface::WlSurface>,
    buffer: Option<wl_buffer::WlBuffer>,
    wm_base: Option<xdg_wm_base::XdgWmBase>,
    xdg_surface: Option<(xdg_surface::XdgSurface, xdg_toplevel::XdgToplevel)>,
    configured: bool,
}

impl AppState {
    fn init_xdg_surface(&mut self, queue_handle: &QueueHandle<AppState>) {
        //wm_base: Global object that enables clients to turn wl_surfaces into windows
        //in the Desktop Environemnt
        let wm_base = self.wm_base.as_ref().unwrap();

        //base_surface here refers to the wl_surface
        //WlSurfaces are a rectangle area that allows to receive user input, show
        //wl_buffers and have local coordinate systems
        let base_surface = self.base_surface.as_ref().unwrap();

        //XdgSurfaces is an interface that may be implemented by a wl_surface
        //if the implementation needs to provide a desktop-style user interface.
        //
        //Creating an XdgSurface requires you to set up your role-specific object
        //by sending the application info (title, id, size, parent, etc) then
        //performing an initial commit. This initial commit CANNOT have a buffer attached.
        let xdg_surface = wm_base.get_xdg_surface(base_surface, queue_handle, ());
        let toplevel = xdg_surface.get_toplevel(queue_handle, ());

        toplevel.set_title("receba".into());
        toplevel.set_app_id("EstamosAquiDaSilva.org".into());

        base_surface.commit();

        self.xdg_surface = Some((xdg_surface, toplevel));
    }
}

//We need to implement Dispatch<O, _> to each O wayland object that needs to have their events processed.
impl Dispatch<wl_registry::WlRegistry, ()> for AppState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        queue_handle: &QueueHandle<AppState>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            //It is needed to bind the protocols to our registry. So we match the protocols we need
            //and bind them, also requesting their functions.
            //
            //Each protocol event is treated in their own Dispatch impl.
            //
            match &interface[..] {
                "wl_compositor" => {
                    //wl_compositor: the compositor, responsible for creating the displayable
                    //output of multiple surfaces.
                    let compositor = registry.bind::<wl_compositor::WlCompositor, _, _>(
                        name,
                        version,
                        queue_handle,
                        (),
                    );

                    let surface = compositor.create_surface(queue_handle, ());
                    state.base_surface = Some(surface);

                    if state.wm_base.is_some() && state.xdg_surface.is_none() {
                        state.init_xdg_surface(queue_handle);
                    }
                }
                "wl_shm" => {
                    //shm: this singleton provides support for shared memory. Clients are able to
                    //create wl_shm_pools using the create_pool request.
                    let shm = registry.bind::<wl_shm::WlShm, _, _>(name, version, queue_handle, ());

                    let (initial_width, initial_height) = (320, 240);

                    let mut file = tempfile().unwrap();

                    draw(&mut file, (initial_width, initial_height));

                    //wl_shm_pool: this object encapsulates a piece of memory shared between the compositor and
                    //client.
                    //
                    //With wl_shm_pool, the client can allocate shared memory wl_buffer objects.
                    //If you create an object through the same pool it will share the same mapped memory.
                    //As per documentation: "Reusing the mapped memory avoids the setup/teardown overhead and is
                    //useful when: interactively resizing a surface OR when using many small buffers."
                    let pool = shm.create_pool(
                        file.as_fd(),
                        (initial_width * initial_height * 4) as i32,
                        queue_handle,
                        (),
                    );

                    //Quoting documentation: "A buffer provides the content for a wl_surface.
                    //Buffers are created through factory interfaces such as wl_shm, wp_linux_buffer_params
                    //(from the linux-dmabuf protocol extension) or similar. It has a width and a height
                    //and can be attached to a wl_surface, but the mechanism by which a client provides and
                    //updates the contents is defined by the buffer factory interface."
                    let buffer = pool.create_buffer(
                        0,
                        initial_width as i32,
                        initial_height as i32,
                        (initial_width * 4) as i32,
                        wl_shm::Format::Argb8888,
                        queue_handle,
                        (),
                    );

                    state.buffer = Some(buffer.clone());

                    if state.configured {
                        let surface = state.base_surface.as_ref().unwrap();
                        surface.attach(Some(&buffer), 0, 0);
                        surface.commit();
                    }
                }
                "wl_seat" => {
                    //wl_seat: A seat is a greoup of input devices (mouse, keyboard, touch).
                    //Quoting documentation: "A seat is published during start up, or when a device is hot plugged. A seat
                    //typically has a pointer and maintains a keyboard focus and a pointer focus"
                    registry.bind::<wl_seat::WlSeat, _, _>(name, version, queue_handle, ());
                }
                "xdg_wm_base" => {
                    //Quoting documentation: The xdg_wm_base interface is exposed as a global object enabling clients
                    //to turn their wl_surfaces into windows in a desktop environment. It defines the basic functionality
                    //needed for clients and the compositor to create windows that can be dragged, resized, maximized,
                    //etc, as well as creating transient windows such as popup menus.
                    //
                    let wm_base = registry.bind::<xdg_wm_base::XdgWmBase, _, _>(
                        name,
                        version,
                        queue_handle,
                        (),
                    );

                    state.wm_base = Some(wm_base);

                    if state.base_surface.is_some() && state.xdg_surface.is_none() {
                        state.init_xdg_surface(queue_handle);
                    }
                }

                //No need to bind other protocols so we just don't bind them.
                _ => {}
            }
        }
    }
}

impl Dispatch<xdg_wm_base::XdgWmBase, ()> for AppState {
    fn event(
        _: &mut Self,
        wm_base: &xdg_wm_base::XdgWmBase,
        event: xdg_wm_base::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<AppState>,
    ) {
        if let xdg_wm_base::Event::Ping { serial } = event {
            wm_base.pong(serial);
        }
    }
}

impl Dispatch<xdg_surface::XdgSurface, ()> for AppState {
    fn event(
        state: &mut Self,
        surface_xdg: &xdg_surface::XdgSurface,
        event: xdg_surface::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<AppState>,
    ) {
        if let xdg_surface::Event::Configure { serial } = event {
            surface_xdg.ack_configure(serial);
            state.configured = true;

            let base_surface = state.base_surface.as_ref().unwrap();
            if let Some(ref buffer) = state.buffer {
                base_surface.attach(Some(buffer), 0, 0);
                base_surface.commit();
            }
        }
    }
}

impl Dispatch<xdg_toplevel::XdgToplevel, ()> for AppState {
    fn event(
        state: &mut Self,
        _: &XdgToplevel,
        event: xdg_toplevel::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<AppState>,
    ) {
        if let xdg_toplevel::Event::Close = event {
            state.running = false;
        }
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for AppState {
    fn event(
        _: &mut Self,
        seat: &wl_seat::WlSeat,
        event: <wl_seat::WlSeat as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        queue_handle: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities {
            capabilities: WEnum::Value(capabilities),
        } = event
        {
            if capabilities.contains(wl_seat::Capability::Keyboard) {
                seat.get_keyboard(queue_handle, ());
            }
        }
    }
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for AppState {
    fn event(
        state: &mut Self,
        _: &wl_keyboard::WlKeyboard,
        event: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_keyboard::Event::Key {
            serial, time, key, ..
        } = event
        {
            println!("Key {key} did smth!, time: {time}. Serial: {serial}");

            if key == 1 {
                //esc is version
                state.running = false;
            }
        }
    }
}

//Function to draw the image. idk what they doing here idc for now
//TODO: learn this later
fn draw(tmp: &mut File, (buf_x, buf_y): (u32, u32)) {
    use std::{cmp::min, io::Write};
    let mut buf = std::io::BufWriter::new(tmp);
    for y in 0..buf_y {
        for x in 0..buf_x {
            let a = 0xFF;
            let r = min(((buf_x - x) * 0xFF) / buf_x, ((buf_y - y) * 0xFF) / buf_y);
            let g = min((x * 0xFF) / buf_x, ((buf_y - y) * 0xFF) / buf_y);
            let b = min(((buf_x - x) * 0xFF) / buf_x, (y * 0xFF) / buf_y);
            buf.write_all(&[b as u8, g as u8, r as u8, a as u8])
                .unwrap();
        }
    }
    buf.flush().unwrap();
}

//These protocols events are being ignored since we don't care about them in the scope our
//application.
delegate_noop!(AppState: ignore wl_shm::WlShm);
delegate_noop!(AppState: ignore wl_shm_pool::WlShmPool);
delegate_noop!(AppState: ignore wl_buffer::WlBuffer);
delegate_noop!(AppState: ignore wl_compositor::WlCompositor);
delegate_noop!(AppState: ignore wl_surface::WlSurface);

fn main() {
    //Connect to the wayland server through the configuration provided by the environment.
    let connection = Connection::connect_to_env().unwrap();

    //A display is the starting point of any Wayland program.
    //All other objects are created from it.
    let display = connection.display();

    //An event_queue is needed for event processing.
    let mut event_queue = connection.new_event_queue();

    //Its handle is needed to associate objects to the it.
    let queue_handle = event_queue.handle();

    //A registry allows the client to list and bind the global objects
    //available from the compositor.
    //
    //Following the logic, we associate the registry we created to our queue_handle.
    display.get_registry(&queue_handle, ());

    //Create our Application State.
    let mut app_state = AppState {
        running: true,
        base_surface: None,
        buffer: None,
        wm_base: None,
        xdg_surface: None,
        configured: false,
    };

    //Application loop
    while app_state.running {
        //Block waiting for events and dispatch them.
        //Quoting documentation: "This method is similar to dispatch_pending(), but if there are no pending events it will also flush the connection
        //and block waiting for the Wayland server to send an event."
        event_queue.blocking_dispatch(&mut app_state).unwrap();
    }
}
