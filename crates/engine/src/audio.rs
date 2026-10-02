//! A procedural synth for the inside of a ship: what's heard is what
//! carries through the hull and the cabin air. No sample files; everything
//! is generated, driven by what the simulation does.
//!
//! - Continuous layers set every frame: each thruster's hiss (panned to the
//!   side it sits on), the main drive's rumble through the structure, the
//!   cabin's life support, the hyperdrive's drone.
//! - One-shots: struck metal (a hull's ringing modes, a crack, a thud
//!   felt through the frame), thuds, hisses (air, valves), debris rattle,
//!   systems spooling up and down, the computer's soft chirps, explosions.
//! - Stereo, through a small metal room's reverb.

use std::f32::consts::{PI, TAU};
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};

pub struct Audio {
    synth: Arc<Mutex<Synth>>,
    /// The device it plays on (none: rendered offline, see `render`).
    _stream: Option<cpal::Stream>,
}

/// One thruster as heard this frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct Jet {
    /// How hard it fires, 0..1.
    pub level: f32,
    /// Where it sits, left (-1) to right (1).
    pub pan: f32,
    /// How near the cabin, 0 (far aft) .. 1 (by the cockpit): nearer is louder and brighter.
    pub near: f32,
    /// A lift jet (bigger, lower) rather than a small thruster.
    pub lift: bool,
}

impl Audio {
    /// Open the default output device. Returns `None` (silently playing nothing) if unavailable.
    pub fn new() -> Option<Self> {
        let host = cpal::default_host();
        let Some(device) = host.default_output_device() else {
            log::warn!("audio: no output device on host {:?}", host.id());
            return None;
        };
        let config = match device.default_output_config() {
            Ok(c) => c,
            Err(e) => {
                log::warn!("audio: no default output config: {e}");
                return None;
            }
        };
        log::info!(
            "audio: host {:?}, device {:?}, {:?}",
            host.id(),
            device.description().map(|d| d.to_string()).unwrap_or_default(),
            config
        );
        let format = config.sample_format();
        let config: cpal::StreamConfig = config.into();
        let synth = Arc::new(Mutex::new(Synth::new(config.sample_rate as f32)));
        let stream = match format {
            SampleFormat::F32 => build::<f32>(&device, config, synth.clone()),
            SampleFormat::I16 => build::<i16>(&device, config, synth.clone()),
            SampleFormat::I32 => build::<i32>(&device, config, synth.clone()),
            SampleFormat::U16 => build::<u16>(&device, config, synth.clone()),
            other => {
                log::warn!("audio: unsupported sample format {other}");
                return None;
            }
        }?;
        if let Err(e) = stream.play() {
            log::warn!("audio: failed to start stream: {e}");
            return None;
        }
        Some(Self { synth, _stream: Some(stream) })
    }

    /// A synth playing on no device, at `rate` samples a second: what it
    /// plays comes out of `render` (for checking and recording sounds).
    pub fn offline(rate: f32) -> Self {
        Self { synth: Arc::new(Mutex::new(Synth::new(rate))), _stream: None }
    }

    /// The next `n` stereo samples (offline).
    pub fn render(&self, n: usize) -> Vec<(f32, f32)> {
        let mut out = Vec::with_capacity(n);
        self.with(|s| {
            for _ in 0..n {
                out.push(s.sample());
            }
        });
        out
    }

    fn with(&self, f: impl FnOnce(&mut Synth)) {
        if let Ok(mut s) = self.synth.lock() {
            f(&mut s);
        }
    }

    pub fn set_master(&self, volume: f32) {
        self.with(|s| s.master = volume);
    }

    /// The main drive's rumble through the hull, 0..1.
    pub fn set_engine(&self, level: f32) {
        self.with(|s| s.engine.target = level.clamp(0.0, 1.0));
    }

    /// The cabin's life support (fans, the hum of the plant), 0..1.
    pub fn set_ambience(&self, level: f32) {
        self.with(|s| s.ambience.target = level.clamp(0.0, 1.0));
    }

