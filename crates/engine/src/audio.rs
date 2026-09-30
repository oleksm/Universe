//! A tiny procedural synth: continuous engine/drone layers plus one-shot bleeps
//! and noise bursts. No sample files, everything is generated.

use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};

pub struct Audio {
    synth: Arc<Mutex<Synth>>,
    _stream: cpal::Stream,
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
        Some(Self { synth, _stream: stream })
    }

    fn with(&self, f: impl FnOnce(&mut Synth)) {
        if let Ok(mut s) = self.synth.lock() {
            f(&mut s);
        }
    }

    pub fn set_master(&self, volume: f32) {
        self.with(|s| s.master = volume);
    }

    /// Engine rumble, 0..1.
    pub fn set_engine(&self, level: f32) {
        self.with(|s| s.engine.target = level.clamp(0.0, 1.0));
    }

    /// Droning hum (hyperdrive), 0..1 level at `pitch` Hz.
    pub fn set_drone(&self, level: f32, pitch: f32) {
        self.with(|s| {
            s.drone.target = level.clamp(0.0, 1.0);
            s.drone_pitch.target = pitch;
        });
    }

    /// Square-wave bleep sweeping from `f0` to `f1` Hz.
    pub fn tone(&self, f0: f32, f1: f32, seconds: f32, volume: f32) {
        self.with(|s| s.voices.push(Voice::Tone { phase: 0.0, f0, f1, t: 0.0, dur: seconds, vol: volume }));
    }

    /// Decaying noise burst (explosions, impacts).
    pub fn noise(&self, seconds: f32, volume: f32) {
        self.with(|s| s.voices.push(Voice::Noise { t: 0.0, dur: seconds, vol: volume, lp: 0.0 }));
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
                let v = T::from_sample(s.sample());
                frame.fill(v);
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

enum Voice {
    Tone { phase: f32, f0: f32, f1: f32, t: f32, dur: f32, vol: f32 },
    Noise { t: f32, dur: f32, vol: f32, lp: f32 },
}

struct Synth {
    rate: f32,
    master: f32,
    rng: u32,
    engine: Smoothed,
    engine_lp: f32,
    engine_lp2: f32,
    drone: Smoothed,
    drone_pitch: Smoothed,
    drone_phase: [f32; 2],
    drone_lp: f32,
    voices: Vec<Voice>,
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
            drone: Smoothed::default(),
            drone_pitch: Smoothed { value: 60.0, target: 60.0 },
            drone_phase: [0.0; 2],
            drone_lp: 0.0,
            voices: Vec::new(),
        }
    }

    fn white(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        self.rng as f32 / u32::MAX as f32 * 2.0 - 1.0
    }

    fn sample(&mut self) -> f32 {
        let dt = 1.0 / self.rate;
        let glide = 8.0 * dt;
        let mut out = 0.0;

        // Engine: brown-ish filtered noise, brighter with more throttle.
        let engine = self.engine.next(glide);
        if engine > 1e-4 {
            let n = self.white();
            let cutoff = 0.01 + 0.06 * engine;
            self.engine_lp += (n - self.engine_lp) * cutoff;
            self.engine_lp2 += (self.engine_lp - self.engine_lp2) * cutoff;
            out += self.engine_lp2 * engine * 2.5;
        }

        // Drone: two detuned saws through a low-pass.
        let drone = self.drone.next(glide);
        let pitch = self.drone_pitch.next(glide * 0.5);
        if drone > 1e-4 {
            let mut saw = 0.0;
            for (i, p) in self.drone_phase.iter_mut().enumerate() {
                *p = (*p + pitch * (1.0 + i as f32 * 0.007) * dt).fract();
                saw += *p * 2.0 - 1.0;
            }
            self.drone_lp += (saw * 0.5 - self.drone_lp) * 0.05;
            out += self.drone_lp * drone * 1.6;
        }

        // One-shots.
        let mut i = 0;
        while i < self.voices.len() {
            let noise = self.white();
            let (v, done) = match &mut self.voices[i] {
                Voice::Tone { phase, f0, f1, t, dur, vol } => {
                    let k = *t / *dur;
                    *phase = (*phase + (*f0 + (*f1 - *f0) * k) * dt).fract();
                    let env = (*t / 0.004).min(1.0) * (1.0 - k);
                    *t += dt;
                    (if *phase < 0.5 { 1.0 } else { -1.0 } * env * *vol, *t >= *dur)
                }
                Voice::Noise { t, dur, vol, lp } => {
                    let k = *t / *dur;
                    *lp += (noise - *lp) * (0.3 * (1.0 - k) + 0.02);
                    *t += dt;
                    (*lp * (1.0 - k) * (1.0 - k) * *vol * 2.0, *t >= *dur)
                }
            };
            out += v;
            if done {
                self.voices.swap_remove(i);
            } else {
                i += 1;
            }
        }

        (out * self.master).tanh()
    }
}
