use std::sync::RwLock;
use std::thread;
use std::time::Duration;
use serde::{ Deserialize, Serialize };
mod animations;
use animations::Animation;

#[derive(Serialize, Deserialize)]
enum Kind {
    Grow,
    GrowOutline,
    Shrink,
    ShrinkOutline,
}

#[derive(Serialize, Deserialize)]
pub struct IndicatorConfig {
    max_size: u16, // display pixels
    duration: u64, // milliseconds
    thickness: u32, // display pixels
    framerate: u16, // number of circles to display
    color: u32, // color in hex, eg.: 0x00FECA
    animation: Kind, // 'Grow' | 'Shrink' | 'GrowOutline' | 'ShrinkOutline'
}

// sane defaults
impl std::default::Default for IndicatorConfig {
    fn default() -> IndicatorConfig {
        IndicatorConfig {
            max_size: 300u16,
            duration: 500u64,
            thickness: 1,
            framerate: 30,
            color: 0xffffff,
            animation: Kind::Grow,
        }
    }
}

struct OverlayWindow {
    conn: xcb::Connection,
    screen_num: usize,
    win: xcb::x::Window,
    gfx: xcb::x::Gcontext,
    size: u32,
}

impl OverlayWindow {
    pub fn new(conn: xcb::Connection, screen_num: usize) -> OverlayWindow {
        let win = OverlayWindow::create_window(&conn, screen_num);
        let gfx = conn.create_gcontext(win);
        OverlayWindow {
            conn,
            screen_num,
            win,
            gfx,
            size: 300,
        }
    }

    fn create_window(conn: &xcb::Connection, screen_num: usize) -> xcb::x::Window {
        let window_state = conn.atom_(b"ATOM_WM_STATE");
        let window_on_top = conn.atom_(b"ATOM_WM_STATE_STAYS_ON_TOP");

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
                width: 500,
                height: 500,
                border_width: 0,
                class: xcb::x::WindowClass::InputOutput,
                visual: alpha.visual_id(),
                value_list: &[
                    xcb::x::Cw::BackPixel(0x00),
                    xcb::x::Cw::BorderPixel(0x00), // you need this if you use alpha apparently
                    xcb::x::Cw::OverrideRedirect(true), // dont take focus
                    xcb::x::Cw::EventMask(xcb::x::EventMask::EXPOSURE),
                    xcb::x::Cw::Colormap(colormap),
                ],
            })
        );

        conn.send_request(
            &(xcb::x::FreeColormap {
                cmap: colormap,
            })
        );

        conn.send_request(
            &(xcb::x::ChangeProperty {
                mode: xcb::x::PropMode::Replace,
                window: win,
                property: xcb::x::ATOM_WM_NAME,
                r#type: xcb::x::ATOM_STRING,
                data: "hello".as_bytes(),
            })
        );

        conn.send_request(
            &(xcb::x::ChangeProperty {
                mode: xcb::x::PropMode::Append,
                window: win,
                property: window_state,
                r#type: xcb::x::ATOM_ATOM,
                data: &[window_on_top],
            })
        );

        conn.send_request(
            &(xcb::shape::Rectangles {
                operation: xcb::shape::So::Set,
                destination_kind: xcb::shape::Sk::Input,
                destination_window: win,
                x_offset: 0,
                y_offset: 0,
                ordering: xcb::x::ClipOrdering::Unsorted,
                rectangles: &[
                    xcb::x::Rectangle {
                        x: 0,
                        y: 0,
                        width: 0,
                        height: 0,
                    },
                ],
            })
        );

        win
    }

    pub fn move_win_to_cursor(&self, p_x: i32, p_y: i32) {
        let win_x = p_x - (self.size as i32) / 2;
        let win_y = p_y - (self.size as i32) / 2;

        self.conn.send_request(
            &(xcb::x::ConfigureWindow {
                window: self.win,
                value_list: &[xcb::x::ConfigWindow::X(win_x), xcb::x::ConfigWindow::Y(win_y)],
            })
        );
    }

    pub fn get_win(&self) -> xcb::x::Window {
        self.win
    }

    fn show(&self) {
        self.conn.send_request(
            &(xcb::x::MapWindow {
                window: self.win,
            })
        );
    }

    fn hide(&self) {
        self.conn.send_request(
            &(xcb::x::UnmapWindow {
                window: self.win,
            })
        );
    }

    fn get_gfx(&self) -> xcb::x::Gcontext {
        self.gfx
    }

    fn handle_event(&self) {
        let poll_for_queued_event = self.conn.poll_for_queued_event();
        match poll_for_queued_event {
            Ok(Some(xcb::Event::X(xcb::x::Event::Expose(_)))) => {}
            Ok(Some(x)) => println!("event: {:?}", x),
            Ok(None) => {}
            Err(e) => println!("error: {}", e),
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
        visual: &xcb::x::Visualtype
    ) -> xcb::x::Colormap;
    fn get_pointer(&self, win: xcb::x::Window) -> (i16, i16);
    fn atom_(&self, name: &[u8]) -> xcb::x::Atom;
    fn create_gcontext(&self, win: xcb::x::Window) -> xcb::x::Gcontext;
}

