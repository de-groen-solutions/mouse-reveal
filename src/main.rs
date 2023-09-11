use animations::Animation;
use models::Config;
use std::fmt::Debug;
use std::sync::mpsc::RecvTimeoutError;
use std::sync::RwLock;
use std::thread;
use std::time::Duration;

mod animations;
mod logging;
mod models;

pub struct ChainContext<Context, V> {
    context: Context,
    result: V,
}

impl<Context, Value> ChainContext<Context, Value>
where
    Context: Sized + Copy,
{
    pub fn result(self) -> Value {
        self.result
    }

    pub fn chain_resultx<F, R>(self, f: F) -> ChainContext<Context, R>
    where
        F: FnOnce(Value) -> R,
    {
        ChainContext {
            context: self.context,
            result: f(self.result),
        }
    }

    pub fn chain_mapx<F, R>(self, f: F) -> R
    where
        F: FnOnce(Value) -> R,
    {
        f(self.result)
    }

    pub fn chain_callx<F, R>(self, f: F) -> ChainContext<Context, R>
    where
        F: FnOnce(Context, Value) -> R,
    {
        ChainContext {
            context: self.context,
            result: f(self.context, self.result),
        }
    }

    pub fn chain_end<F, R>(self, f: F) -> R
    where
        F: FnOnce(Context, Value) -> R,
    {
        f(self.context, self.result)
    }
}

trait PipeFactory<T> {
    fn chain<V>(&self, v: V) -> ChainContext<&Self, V>;
}

impl<T> PipeFactory<T> for T {
    fn chain<V>(&self, v: V) -> ChainContext<&Self, V> {
        ChainContext {
            context: self,
            result: v,
        }
    }
}

struct OverlayWindow {
    conn: xcb::Connection,
    win: xcb::x::Window,
    gfx: xcb::x::Gcontext,
    size: u32,
    visible: bool,
    position: models::Position32,
}

impl Debug for OverlayWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OverlayWindow")
            .field("size", &self.size)
            .field("visible", &self.visible)
            .field("position", &self.position)
            .finish()
    }
}

impl OverlayWindow {
    pub fn new(config: models::Config, conn: xcb::Connection, screen_num: usize) -> OverlayWindow {
        let win = OverlayWindow::create_window(&conn, screen_num, config.window_size as _);
        let gfx = conn.create_gcontext(win);

        OverlayWindow {
            conn,
            win,
            gfx,
            size: config.window_size as _,
            position: models::Position32::new(0, 0),
            visible: false,
        }
    }

    fn create_window(conn: &xcb::Connection, screen_num: usize, size: u32) -> xcb::x::Window {
        let window_state = conn.get_atom(b"ATOM_WM_STATE");
        let window_on_top = conn.get_atom(b"ATOM_WM_STATE_STAYS_ON_TOP");

        let screen = conn.get_setup().roots().nth(screen_num).unwrap();
        let alpha = screen.alpha_visual().unwrap();
        let colormap = conn.create_colormap(screen, &alpha);

        let win: xcb::x::Window = conn.generate_id();
        conn.send_request(
            &(xcb::x::CreateWindow {
                depth: 32,
                wid: win,
                parent: screen.root(),
                x: 0,
                y: 0,
                width: size as u16,
                height: size as u16,
                border_width: 0,
                class: xcb::x::WindowClass::InputOutput,
                visual: alpha.visual_id(),
                value_list: &[
                    xcb::x::Cw::BackPixel(0x00),
                    xcb::x::Cw::BorderPixel(0x00),
                    xcb::x::Cw::OverrideRedirect(true),
                    xcb::x::Cw::EventMask(xcb::x::EventMask::EXPOSURE),
                    xcb::x::Cw::Colormap(colormap),
                ],
            }),
        );

        conn.send_request(&(xcb::x::FreeColormap { cmap: colormap }));

        conn.send_request(
            &(xcb::x::ChangeProperty {
                mode: xcb::x::PropMode::Replace,
                window: win,
                property: xcb::x::ATOM_WM_NAME,
                r#type: xcb::x::ATOM_STRING,
                data: "dgsmousereveal".as_bytes(),
            }),
        );

        conn.send_request(
            &(xcb::x::ChangeProperty {
                mode: xcb::x::PropMode::Append,
                window: win,
                property: window_state,
                r#type: xcb::x::ATOM_ATOM,
                data: &[window_on_top],
            }),
        );

        // Prevent interaction from the mouse with the window,
        // OverrideRedirect did not work, so applying a clip mask instead does the trick.
        conn.send_request(
            &(xcb::shape::Rectangles {
                operation: xcb::shape::So::Set,
                destination_kind: xcb::shape::Sk::Input,
                destination_window: win,
                x_offset: 0,
                y_offset: 0,
                ordering: xcb::x::ClipOrdering::Unsorted,
                rectangles: &[xcb::x::Rectangle {
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 0,
                }],
            }),
        );

        win
    }