    /// Each thruster this frame (their order kept from frame to frame).
    pub fn set_jets(&self, jets: &[Jet]) {
        self.with(|s| {
            s.jets.resize_with(jets.len(), JetVoice::default);
            for (k, (v, j)) in s.jets.iter_mut().zip(jets).enumerate() {
                v.jet = *j;
                v.seed = k as u32;
            }
        });
    }

    /// Droning hum (hyperdrive), 0..1 level at `pitch` Hz.
    pub fn set_drone(&self, level: f32, pitch: f32) {
        self.with(|s| {
            s.drone.target = level.clamp(0.0, 1.0);
            s.drone_pitch.target = pitch;
        });
    }

    /// The computer's chirp: a soft tone gliding from `f0` to `f1` Hz.
    pub fn tone(&self, f0: f32, f1: f32, seconds: f32, volume: f32) {
        self.with(|s| s.voices.push(Voice::Beep { phase: 0.0, f0, f1, t: 0.0, dur: seconds, vol: volume }));
    }

    /// An alarm's note: a hard-edged tone (a saw, rounded off) gliding from
    /// `f0` to `f1` Hz, to cut through everything else.
    pub fn alarm(&self, f0: f32, f1: f32, seconds: f32, volume: f32) {
        self.alarm_after(0.0, f0, f1, seconds, volume);
    }

    /// `alarm`, starting `delay` seconds from now.
    pub fn alarm_after(&self, delay: f32, f0: f32, f1: f32, seconds: f32, volume: f32) {
        let note = Voice::Alarm { phase: 0.0, f0, f1, t: 0.0, dur: seconds, vol: volume, lp: 0.0 };
        self.with(|s| s.voices.push(if delay > 0.0 { Voice::Later { wait: delay, then: Box::new(note) } } else { note }));
    }

    /// A voice on the radio for `seconds`, starting `delay` seconds from
    /// now: speech you hear but can't make out (a buzz at `pitch` Hz shaped
    /// into syllables by shifting vowel formants, consonants between),
    /// through a radio's narrow band and a little distortion, static under
    /// it, the squelch clicking open and shut. `voice` keeps a speaker's
    /// manner (its pace and its vowels) the same each time; `volume` its loudness.
    pub fn radio(&self, delay: f32, seconds: f32, pitch: f32, voice: u32, volume: f32) {
        let r = Voice::Radio(Box::new(Radio::new(seconds, pitch, voice, volume)));
        self.with(|s| s.voices.push(if delay > 0.0 { Voice::Later { wait: delay, then: Box::new(r) } } else { r }));
    }

    /// A blast: a deep boom and its crackle (`seconds` long).
    pub fn noise(&self, seconds: f32, volume: f32) {
        self.with(|s| s.voices.push(Voice::Blast { t: 0.0, dur: seconds, vol: volume, lp: 0.0, lp2: 0.0, phase: 0.0 }));
    }

    /// The hull struck, `strength` 0..1, from `pan` (-1 left .. 1 right):
    /// a crack, the plating ringing in its own (inharmonic) modes, a thud
    /// through the frame; `pitch` scales the modes (smaller, higher).
    pub fn impact(&self, strength: f32, pitch: f32, pan: f32) {
        self.with(|s| {
            let st = strength.clamp(0.0, 1.0);
            // A plate's modes: inharmonic ratios, the higher dying sooner.
            const RATIOS: [f32; 6] = [1.0, 1.59, 2.14, 2.65, 3.16, 4.18];
            let base = 180.0 * pitch * (1.0 + 0.15 * s.white());
            let mut modes = [(0.0, 0.0, 0.0, 0.0); 6];
            for (m, r) in modes.iter_mut().zip(RATIOS) {
                let f = base * r * (1.0 + 0.02 * s.white());
                // (Plating held by its frame and lined: damped, not a bell.)
                *m = (f, 0.08 + 0.3 * st / r, (1.0 / r).sqrt() * (0.5 + 0.5 * s.white().abs()), 0.0);
            }
            s.voices.push(Voice::Ring { modes, t: 0.0, vol: 0.35 + 0.65 * st, pan });
            s.voices.push(Voice::Crack { t: 0.0, vol: 0.4 + 0.6 * st, pan, hp: 0.0, last: 0.0 });
            s.voices.push(Voice::Thud { phase: 0.0, freq: 55.0, t: 0.0, decay: 0.12 + 0.25 * st, vol: 0.3 + 0.7 * st, pan: pan * 0.3 });
        });
    }