impl ConnExt for xcb::Connection {
    fn create_colormap(
        &self,
        screen: &xcb::x::Screen,
        visual: &xcb::x::Visualtype
    ) -> xcb::x::Colormap {
        let colormap = self.generate_id();
        self.send_request(
            &(xcb::x::CreateColormap {
                alloc: xcb::x::ColormapAlloc::None,
                mid: colormap,
                window: screen.root(),
                visual: visual.visual_id(),
            })
        );
        colormap
    }

    fn get_pointer(&self, win: xcb::x::Window) -> (i16, i16) {
        let x = self
            .wait_for_reply(
                self.send_request(
                    &(xcb::x::QueryPointer {
                        window: win,
                    })
                )
            )
            .unwrap();

        (x.root_x(), x.root_y())
    }

    fn atom_(&self, name: &[u8]) -> xcb::x::Atom {
        self.wait_for_reply(
            self.send_request(
                &(xcb::x::InternAtom {
                    only_if_exists: false,
                    name,
                })
            )
        )
            .unwrap()
            .atom()
    }

    fn create_gcontext(&self, win: xcb::x::Window) -> xcb::x::Gcontext {
        let gfx_ctx = self.generate_id();
        self.send_request(
            &(xcb::x::CreateGc {
                cid: gfx_ctx,
                drawable: xcb::x::Drawable::Window(win),
                value_list: &[
                    xcb::x::Gc::Foreground(0x00c00000),
                    xcb::x::Gc::LineWidth(10),
                    xcb::x::Gc::GraphicsExposures(false),
                ],
            })
        );
        gfx_ctx
    }
}

#[derive(Debug, Clone, Copy)]
struct MouseUJpdate {
    velocity: f64,
    time: std::time::Instant,
}

impl MouseUJpdate {
    pub fn new(velocity: f64) -> MouseUJpdate {
        MouseUJpdate {
            velocity,
            time: std::time::Instant::now(),
        }
    }
}