    pub fn set_center_position(&mut self, pos: models::Position32) {
        let pos = models::Position32::new(
            pos.x - (self.size as i32) / 2,
            pos.y - (self.size as i32) / 2,
        );

        if self.position == pos {
            return;
        }

        self.conn.send_request(
            &(xcb::x::ConfigureWindow {
                window: self.win,
                value_list: &[
                    xcb::x::ConfigWindow::X(pos.x as _),
                    xcb::x::ConfigWindow::Y(pos.y as _),
                ],
            }),
        );

        self.position = pos;
    }

    pub fn get_win(&self) -> xcb::x::Window {
        self.win
    }

    fn show(&mut self) {
        self.visible = true;
        self.conn
            .send_request(&(xcb::x::MapWindow { window: self.win }));
    }

    fn hide(&mut self) {
        self.visible = false;
        self.conn
            .send_request(&(xcb::x::UnmapWindow { window: self.win }));
    }

    fn get_gfx(&self) -> xcb::x::Gcontext {
        self.gfx
    }

    fn handle_event(&self) {
        match self.conn.poll_for_queued_event() {
            Ok(Some(xcb::Event::X(xcb::x::Event::Expose(_)))) => {}
            Ok(Some(x)) => println!("event: {:?}", x),
            Err(e) => println!("error: {}", e),
            Ok(None) => {
                // No event
            }
        }
    }

    fn get_conn(&self) -> &xcb::Connection {
        &self.conn
    }
}

trait ScreenUtil {
    fn alpha_visual(&self) -> Option<xcb::x::Visualtype>;
}

impl ScreenUtil for xcb::x::Screen {
    fn alpha_visual(&self) -> std::option::Option<xcb::x::Visualtype> {
        let depths = self.allowed_depths();
        let mut alpha_depths = depths.filter(|d| d.depth() == 32u8).peekable();
        if alpha_depths.peek().is_none() {
            panic!("Alpha channel not found!");
        }

        // fetch a visual supporting alpha channels
        alpha_depths.next().unwrap().visuals().get(1_usize).copied()
    }
}

trait ConnExt {
    fn create_colormap(
        &self,
        screen: &xcb::x::Screen,
        visual: &xcb::x::Visualtype,
    ) -> xcb::x::Colormap;
    fn create_gcontext(&self, win: xcb::x::Window) -> xcb::x::Gcontext;
    fn get_pointer(&self, win: xcb::x::Window) -> models::Position32;
    fn get_atom(&self, name: &[u8]) -> xcb::x::Atom;
}

impl ConnExt for xcb::Connection {
    fn create_colormap(
        &self,
        screen: &xcb::x::Screen,
        visual: &xcb::x::Visualtype,
    ) -> xcb::x::Colormap {
        let colormap = self.generate_id();
        self.send_request(
            &(xcb::x::CreateColormap {
                alloc: xcb::x::ColormapAlloc::None,
                mid: colormap,
                window: screen.root(),
                visual: visual.visual_id(),
            }),
        );
        colormap
    }

    fn create_gcontext(&self, win: xcb::x::Window) -> xcb::x::Gcontext {
        let gfx_ctx = self.generate_id();
        let create_gc = xcb::x::CreateGc {
            cid: gfx_ctx,
            drawable: xcb::x::Drawable::Window(win),
            value_list: &([
                    // xcb::x::Gc::GraphicsExposures(false),
                ]),
        };
        self.send_request(&create_gc);
        gfx_ctx
    }

    fn get_pointer(&self, win: xcb::x::Window) -> models::Position32 {
        self.chain(&(xcb::x::QueryPointer { window: win }))
            .chain_callx(Self::send_request)
            .chain_callx(Self::wait_for_reply)
            .chain_resultx(Result::unwrap)
            .chain_mapx(|r| models::Position32::new(r.root_x() as i32, r.root_y() as i32))
    }

    fn get_atom(&self, name: &[u8]) -> xcb::x::Atom {
        let atom = xcb::x::InternAtom {
            only_if_exists: false,
            name,
        };

        self.chain(&atom)
            .chain_callx(Self::send_request)
            .chain_end(Self::wait_for_reply)
            .unwrap()
            .atom()
    }
}