    /// A thud felt through the deck (a clamp, a step, the gun's recoil):
    /// a low knock at `freq` Hz, falling as it sounds.
    pub fn thud(&self, freq: f32, volume: f32, pan: f32) {
        self.with(|s| s.voices.push(Voice::Thud { phase: 0.0, freq, t: 0.0, decay: 0.09, vol: volume, pan }));
    }

    /// A hiss of air or gas (a valve, the hatch, a breach venting):
    /// `bright` 0 (a dull rush) .. 1 (a sharp jet).
    pub fn hiss(&self, seconds: f32, volume: f32, bright: f32, pan: f32) {
        self.with(|s| s.voices.push(Voice::Hiss { t: 0.0, dur: seconds, vol: volume, bright: bright.clamp(0.0, 1.0), pan, low: 0.0, band: 0.0 }));
    }

    /// Bits of something rattling off the hull for `seconds`.
    pub fn rattle(&self, seconds: f32, volume: f32, pan: f32) {
        self.with(|s| s.voices.push(Voice::Rattle { t: 0.0, dur: seconds, vol: volume, pan, next: 0.0, env: 0.0, low: 0.0, band: 0.0 }));
    }

    /// The ship's systems spooling up (relays, then a rising whine) or down.
    pub fn spool(&self, up: bool) {
        self.with(|s| {
            s.voices.push(Voice::Spool { up, t: 0.0, phase: 0.0 });
            for k in 0..3 {
                s.voices.push(Voice::Delayed { wait: 0.07 * k as f32 + if up { 0.0 } else { 0.9 }, freq: 900.0 + 300.0 * k as f32 });
            }
        });
    }
}

fn build<T: SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    synth: Arc<Mutex<Synth>>,
) -> Option<cpal::Stream> {
    let channels = config.channels as usize;
    let stream = device.build_output_stream(
        config,
        move |data: &mut [T], _| {
            let Ok(mut s) = synth.lock() else { return };
            for frame in data.chunks_mut(channels) {
                let (l, r) = s.sample();
                if frame.len() >= 2 {
                    frame[0] = T::from_sample(l);
                    frame[1] = T::from_sample(r);
                    for c in &mut frame[2..] {
                        *c = T::from_sample((l + r) * 0.5);
                    }
                } else {
                    frame.fill(T::from_sample((l + r) * 0.5));
                }
            }
        },
        |e| log::warn!("audio stream error: {e}"),
        None,
    );
    stream.map_err(|e| log::warn!("audio: failed to open stream: {e}")).ok()
}

/// A parameter that glides toward its target to avoid clicks.
#[derive(Default)]
struct Smoothed {
    value: f32,
    target: f32,
}

impl Smoothed {
    fn next(&mut self, rate: f32) -> f32 {
        self.value += (self.target - self.value) * rate;
        self.value
    }
}

/// A state-variable filter (Chamberlin): low and band outputs.
fn svf(low: &mut f32, band: &mut f32, input: f32, freq: f32, q: f32, rate: f32) {
    let f = 2.0 * (PI * (freq / rate).min(0.16)).sin();
    let high = input - *low - band.clamp(-8.0, 8.0) / q;
    *band += f * high;
    *low += f * *band;
}

/// Constant-power pan: (left, right) gains for `pan` -1..1.
fn panned(pan: f32) -> (f32, f32) {
    let a = (pan.clamp(-1.0, 1.0) + 1.0) * PI / 4.0;
    (a.cos(), a.sin())
}

/// One thruster's voice: its hiss through a band filter, opened and closed
/// by a valve envelope, a tick as the valve opens.
#[derive(Default)]
struct JetVoice {
    jet: Jet,
    seed: u32,
    env: f32,
    low: f32,
    band: f32,
    rush: f32,
    open: bool,
    tick: f32,
}

