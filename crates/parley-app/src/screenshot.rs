//! `--screenshot FILE`: saves the window as a PPM image a few seconds after start, then quits.
//! For development: to see a change in the running app without a screenshot tool.

use std::time::{Duration, Instant};

use eframe::egui::{self, ScreenshotCallback, ViewportCommand};

const DELAY: Duration = Duration::from_secs(6);

#[derive(Debug)]
pub struct Pending {
    path: Option<String>,
    started: Instant,
}

impl Pending {
    pub fn new(path: String) -> Self {
        Self {
            path: Some(path),
            started: Instant::now(),
        }
    }

    pub fn update(&mut self, ctx: &egui::Context) {
        if self.started.elapsed() < DELAY {
            ctx.request_repaint_after(Duration::from_millis(200));
            return;
        }
        let Some(path) = self.path.take() else { return };
        let close = ctx.clone();
        let callback = ScreenshotCallback::new(move |image| {
            let mut ppm = format!("P6\n{} {}\n255\n", image.width(), image.height()).into_bytes();
            ppm.extend(image.pixels.iter().flat_map(|p| [p.r(), p.g(), p.b()]));
            match std::fs::write(&path, ppm) {
                Ok(()) => println!("parley: saved the window to {path}"),
                Err(e) => eprintln!("parley: cannot write {path}: {e}"),
            }
            close.send_viewport_cmd(ViewportCommand::Close);
            close.request_repaint();
        });
        ctx.send_viewport_cmd(ViewportCommand::Screenshot(callback));
    }
}
