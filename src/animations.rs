use crate::IndicatorConfig;
use std::ops::ShlAssign;
use std::thread;
use std::time::Duration;

pub struct Animation {
    frames: Vec<xcb::x::Arc>,
}

impl Animation {
    pub fn circles(size: u32) -> Animation {
        let frames = (10..200)
            .map(|s| {
                let x = 5 + ((size as i16) - 10) / 2 - (s as i16) / 2;
                let y = 5 + ((size as i16) - 10) / 2 - (s as i16) / 2;

                println!("x: {}, y: {}, w: {}, h: {}", x, y, s, s);

                xcb::x::Arc {
                    x,
                    y,
                    width: s,
                    height: s,
                    angle1: 0, // start angle
                    angle2: 360 << 6, // end angle
                }
            })
            .collect::<Vec<xcb::x::Arc>>();
        Animation {
            frames,
        }
    }
    pub fn play(
        &self,
        conn: &xcb::Connection,
        win: xcb::x::Window,
        gfx_ctx: xcb::x::Gcontext,
        speed: f64
    ) {
        let c = xcb::x::Gc::Foreground(((speed / 0.8).max(0.0).min(255.0) as u32) << 16);
        let thick = xcb::x::Gc::LineWidth((speed / 25.0).max(5.0).min(200.0) as _);
        let s = ((speed / 5.0).max(0.0) as usize).min(self.frames.len() - 1);
        conn.send_request(
            &(xcb::x::ClearArea {
                exposures: true,
                window: win,
                x: 0,
                y: 0,
                width: 500,
                height: 500,
            })
        );
        conn.send_request(
            &(xcb::x::ChangeGc {
                gc: gfx_ctx,
                value_list: &[c, thick],
            })
        );
        conn.send_request(
            &(xcb::x::PolyArc {
                drawable: xcb::x::Drawable::Window(win),
                gc: gfx_ctx,
                arcs: &[*self.frames.get(s).unwrap()],
            })
        );
    }
}