enum Voice {
    Beep { phase: f32, f0: f32, f1: f32, t: f32, dur: f32, vol: f32 },
    Alarm { phase: f32, f0: f32, f1: f32, t: f32, dur: f32, vol: f32, lp: f32 },
    Blast { t: f32, dur: f32, vol: f32, lp: f32, lp2: f32, phase: f32 },
    /// Struck metal ringing: (frequency, decay time, amplitude, phase) each mode.
    Ring { modes: [(f32, f32, f32, f32); 6], t: f32, vol: f32, pan: f32 },
    Crack { t: f32, vol: f32, pan: f32, hp: f32, last: f32 },
    Thud { phase: f32, freq: f32, t: f32, decay: f32, vol: f32, pan: f32 },
    Hiss { t: f32, dur: f32, vol: f32, bright: f32, pan: f32, low: f32, band: f32 },
    Rattle { t: f32, dur: f32, vol: f32, pan: f32, next: f32, env: f32, low: f32, band: f32 },
    Spool { up: bool, t: f32, phase: f32 },
    /// A relay's click, `wait` seconds on.
    Delayed { wait: f32, freq: f32 },
    /// Another voice, `wait` seconds on.
    Later { wait: f32, then: Box<Voice> },
    Radio(Box<Radio>),
}

/// Vowels as their first two formants (Hz).
const VOWELS: [(f32, f32); 8] = [(730.0, 1090.0), (530.0, 1840.0), (270.0, 2290.0), (570.0, 840.0), (300.0, 870.0), (660.0, 1720.0), (440.0, 1020.0), (490.0, 1350.0)];

/// A transmission (see `Audio::radio`).
struct Radio {
    t: f32,
    dur: f32,
    vol: f32,
    pitch: f32,
    rng: u32,
    /// The glottal pulse's phase; the syllable's start, length and vowel.
    phase: f32,
    syllable_at: f32,
    syllable_len: f32,
    pace: f32,
    target: (f32, f32),
    formant: (f32, f32),
    /// Before a vowel, a consonant's hiss (s) this long.
    consonant: f32,
    f1: (f32, f32),
    f2: (f32, f32),
    band: (f32, f32),
    high: (f32, f32),
    high2: (f32, f32),
    stat: f32,
}

impl Radio {
    fn new(dur: f32, pitch: f32, voice: u32, vol: f32) -> Self {
        let mut r = Radio {
            t: 0.0,
            dur,
            vol,
            pitch,
            rng: voice.wrapping_mul(2_654_435_761) | 1,
            phase: 0.0,
            syllable_at: 0.0,
            syllable_len: 0.0,
            pace: 0.0,
            target: VOWELS[0],
            formant: VOWELS[0],
            consonant: 0.0,
            f1: (0.0, 0.0),
            f2: (0.0, 0.0),
            band: (0.0, 0.0),
            high: (0.0, 0.0),
            high2: (0.0, 0.0),
            stat: 0.0,
        };
        r.pace = 0.16 + 0.08 * r.next().abs();
        r.syllable_at = 0.12;
        r
    }

    fn next(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        self.rng as f32 / u32::MAX as f32 * 2.0 - 1.0
    }