fn main() -> ! {
    let (tx, rx) = std::sync::mpsc::channel();

    let config = models::Config::new();
    let last_velocity_event = std::sync::Arc::new(RwLock::new(models::VelocityEvent::new(0.0)));

    start_capture_thread(config.clone(), rx);

    start_motion_thread(
        config.clone(),
        logging::CaptureEmitter::new(
            std::time::Instant::now(),
            std::time::Duration::from_secs_f64(config.capture_seconds),
            tx.clone(),
        ),
        std::sync::Arc::clone(&last_velocity_event),
    );

    start_ui_loop(
        config.clone(),
        logging::CaptureEmitter::new(
            std::time::Instant::now(),
            std::time::Duration::from_secs_f64(config.capture_seconds),
            tx,
        ),
        last_velocity_event,
    );
}

fn start_capture_thread(config: Config, receiver: std::sync::mpsc::Receiver<logging::LogEvent>) {
    std::thread::spawn(move || {
        let start = std::time::Instant::now();
        let mut capture = logging::Capture::new();
        loop {
            if start.elapsed() > Duration::from_secs(config.capture_seconds as _) {
                println!("Capture finished!");
                break;
            }

            match receiver.recv_timeout(Duration::from_millis(100)) {
                Ok(event) => {
                    capture.push(event);
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => {
                    println!("Receiver disconnected");
                    break;
                }
            }
        }

        capture.events().iter().for_each(|x| match x {
            logging::LogEvent::PointerInput { x, y, time } => {
                println!(
                    "{:1.6}s PointerInputEvent, x: {}, y: {}",
                    (*time - start).as_secs_f64(),
                    x,
                    y
                );
            }
            logging::LogEvent::Velocity { velocity, time } => {
                println!(
                    "{:1.6}s VelocityEvent, velocity: {:1.1}",
                    (*time - start).as_secs_f64(),
                    velocity
                );
            }
            logging::LogEvent::Evdev { time, evdev_event } => {
                println!(
                    "{:1.6}s EvdevEvent, evdev_event: {:?}, value: {}",
                    (*time - start).as_secs_f64(),
                    evdev_event.kind(),
                    evdev_event.value()
                );
            }
        });
    });
}

fn start_ui_loop(
    config: models::Config,
    capture: logging::CaptureEmitter,
    last_velocity_event: std::sync::Arc<RwLock<models::VelocityEvent>>,
) -> ! {
    let (conn, screen_num) = xcb::Connection::connect(None).unwrap();
    let mut win = OverlayWindow::new(config.clone(), conn, screen_num as _);
    let animation = Animation::new(win.size);

    let mut avg_weighted = 0.0;
    let mut avg_ui = 0.0;

    let mut last_render = std::time::Instant::now();
    let mut last_debug = std::time::Instant::now();

    let fps_hidden = Duration::from_millis(1000 / 20);
    let fps_visible = Duration::from_millis(1000 / 120);
    let fps_animation = Duration::from_millis(1000 / 30);

    loop {
        win.handle_event();

        let velocity_event = *last_velocity_event.read().unwrap();
        let velocity = if velocity_event.expired() {
            0.0
        } else {
            velocity_event.velocity()
        };

        avg_weighted = update_avg(config.clone(), avg_weighted, velocity);

        if avg_ui > 50.0 || avg_weighted > config.threshold {
            avg_ui = avg_ui * 0.95 + avg_weighted * 0.05;

            if last_render.elapsed() > fps_animation {
                last_render = std::time::Instant::now();
                animation.play(win.get_conn(), win.get_win(), win.get_gfx(), avg_ui);
            }

            win.show();
            win.set_center_position(win.get_conn().get_pointer(win.get_win()));
            win.conn.flush().unwrap();

            thread::sleep(fps_visible);
        } else {
            avg_ui = 0.0;

            if win.visible {
                win.hide();
                win.conn.flush().unwrap();
            }

            thread::sleep(fps_hidden);
        }

        if last_debug.elapsed() > Duration::from_secs(1) {
            last_debug = std::time::Instant::now();
            println!("{:?}", win);
        }
    }
}

fn update_avg(config: models::Config, avg: f64, velocity: f64) -> f64 {
    let weight_input = (velocity / config.accel)
        .max(config.accel_decay)
        .min(config.accel_inc);
    let weight_state = 1.0 - weight_input;

    avg * weight_state * config.decay + velocity * weight_input
}

fn start_motion_thread(
    config: models::Config,
    capture: logging::CaptureEmitter,
    last_speed: std::sync::Arc<RwLock<models::VelocityEvent>>,
) {
    std::thread::spawn(move || loop {
        thread::sleep(Duration::from_secs(1));

        MotionMonitor::new(
            config.device_name.clone(),
            capture.clone(),
            std::sync::Arc::clone(&last_speed),
        )
        .start_until_error();
    });
}

struct MotionMonitor {
    device_name: String,
    capture: logging::CaptureEmitter,
    last_speed: std::sync::Arc<RwLock<models::VelocityEvent>>,
    last: models::PointerInputEvent,
    working: models::PointerInputEvent,
    ignore_block: bool,
}

impl Debug for MotionMonitor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MotionMonitor")
            .field("device_name", &self.device_name)
            .field("last_speed", &self.last_speed)
            .field("last", &self.last)
            .field("working", &self.working)
            .field("ignore_block", &self.ignore_block)
            .finish()
    }
}

