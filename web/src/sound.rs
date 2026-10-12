//! The machine's sounds, synthesised in Rust and played through Web Audio.
//!
//! Each sound is computed once into a sample buffer: a key clack (four variants so a run of
//! keys never sounds looped), the space bar thump, the margin bell and the carriage return.

use std::cell::RefCell;

use web_sys::{AudioBuffer, AudioContext};

/// Which sound to play.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cue {
    /// A typebar striking the platen.
    Clack,
    /// The space bar.
    Space,
    /// The bell near the right margin.
    Bell,
    /// The carriage being thrown back.
    Return,
}

const RATE: f32 = 44_100.0;
const CLACKS: usize = 4;

/// Small deterministic noise source (xorshift32), so the synth needs no RNG crate.
struct Noise(u32);

impl Noise {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

/// Renders `seconds` of `at(t)` and scales it so its loudest sample is `peak`.
fn samples(seconds: f32, peak: f32, mut at: impl FnMut(f32) -> f32) -> Vec<f32> {
    let len = (seconds * RATE) as usize;
    // The last 8% fades to silence, so a buffer that ends mid-ring never clicks.
    let fade_from = len - len / 12;
    let mut data: Vec<f32> = (0..len)
        .map(|i| {
            let fade = if i < fade_from {
                1.0
            } else {
                (len - i) as f32 / (len - fade_from) as f32
            };
            at(i as f32 / RATE) * fade
        })
        .collect();
    let loudest = data.iter().fold(0.0_f32, |max, s| max.max(s.abs()));
    if loudest > 0.0 {
        let gain = peak / loudest;
        data.iter_mut().for_each(|s| *s *= gain);
    }
    data
}

/// A short bright click over a low body knock, the sound of slug on rubber.
fn clack(seed: u32, body_hz: f32) -> Vec<f32> {
    let mut noise = Noise(seed);
    let mut previous = 0.0;
    samples(0.07, 0.8, |t| {
        let white = noise.next();
        let bright = white - previous;
        previous = white;
        let click = bright * (-t / 0.004).exp() * 0.55;
        let knock = (std::f32::consts::TAU * body_hz * t).sin() * (-t / 0.014).exp() * 0.5;
        click + knock
    })
}

fn bell() -> Vec<f32> {
    let partials = [
        (2_093.0, 0.5, 0.45),
        (4_186.0, 0.18, 0.25),
        (5_430.0, 0.12, 0.18),
        (2_617.0, 0.08, 0.35),
    ];
    samples(1.6, 0.35, |t| {
        let attack = (t / 0.002).min(1.0);
        partials
            .iter()
            .map(|&(hz, amp, decay)| {
                (std::f32::consts::TAU * hz * t).sin() * amp * (-t / decay).exp()
            })
            .sum::<f32>()
            * attack
            * 0.45
    })
}

/// A ratchet of pawl clicks over a sliding rumble, ending in a knock at the stop.
fn carriage_return() -> Vec<f32> {
    let mut noise = Noise(0x9e37_79b9);
    samples(0.42, 0.7, |t| {
        let n = noise.next();
        let slide = n
            * 0.08
            * (t / 0.32).min(1.0)
            * if t < 0.34 {
                1.0
            } else {
                (-(t - 0.34) / 0.02).exp()
            };
        let phase = (t % 0.028) / 0.028;
        let ratchet = if t < 0.3 {
            n * (-phase / 0.06).exp() * 0.35
        } else {
            0.0
        };
        let stop = if t >= 0.34 {
            let s = t - 0.34;
            (std::f32::consts::TAU * 110.0 * s).sin() * (-s / 0.03).exp() * 0.7
                + n * (-s / 0.005).exp() * 0.4
        } else {
            0.0
        };
        slide + ratchet + stop
    })
}

struct Bank {
    context: AudioContext,
    clacks: Vec<AudioBuffer>,
    space: AudioBuffer,
    bell: AudioBuffer,
    ret: AudioBuffer,
    next_clack: usize,
}

impl Bank {
    fn new() -> Option<Self> {
        let context = AudioContext::new().ok()?;
        let buffer = |data: Vec<f32>| -> Option<AudioBuffer> {
            let buffer = context.create_buffer(1, data.len() as u32, RATE).ok()?;
            buffer.copy_to_channel(&data, 0).ok()?;
            Some(buffer)
        };
        let clacks = (0..CLACKS)
            .map(|i| {
                buffer(clack(
                    0x2545_f491 ^ (i as u32 * 7_919),
                    150.0 + i as f32 * 14.0,
                ))
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Self {
            space: buffer(clack(0x1357_9bdf, 92.0))?,
            bell: buffer(bell())?,
            ret: buffer(carriage_return())?,
            clacks,
            next_clack: 0,
            context,
        })
    }

    fn play(&mut self, cue: Cue) {
        let buffer = match cue {
            Cue::Clack => {
                self.next_clack = (self.next_clack + 1) % CLACKS;
                &self.clacks[self.next_clack]
            }
            Cue::Space => &self.space,
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
    fn sounds_stay_in_range_and_decay() {
        for data in [clack(1, 150.0), bell(), carriage_return()] {
            assert!(data.iter().all(|s| s.abs() <= 1.0));
            let tail = &data[data.len() - 100..];
            assert!(tail.iter().all(|s| s.abs() < 0.05));
        }
    }
}