    fn sample(&mut self, noise: f32, rate: f32) -> (f32, bool) {
        let dt = 1.0 / rate;
        let t = self.t;
        self.t += dt;
        // The squelch: a click open, static under all of it, a "kssht" closing.
        let (open, close) = (0.08, self.dur - 0.15);
        self.stat += (noise - self.stat) * 0.5;
        let mut out = self.stat * 0.06;
        if t < 0.01 || (t > close && t < close + 0.01) {
            out += noise * 0.8;
        }
        if t > close {
            out += noise * 0.35 * (1.0 - (t - close) / 0.15).max(0.0);
        }
        // Speech, between: syllables at its pace, a pause now and then.
        if t > open && t < close - 0.05 {
            if t >= self.syllable_at {
                self.syllable_len = self.pace * (0.7 + 0.6 * self.next().abs());
                let pause = if self.next() > 0.75 { 0.12 + 0.15 * self.next().abs() } else { 0.0 };
                self.syllable_at = t + self.syllable_len + pause;
                let v = (self.next().abs() * VOWELS.len() as f32) as usize % VOWELS.len();
                self.target = VOWELS[v];
                self.consonant = if self.next() > 0.0 { 0.03 + 0.04 * self.next().abs() } else { 0.0 };
            }
            // (How far into the syllable.)
            let k = (1.0 - (self.syllable_at - t) / self.syllable_len.max(1e-3)).clamp(0.0, 1.0);
            // (The formants glide to the vowel; the pitch falls through a phrase.)
            self.formant.0 += (self.target.0 - self.formant.0) * 0.004;
            self.formant.1 += (self.target.1 - self.formant.1) * 0.004;
            let f0 = self.pitch * (1.1 - 0.2 * (t / self.dur)) * (1.0 + 0.05 * (k * PI).sin());
            self.phase += f0 * dt;
            let pulse = if self.phase >= 1.0 {
                self.phase -= 1.0;
                1.0
            } else {
                0.0
            };
            let env = (k * PI).sin().max(0.0).powf(0.6);
            let consonant = k * self.syllable_len < self.consonant;
            let source = if consonant { noise * 0.25 } else { pulse * 6.0 };
            svf(&mut self.f1.0, &mut self.f1.1, source, self.formant.0, 6.0, rate);
            svf(&mut self.f2.0, &mut self.f2.1, source, self.formant.1, 8.0, rate);
            out += (self.f1.1 + 0.6 * self.f2.1) * env * 0.5;
        }
        // The radio: a narrow band (about 400 Hz to 3 kHz), a little overdriven.
        // (Two high-pass stages: a radio's small speaker has no bass.)
        svf(&mut self.high.0, &mut self.high.1, out, 400.0, 0.7, rate);
        let hp = out - self.high.0;
        svf(&mut self.high2.0, &mut self.high2.1, hp, 400.0, 0.7, rate);
        let hp = hp - self.high2.0;
        svf(&mut self.band.0, &mut self.band.1, hp, 2800.0, 0.7, rate);
        let v = (self.band.0 * 4.0).tanh() * 1.2;
        (v * self.vol, self.t >= self.dur)
    }
}

/// A small metal room: Schroeder reverb (combs into all-passes), a side each.
struct Room {
    combs: [Vec<f32>; 4],
    comb_at: [usize; 4],
    comb_lp: [f32; 4],
    passes: [Vec<f32>; 2],
    pass_at: [usize; 2],
}

impl Room {
    fn new(rate: f32, spread: usize) -> Self {
        let n = |ms: f32| ((ms * 0.001 * rate) as usize).max(1) + spread;
        Room {
            combs: [vec![0.0; n(23.1)], vec![0.0; n(26.9)], vec![0.0; n(29.3)], vec![0.0; n(31.7)]],
            comb_at: [0; 4],
            comb_lp: [0.0; 4],
            passes: [vec![0.0; n(5.1)], vec![0.0; n(1.7)]],
            pass_at: [0; 2],
        }
    }

    fn sample(&mut self, input: f32) -> f32 {
        let mut out = 0.0;
        for k in 0..4 {
            let line = &mut self.combs[k];
            let at = self.comb_at[k];
            let y = line[at];
            // (The walls soak up the highs: each echo darker.)
            self.comb_lp[k] += (y - self.comb_lp[k]) * 0.45;
            line[at] = input + self.comb_lp[k] * 0.72;
            self.comb_at[k] = (at + 1) % line.len();
            out += y;
        }
        out *= 0.25;
        for k in 0..2 {
            let line = &mut self.passes[k];
            let at = self.pass_at[k];
            let y = line[at];
            line[at] = out + y * 0.5;
            out = y - out * 0.5;
            self.pass_at[k] = (at + 1) % line.len();
        }
        out
    }
}

struct Synth {
    rate: f32,
    master: f32,
    rng: u32,
    engine: Smoothed,
    engine_lp: f32,
    engine_lp2: f32,
    engine_phase: f32,
    throb: f32,
    ambience: Smoothed,
    hum_phase: f32,
    air_lp: f32,
    drone: Smoothed,
    drone_pitch: Smoothed,
    drone_phase: [f32; 3],
    drone_lp: f32,
    jets: Vec<JetVoice>,
    voices: Vec<Voice>,
    rooms: [Room; 2],
}

