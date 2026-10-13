//! The machine's sounds, cut from a recording of a real typewriter and played through Web Audio.
//!
//! The clips in `web/assets/sounds/` are raw mono PCM (signed 16-bit little-endian at 44.1 kHz):
//! six key clacks, so a run of keys never sounds looped, the space bar, the backspace, the margin
//! bell and the carriage return. They are embedded in the binary, so every cue is ready on the
//! first key press with no fetch or decode.

use std::cell::RefCell;

use web_sys::{AudioBuffer, AudioContext};

/// Which sound to play.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cue {
    /// A typebar striking the platen.
    Clack,
    /// The space bar: a muted thock as the bar lands, then the escapement tick.
    Space,
    /// The carriage stepping back one notch: a soft lever tick, then the escapement pawl.
    Backspace,
    /// The bell near the right margin.
    Bell,
    /// The carriage being thrown back.
    Return,
}

const RATE: f32 = 44_100.0;

/// `include_bytes!` embeds a file's bytes in the binary at compile time, like reading it once
/// at import in Python but with no file on disk at runtime.
const CLACKS: [&[u8]; 6] = [
    include_bytes!("../assets/sounds/clack-0.pcm"),
    include_bytes!("../assets/sounds/clack-1.pcm"),
    include_bytes!("../assets/sounds/clack-2.pcm"),
    include_bytes!("../assets/sounds/clack-3.pcm"),
    include_bytes!("../assets/sounds/clack-4.pcm"),
    include_bytes!("../assets/sounds/clack-5.pcm"),
];
const SPACE: &[u8] = include_bytes!("../assets/sounds/space.pcm");
const BACKSPACE: &[u8] = include_bytes!("../assets/sounds/backspace.pcm");
const BELL: &[u8] = include_bytes!("../assets/sounds/bell.pcm");
const RETURN: &[u8] = include_bytes!("../assets/sounds/return.pcm");

/// Converts signed 16-bit little-endian PCM to Web Audio's floats in -1.0..=1.0.
fn samples(pcm: &[u8]) -> Vec<f32> {
    pcm.chunks_exact(2)
        .map(|pair| f32::from(i16::from_le_bytes([pair[0], pair[1]])) / 32_768.0)
        .collect()
}

struct Bank {
    context: AudioContext,
    clacks: Vec<AudioBuffer>,
    space: AudioBuffer,
    backspace: AudioBuffer,
    bell: AudioBuffer,
    ret: AudioBuffer,
    next_clack: usize,
}

impl Bank {
    fn new() -> Option<Self> {
        let context = AudioContext::new().ok()?;
        let buffer = |pcm: &[u8]| -> Option<AudioBuffer> {
            let data = samples(pcm);
            let buffer = context.create_buffer(1, data.len() as u32, RATE).ok()?;
            buffer.copy_to_channel(&data, 0).ok()?;
            Some(buffer)
        };
        let clacks = CLACKS
            .iter()
            .map(|pcm| buffer(pcm))
            .collect::<Option<Vec<_>>>()?;
        Some(Self {
            space: buffer(SPACE)?,
            backspace: buffer(BACKSPACE)?,
            bell: buffer(BELL)?,
            ret: buffer(RETURN)?,
            clacks,
            next_clack: 0,
            context,
        })
    }

    fn play(&mut self, cue: Cue) {
        let buffer = match cue {
            Cue::Clack => {
                self.next_clack = (self.next_clack + 1) % self.clacks.len();
                &self.clacks[self.next_clack]
            }
            Cue::Space => &self.space,
            Cue::Backspace => &self.backspace,
            Cue::Bell => &self.bell,
            Cue::Return => &self.ret,
        };
        let Ok(source) = self.context.create_buffer_source() else {
            return;
        };
        source.set_buffer(Some(buffer));
        if source
            .connect_with_audio_node(&self.context.destination())
            .is_ok()
        {
            // A sound that fails to start is only a missing click; typing goes on.
            let _ = source.start();
        }
    }
}

thread_local! {
    // WASM is single-threaded; `thread_local!` with `RefCell` is the idiomatic global here.
    static BANK: RefCell<Option<Bank>> = const { RefCell::new(None) };
}

/// Plays a cue. The audio context is created on the first call, which always comes from a key
/// press, so browsers allow it to start.
pub fn play(cue: Cue) {
    BANK.with(|bank| {
        let mut bank = bank.borrow_mut();
        if bank.is_none() {
            *bank = Bank::new();
        }
        if let Some(bank) = bank.as_mut() {
            bank.play(cue);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_decode_little_endian_i16() {
        let pcm = [0x00, 0x00, 0xff, 0x7f, 0x00, 0x80, 0x00, 0x40];

        let decoded = samples(&pcm);

        assert_eq!(decoded, [0.0, 32_767.0 / 32_768.0, -1.0, 0.5]);
    }

    #[test]
    fn clips_start_on_the_sound_and_fade_to_silence() {
        for pcm in CLACKS.into_iter().chain([SPACE, BACKSPACE, BELL, RETURN]) {
            let data = samples(pcm);
            assert_eq!(pcm.len() % 2, 0);
            let peak = data.iter().fold(0.0_f32, |max, s| max.max(s.abs()));
            assert!((0.3..=0.85).contains(&peak), "peak {peak}");
            // Every cue is audible within its first 20 ms, so a key press never lags its sound.
            let onset = &data[..(0.02 * RATE) as usize];
            assert!(onset.iter().any(|s| s.abs() > peak * 0.2));
            assert!(data[data.len() - 100..].iter().all(|s| s.abs() < 0.01));
        }
    }

    #[test]
    fn space_is_softer_than_any_key_strike() {
        let peak = |pcm: &[u8]| samples(pcm).iter().fold(0.0_f32, |max, s| max.max(s.abs()));

        let quietest_clack = CLACKS.into_iter().map(peak).fold(f32::MAX, f32::min);

        // The space bar strikes no paper; it must not sound like a key hit.
        assert!(peak(SPACE) < quietest_clack, "space peak {}", peak(SPACE));
    }

    #[test]
    fn return_knocks_while_the_carriage_lands() {
        let data = samples(RETURN);

        // The carriage slides back in 380 ms (app.css); the stop knock, the loudest sample,
        // must land with it, not seconds later.
        let knock = data
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
            .map(|(index, _)| index as f32 / RATE)
            .expect("the return clip is not empty");
        assert!((0.25..=0.45).contains(&knock), "knock at {knock} s");
        assert!(data.len() as f32 / RATE < 0.6);
    }
}
