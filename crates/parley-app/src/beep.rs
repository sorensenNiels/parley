//! The 5250 alarm's beep (R-UI-25, D37): a short sine tone on the default sound output.

use std::thread;
use std::time::Duration;

use rodio::Source;
use rodio::source::SineWave;

const PITCH: f32 = 880.0;
const LENGTH: Duration = Duration::from_millis(150);
/// Quiet enough not to startle; the alarm often comes with every error.
const VOLUME: f32 = 0.2;

/// Plays the beep on a thread of its own, opening the sound output for it and closing it after.
/// Without a sound output there is no beep and no error: the screen's flash is enough.
pub fn beep() {
    // A failed spawn only loses one beep.
    let _ = thread::Builder::new()
        .name("parley-alarm".into())
        .spawn(|| {
            let Ok(mut sink) = rodio::DeviceSinkBuilder::open_default_sink() else {
                return;
            };
            sink.log_on_drop(false);
            sink.mixer()
                .add(SineWave::new(PITCH).take_duration(LENGTH).amplify(VOLUME));
            // The sink plays only while it lives: keep it a little past the tone.
            thread::sleep(LENGTH + Duration::from_millis(100));
        });
}