impl MotionMonitor {
    pub fn new(
        device_name: String,
        capture: logging::CaptureEmitter,
        last_speed: std::sync::Arc<RwLock<models::VelocityEvent>>,
    ) -> MotionMonitor {
        MotionMonitor {
            device_name,
            capture,
            last_speed,
            last: models::PointerInputEvent {
                x: 0,
                y: 0,
                time: std::time::Instant::now(),
            },
            working: models::PointerInputEvent {
                x: 0,
                y: 0,
                time: std::time::Instant::now(),
            },
            ignore_block: false,
        }
    }

    fn get_device(&self) -> Option<evdev::Device> {
        evdev::enumerate()
            .find(|(_, device)| {
                device
                    .name()
                    .unwrap_or_default()
                    .contains(self.device_name.as_str())
            })
            .map(|(_, device)| device)
    }

    pub fn start_until_error(&mut self) {
        let device = match self.get_device() {
            Some(device) => device,
            None => {
                println!("No device found!");
                return;
            }
        };

        println!("Device found: {}", device.name().unwrap_or("(unknown)"));

        let result = self.listen_event_loop(device);
        if let Err(e) = result {
            println!("Error while monitoring: {}", e);
        }

        println!("Device disconnected!");
    }

    fn listen_event_loop(&mut self, mut device: evdev::Device) -> Result<(), evdev::Error> {
        println!(
            "Starts monitoring device: {}",
            device.name().unwrap_or("(unknown)")
        );

        let mut last_debug = std::time::Instant::now();
        loop {
            self.ignore_block = false;
            device.fetch_events()?.for_each(|e| self.handle_event(e));

            if last_debug.elapsed() > Duration::from_secs(1) {
                last_debug = std::time::Instant::now();
                println!("{:?}", self);
            }
        }
    }

    fn handle_event(&mut self, input_event: evdev::InputEvent) {
        self.capture.emit(logging::LogEvent::Evdev {
            time: std::time::Instant::now(),
            evdev_event: input_event,
        });
        match (
            input_event.event_type(),
            input_event.kind(),
            input_event.value(),
        ) {
            (
                evdev::EventType::ABSOLUTE,
                evdev::InputEventKind::AbsAxis(evdev::AbsoluteAxisType::ABS_MT_SLOT),
                _num,
            ) => {
                self.ignore_block = true;
            }
            (
                evdev::EventType::ABSOLUTE,
                evdev::InputEventKind::AbsAxis(evdev::AbsoluteAxisType::ABS_X),
                val,
            ) => {
                if self.ignore_block {
                    return;
                }
                self.working.x = val;
            }
            (
                evdev::EventType::ABSOLUTE,
                evdev::InputEventKind::AbsAxis(evdev::AbsoluteAxisType::ABS_Y),
                val,
            ) => {
                if self.ignore_block {
                    return;
                }
                self.working.y = val;
            }
            (evdev::EventType::SYNCHRONIZATION, _, _) => {
                if self.ignore_block {
                    self.ignore_block = false;
                    return;
                }
                self.working.time = std::time::Instant::now();

                let velocity = self.working.velocity(&self.last);
                self.last = self.working;

                if velocity > 5000.0 {
                    // Ignore extreme values
                    return;
                }

                let velocity_event = models::VelocityEvent::new(velocity);
                self.capture.emit(logging::LogEvent::Velocity {
                    velocity: velocity_event.velocity(),
                    time: velocity_event.time(),
                });

                *self.last_speed.write().unwrap() = velocity_event;
            }
            _ => {
                // Other events are ignored
            }
        }
    }
}
