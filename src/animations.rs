pub struct Animation {
    max_border: u32,
    frames: Vec<xcb::x::Arc>,
}

impl Animation {
    pub fn new(max_size: u32) -> Animation {
        let max_border = max_size / 2 - 16;
        let frames = (0..100)
            .map(|s| {
                circle(
                    max_size,
                    (((max_size - max_border) as f64) * ((s as f64) / 100.0)) as u32,
                )
            })
            .collect::<Vec<xcb::x::Arc>>();

        Animation { max_border, frames }
    }
    pub fn play(
        &self,
        conn: &xcb::Connection,
        win: xcb::x::Window,
        gfx_ctx: xcb::x::Gcontext,
        speed: f64,
    ) {
        let alpha = ((speed / 5.0).max(0.0).min(200.0) as u32) << 24;
        let red = ((speed / 0.8).max(0.0).min(255.0) as u32) << 16;
        let color = xcb::x::Gc::Foreground(red | alpha);
        let border = xcb::x::Gc::LineWidth((speed / 30.0).max(1.0).min(self.max_border as _) as _);
        let frame_idx = ((speed / 10.0).max(0.0) as usize).min(self.frames.len() - 1);
        conn.send_request(
            &(xcb::x::ClearArea {
                exposures: true,
                window: win,
                x: 0,
                y: 0,
                width: 500,
                height: 500,
            }),
        );
        conn.send_request(
            &(xcb::x::ChangeGc {
                gc: gfx_ctx,
                value_list: &[color, border],
            }),
        );
        conn.send_request(
            &(xcb::x::PolyArc {
                drawable: xcb::x::Drawable::Window(win),
                gc: gfx_ctx,
                arcs: &[*self.frames.get(frame_idx).unwrap()],
            }),
        );
    }
}

fn circle(max_size: u32, size: u32) -> xcb::x::Arc {
    let max_size = max_size;
    let x = (max_size as i16) / 2 - (size as i16) / 2;
    let y = (max_size as i16) / 2 - (size as i16) / 2;

    xcb::x::Arc {
        x,
        y,
        width: size as _,
        height: size as _,
        angle1: 0,
        angle2: 360 << 6,
    }
}