fn main() -> ! {
    let (conn, screen_num) = xcb::Connection::connect(None).unwrap();
    let win = OverlayWindow::new(conn, screen_num as _);
    let animation = Animation::circles(win.size);

    let last_speed = std::sync::Arc::new(RwLock::new(MouseUJpdate::new(0.0)));

    start_motion_thread(std::sync::Arc::clone(&last_speed));

    let mut shown = true;
    let mut avg = 0.0;
    let mut thresh = 0.0;
    let mut thresh2 = 3000.0;
    let mut last_print = std::time::Instant::now();
    loop {
        win.handle_event();
        let mouse_ujpdate = *last_speed.read().unwrap();
        // println!("speed: {}, elapsed: {:?}", mouse_ujpdate.velocity, mouse_ujpdate.time.elapsed());
        let lasts = if mouse_ujpdate.time.elapsed() > Duration::from_millis(1000) {
            0.0
        } else {
            mouse_ujpdate.velocity
        };

        let weight = (avg / 4000.0f64).max(0.02).min(0.08);
        let rweight = 1.0 - weight;
        avg = avg * rweight * 0.999 + lasts * weight;
        // avg = if lasts < 200.0 { avg * 0.9 + lasts * 0.1 } else if avg < lasts && lasts > 1000.0 { avg * 0.8 + lasts * 0.2 } else { avg * 0.99 + lasts * 0.01 };

        if avg > thresh || mouse_ujpdate.velocity > thresh2 {
            thresh = 100.0;
            if !shown {
                println!(
                    "avg: {:6.1}, thresh: {:6.1}, velocity: {:6.1}, weight: {:6.1}, rweight: {:6.1}",
                    avg,
                    thresh,
                    mouse_ujpdate.velocity,
                    weight,
                    rweight
                );
                shown = true;
                win.show();
            }
            animation.play(win.get_conn(), win.get_win(), win.get_gfx(), avg - thresh);
            let (p_x, p_y) = win.get_conn().get_pointer(win.get_win());
            win.move_win_to_cursor(p_x as _, p_y as _);
            thread::sleep(Duration::from_millis(1000 / 90));
        } else if shown {
            thresh = 1800.0;
            shown = false;
            win.hide();
            thread::sleep(Duration::from_millis(1000 / 10));
        }

        win.conn.flush().unwrap();

        // if last_print.elapsed() > Duration::from_millis(200) {
        //     last_print = std::time::Instant::now();
        //     println!(
        //         "avg: {:6.1}, thresh: {:6.1}, velocity: {:6.1}, weight: {:6.1}, rweight: {:6.1}",
        //         avg,
        //         thresh,
        //         mouse_ujpdate.velocity,
        //         weight,
        //         rweight
        //     );
        // }
    }
}

fn start_motion_thread(last_speed: std::sync::Arc<RwLock<MouseUJpdate>>) {
    std::thread::spawn({
        move || {
            let mut enumerate = evdev::enumerate();
            let find = enumerate.find(|(_, device)| device.name().unwrap_or("").contains("Apple"));
            let (_path, mut device) = find.unwrap();
            let mut last_x = 0;
            let mut last_y = 0;
            let mut last = PointerInputEvent {
                x: 0,
                y: 0,
                time: std::time::Instant::now(),
            };
            loop {
                let events = device.fetch_events().unwrap();
                let mut ignore = false;
                for e in events {
                    match (e.event_type(), e.kind(), e.value()) {
                        (
                            evdev::EventType::ABSOLUTE,
                            evdev::InputEventKind::AbsAxis(evdev::AbsoluteAxisType::ABS_MT_SLOT),
                            _num,
                        ) => {
                            ignore = true;
                        }
                        (
                            evdev::EventType::ABSOLUTE,
                            evdev::InputEventKind::AbsAxis(
                                evdev::AbsoluteAxisType::ABS_MT_POSITION_X,
                            ),
                            val,
                        ) => {
                            if ignore {
                                continue;
                            }
                            last_x = val;
                        }
                        (
                            evdev::EventType::ABSOLUTE,
                            evdev::InputEventKind::AbsAxis(evdev::AbsoluteAxisType::ABS_X),
                            val,
                        ) => {
                            if ignore {
                                continue;
                            }
                            last_x = val;
                        }
                        (
                            evdev::EventType::ABSOLUTE,
                            evdev::InputEventKind::AbsAxis(evdev::AbsoluteAxisType::ABS_Y),
                            val,
                        ) => {
                            if ignore {
                                continue;
                            }
                            last_y = val;
                        }
                        (evdev::EventType::SYNCHRONIZATION, _, _) => {
                            if ignore {
                                continue;
                            }
                            let cur = PointerInputEvent {
                                x: last_x,
                                y: last_y,
                                time: std::time::Instant::now(),
                            };
                            let m = cur.velocity(&last);
                            last = cur;

                            if m > 5000.0 {
                                continue;
                            }
                            let mouse_ujpdate = MouseUJpdate::new(m);

                            *last_speed.write().unwrap() = mouse_ujpdate;
                        }
                        _ => {}
                    }
                }
            }
        }
    });
}

struct PointerInputEvent {
    x: i32,
    y: i32,
    time: std::time::Instant,
}

impl PointerInputEvent {
    pub fn velocity(&self, previous: &PointerInputEvent) -> f64 {
        let delta = self.time - previous.time;
        let x = self.x - previous.x;
        let y = self.y - previous.y;
        let delta = delta.as_secs_f64();
        let x = (x as f64) / delta;
        let y = (y as f64) / delta;
        (x * y).abs().sqrt()
    }
}
