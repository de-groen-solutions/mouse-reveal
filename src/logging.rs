pub enum LogEvent {
    PointerInput {
        x: i32,
        y: i32,
        time: std::time::Instant,
    },
    Velocity {
        velocity: f64,
        time: std::time::Instant,
    },
    Evdev {
        time: std::time::Instant,
        evdev_event: evdev::InputEvent,
    },
}

pub struct Capture {
    events: Vec<LogEvent>,
}

impl Capture {
    pub fn new() -> Capture {
        Capture { events: Vec::new() }
    }

    pub fn push(&mut self, event: LogEvent) {
        self.events.push(event);
    }

    pub fn events(&self) -> &Vec<LogEvent> {
        &self.events
    }
}

#[derive(Clone)]
pub struct CaptureEmitter {
    start: std::time::Instant,
    expires: std::time::Duration,
    emitter: std::sync::mpsc::Sender<LogEvent>,
}

impl CaptureEmitter {
    pub fn new(
        start: std::time::Instant,
        expires: std::time::Duration,
        emitter: std::sync::mpsc::Sender<LogEvent>
    ) -> CaptureEmitter {
        CaptureEmitter {
            start,
            expires,
            emitter,
        }
    }

    pub fn emit(&self, event: LogEvent) {
        if self.start.elapsed() > self.expires {
            return;
        }
        self.emitter.send(event).unwrap();
    }
}