impl Synth {
    fn new(rate: f32) -> Self {
        Self {
            rate,
            master: 0.5,
            rng: 0x1234_5678,
            engine: Smoothed::default(),
            engine_lp: 0.0,
            engine_lp2: 0.0,
            engine_phase: 0.0,
            throb: 0.0,
            ambience: Smoothed::default(),
            hum_phase: 0.0,
            air_lp: 0.0,
            drone: Smoothed::default(),
            drone_pitch: Smoothed { value: 60.0, target: 60.0 },
            drone_phase: [0.0; 3],
            drone_lp: 0.0,
            jets: Vec::new(),
            voices: Vec::new(),
            rooms: [Room::new(rate, 0), Room::new(rate, 23)],
        }
    }

    fn white(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        self.rng as f32 / u32::MAX as f32 * 2.0 - 1.0
    }

    /// One stereo sample.
    fn sample(&mut self) -> (f32, f32) {
        let rate = self.rate;
        let dt = 1.0 / rate;
        let glide = 8.0 * dt;
        // Dry (left, right), and what goes to the room.
        let (mut l, mut r) = (0.0f32, 0.0f32);
        let mut send = 0.0f32;
        let put = |v: f32, pan: f32, wet: f32, l: &mut f32, r: &mut f32, send: &mut f32| {
            let (gl, gr) = panned(pan);
            *l += v * gl;
            *r += v * gr;
            *send += v * wet;
        };

        // The main drive: a deep rumble carried by the frame, a slow throb in it.
        let engine = self.engine.next(glide);
        if engine > 1e-4 {
            let n = self.white();
            let cutoff = 0.004 + 0.02 * engine;
            self.engine_lp += (n - self.engine_lp) * cutoff;
            self.engine_lp2 += (self.engine_lp - self.engine_lp2) * cutoff;
            self.engine_phase = (self.engine_phase + (32.0 + 14.0 * engine) * dt).fract();
            self.throb = (self.throb + 0.9 * dt).fract();
            let sub = (self.engine_phase * TAU).sin() * (0.8 + 0.2 * (self.throb * TAU).sin());
            let v = (self.engine_lp2 * 5.0 + sub * 0.35) * engine;
            put(v, 0.0, 0.1, &mut l, &mut r, &mut send);
        }

        // Life support: the plant's hum and the fans' air.
        let amb = self.ambience.next(glide * 0.25);
        if amb > 1e-4 {
            self.hum_phase = (self.hum_phase + 50.0 * dt).fract();
            let hum = (self.hum_phase * TAU).sin() * 0.5 + (self.hum_phase * 2.0 * TAU).sin() * 0.3 + (self.hum_phase * 3.0 * TAU).sin() * 0.08;
            let n = self.white();
            self.air_lp += (n - self.air_lp) * 0.05;
            put((hum * 0.12 + self.air_lp * 0.45) * amb, 0.0, 0.3, &mut l, &mut r, &mut send);
        }

        // The thrusters: each a hiss through its own band, the valve's
        // envelope opening fast and closing with a tail ("pssht").
        for k in 0..self.jets.len() {
            let n = self.white();
            let v = &mut self.jets[k];
            let j = v.jet;
            let on = j.level > 0.02;
            if on && !v.open {
                v.tick = 1.0;
            }
            v.open = on;
            let target = if on { j.level.sqrt() } else { 0.0 };
            let rate_env = if target > v.env { 1.0 / (0.012 * rate) } else { 1.0 / (0.05 * rate) };
            v.env += (target - v.env) * rate_env;
            if v.env < 1e-4 && v.tick < 1e-4 {
                continue;
            }
            // (Each jet a little different; lift jets bigger and lower; nearer the cabin, brighter.)
            let spread = 1.0 + 0.12 * (((v.seed * 2_654_435_761) >> 24) as f32 / 255.0 - 0.5);
            let centre = if j.lift { 700.0 } else { 1500.0 + 900.0 * j.near } * spread;
            svf(&mut v.low, &mut v.band, n, centre, if j.lift { 0.8 } else { 1.4 }, rate);
            v.rush += (n - v.rush) * 0.03;
            // (The small thrusters half the lift jets' loudness: they're small.)
            let gain = (0.35 + 0.65 * j.near) * if j.lift { 1.3 } else { 0.5 };
            let mut s = (v.band * 0.6 + v.rush * 0.8) * v.env * gain * 1.1;
            // The valve: a short knock as it opens.
            if v.tick > 1e-4 {
                s += v.tick * (v.low * 2.0).clamp(-1.0, 1.0) * 0.25;
                v.tick *= 1.0 - 1.0 / (0.004 * rate);
            }
            put(s, j.pan * 0.85, 0.25, &mut l, &mut r, &mut send);
        }

        // The hyperdrive: detuned saws under a low-pass, a sub beneath.
        let drone = self.drone.next(glide);
        let pitch = self.drone_pitch.next(glide * 0.5);
        if drone > 1e-4 {
            let mut saw = 0.0;
            for (i, p) in self.drone_phase.iter_mut().enumerate() {
                let f = if i == 2 { pitch * 0.5 } else { pitch * (1.0 + i as f32 * 0.007) };
                *p = (*p + f * dt).fract();
                saw += if i == 2 { (*p * TAU).sin() } else { *p * 2.0 - 1.0 };
            }
            self.drone_lp += (saw * 0.4 - self.drone_lp) * 0.04;
            put(self.drone_lp * drone * 1.4, 0.0, 0.2, &mut l, &mut r, &mut send);
        }

        // One-shots.
        let mut i = 0;
        while i < self.voices.len() {
            let noise = self.white();
            let (v, pan, wet, done) = match &mut self.voices[i] {
                Voice::Beep { phase, f0, f1, t, dur, vol } => {
                    let k = *t / *dur;
                    *phase = (*phase + (*f0 + (*f1 - *f0) * k) * dt).fract();
                    let env = (*t / 0.006).min(1.0) * (1.0 - k).powf(1.5);
                    *t += dt;
                    let w = *phase * TAU;
                    ((w.sin() + 0.18 * (3.0 * w).sin()) * env * *vol * 1.8, 0.0, 0.15, *t >= *dur)
                }
                Voice::Alarm { phase, f0, f1, t, dur, vol, lp } => {
                    let k = *t / *dur;
                    *phase = (*phase + (*f0 + (*f1 - *f0) * k) * dt).fract();
                    // (A saw through a gentle low-pass: harsh, not shrill; square-edged in time.)
                    *lp += ((*phase * 2.0 - 1.0) - *lp) * 0.35;
                    let env = (*t / 0.004).min(1.0) * ((*dur - *t) / 0.01).clamp(0.0, 1.0);
                    *t += dt;
                    (*lp * env * *vol * 4.0, 0.0, 0.1, *t >= *dur)
                }
                Voice::Blast { t, dur, vol, lp, lp2, phase } => {
                    let k = *t / *dur;
                    *lp += (noise - *lp) * (0.25 * (1.0 - k) + 0.01);
                    *lp2 += (*lp - *lp2) * 0.02;
                    *phase = (*phase + (45.0 * (1.0 - 0.5 * k)) * dt).fract();
                    let boom = (*phase * TAU).sin() * (-*t * 3.0).exp();
                    let crackle = if noise.abs() > 0.995 { noise.signum() * (1.0 - k) } else { 0.0 };
                    *t += dt;
                    ((*lp * 1.4 + *lp2 * 4.0 + boom * 0.8 + crackle * 0.6) * (1.0 - k) * *vol, 0.0, 0.3, *t >= *dur)
                }
                Voice::Ring { modes, t, vol, pan } => {
                    let mut s = 0.0;
                    let mut live = false;
                    for (f, decay, amp, ph) in modes.iter_mut() {
                        *ph = (*ph + *f * dt).fract();
                        let e = (-*t / *decay).exp();
                        live |= e > 1e-3;
                        s += (*ph * TAU).sin() * *amp * e;
                    }
                    *t += dt;
                    (s * *vol * 0.3, *pan, 0.4, !live)
                }
                Voice::Crack { t, vol, pan, hp, last } => {
                    // (High-passed noise, a few milliseconds: the snap of metal struck.)
                    *hp = 0.6 * (*hp + noise - *last);
                    *last = noise;
                    let e = (-*t / 0.006).exp();
                    *t += dt;
                    (*hp * e * *vol * 0.9, *pan, 0.3, e < 1e-3)
                }
                Voice::Thud { phase, freq, t, decay, vol, pan } => {
                    let f = *freq * (1.0 + 1.5 * (-*t * 30.0).exp());
                    *phase = (*phase + f * dt).fract();
                    let e = (*t / 0.002).min(1.0) * (-*t / *decay).exp();
                    *t += dt;
                    ((*phase * TAU).sin() * e * *vol, *pan, 0.2, *t > *decay * 7.0)
                }
                Voice::Hiss { t, dur, vol, bright, pan, low, band } => {
                    let k = *t / *dur;
                    svf(low, band, noise, 600.0 + 4000.0 * *bright, 0.9, rate);
                    let env = (*t / 0.03).min(1.0) * (1.0 - k).powi(2);
                    *t += dt;
                    ((*band * 0.7 + noise * 0.1 * *bright) * env * *vol, *pan, 0.3, *t >= *dur)
                }
                Voice::Rattle { t, dur, vol, pan, next, env, low, band } => {
                    // (Random small knocks, thinning out.)
                    if *t >= *next {
                        *env = 0.4 + 0.6 * noise.abs();
                        *next = *t + 0.01 + 0.08 * (*t / *dur) + 0.03 * noise.abs();
                    }
                    *env *= 1.0 - 1.0 / (0.006 * rate);
                    svf(low, band, noise, 2600.0, 3.0, rate);
                    let k = *t / *dur;
                    *t += dt;
                    (*band * *env * *vol * (1.0 - k), *pan, 0.35, *t >= *dur)
                }
                Voice::Spool { up, t, phase } => {
                    // A whine rising (or falling) over a second and a half, with a hum under it.
                    let dur = 1.6;
                    let k = (*t / dur).min(1.0);
                    let s = if *up { k } else { 1.0 - k };
                    let f = 80.0 + 520.0 * s * s;
                    *phase = (*phase + f * dt).fract();
                    let env = if *up { (k * 4.0).min(1.0) * (1.0 - k).max(0.0).sqrt() } else { (1.0 - k).powi(2) };
                    *t += dt;
                    let w = *phase * TAU;
                    ((w.sin() * 0.5 + (2.0 * w).sin() * 0.2) * env * 0.35, 0.0, 0.3, *t >= dur)
                }
                Voice::Radio(r) => {
                    let (v, done) = r.sample(noise, rate);
                    (v, 0.0, 0.08, done)
                }
                Voice::Later { wait, then } => {
                    *wait -= dt;
                    if *wait <= 0.0 {
                        let next = std::mem::replace(then.as_mut(), Voice::Delayed { wait: f32::INFINITY, freq: 0.0 });
                        self.voices[i] = next;
                    }
                    (0.0, 0.0, 0.0, false)
                }
                Voice::Delayed { wait, freq } => {
                    *wait -= dt;
                    if *wait <= 0.0 {
                        // (Becomes the relay's click: a small, sharp knock.)
                        let f = *freq;
                        self.voices[i] = Voice::Thud { phase: 0.0, freq: f, t: 0.0, decay: 0.008, vol: 0.25, pan: 0.0 };
                    }
                    (0.0, 0.0, 0.0, false)
                }
            };
            put(v, pan, wet, &mut l, &mut r, &mut send);
            if done {
                self.voices.swap_remove(i);
            } else {
                i += 1;
            }
        }

        let (wl, wr) = (self.rooms[0].sample(send), self.rooms[1].sample(send));
        (((l + wl * 0.5) * self.master).tanh(), ((r + wr * 0.5) * self.master).tanh())
    }
}
