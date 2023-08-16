use std::fmt::{ Formatter, Debug };

#[derive(Debug, Clone)]
pub struct Config {
    pub capture_seconds: f64,
    pub window_size: i32,
    pub device_name: String,
    pub decay: f64,
    pub accel: f64,
    pub accel_decay: f64,
    pub accel_inc: f64,
    pub threshold: f64,
}

impl Config {
    pub fn new() -> Config {
        Config {
            capture_seconds: 5.0,
            window_size: 200,
            decay: 0.98,
            accel: 1500.0f64,
            accel_decay: 0.1,
            accel_inc: 0.3,
            threshold: 1500.0,
            device_name: String::from("Apple"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Position32 {
    pub x: i32,
    pub y: i32,
}

impl Position32 {
    pub fn new(x: i32, y: i32) -> Position32 {
        Position32 { x, y }
    }
}

#[derive(Clone, Copy)]
pub struct VelocityEvent {
    velocity: f64,
    time: std::time::Instant,
}

impl Debug for VelocityEvent {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VelocityEvent").field("velocity", &self.velocity.round()).finish()
    }
}

impl VelocityEvent {
    pub fn new(velocity: f64) -> VelocityEvent {
        VelocityEvent {
            velocity,
            time: std::time::Instant::now(),
        }
    }

    pub fn velocity(&self) -> f64 {
        self.velocity
    }

    pub fn time(&self) -> std::time::Instant {
        self.time
    }

    pub fn expired(&self) -> bool {
        self.time.elapsed() > std::time::Duration::from_millis(250)
    }
}

#[derive(Clone, Copy)]
pub struct PointerInputEvent {
    pub x: i32,
    pub y: i32,
    pub time: std::time::Instant,
}

impl Debug for PointerInputEvent {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PointerInputEvent").field("x", &self.x).field("y", &self.y).finish()
    }
}

impl PointerInputEvent {
    pub fn velocity(&self, previous: &PointerInputEvent) -> f64 {
        let delta = (self.time - previous.time).as_secs_f64();
        let w = ((self.x as f64) - (previous.x as f64)) / delta;
        let h = ((self.y as f64) - (previous.y as f64)) / delta;
        (w * h).abs().sqrt()
    }
}
