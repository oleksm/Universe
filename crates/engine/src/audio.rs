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

    /// Air: rushing over the hull (`gusty` false: steady, `bright` 0..1 with
    /// the speed) or wind round someone standing in it (gusty), 0..1.
    pub fn set_air(&self, level: f32, bright: f32, gusty: bool) {
        self.with(|s| {
            s.air.target = level.clamp(0.0, 1.0);
            s.air_bright.target = bright.clamp(0.0, 1.0);
            s.gusty = gusty;
        });
    }

    /// Breathing inside a suit, 0..1.
    pub fn set_breath(&self, level: f32) {
        self.with(|s| s.breath.target = level.clamp(0.0, 1.0));
    }

    /// Other ships' engines heard through the air, 0..1, from `pan`.
    pub fn set_distant(&self, level: f32, pan: f32) {
        self.with(|s| {
            s.distant.target = level.clamp(0.0, 1.0);
            s.distant_pan.target = pan.clamp(-1.0, 1.0);
        });
    }

    /// The score: on or off (0..1 its loudness), and how tense (0 calm .. 1 a fight).
    pub fn set_music(&self, level: f32, tension: f32) {
        self.with(|s| {
            s.music.level.target = level.clamp(0.0, 1.0);
            s.music.tension.target = tension.clamp(0.0, 1.0);
        });
    }

    /// The score swells for a few seconds (an arrival).
    pub fn music_swell(&self) {
        self.with(|s| s.music.swell = 1.0);
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
        self.with(|s| s.push(Voice::Beep { phase: 0.0, f0, f1, t: 0.0, dur: seconds, vol: volume }));
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

    /// A blast: a deep boom and its crackle (`seconds` long).
    pub fn noise(&self, seconds: f32, volume: f32) {
        self.with(|s| s.push(Voice::Blast { t: 0.0, dur: seconds, vol: volume, lp: 0.0, lp2: 0.0, phase: 0.0 }));
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
            s.push(Voice::Ring { modes, t: 0.0, vol: 0.35 + 0.65 * st, pan });
            s.push(Voice::Crack { t: 0.0, vol: 0.4 + 0.6 * st, pan, hp: 0.0, last: 0.0 });
            s.push(Voice::Thud { phase: 0.0, freq: 55.0, t: 0.0, decay: 0.12 + 0.25 * st, vol: 0.3 + 0.7 * st, pan: pan * 0.3 });
        });
    }

    /// A thud felt through the deck (a clamp, a step, the gun's recoil):
    /// a low knock at `freq` Hz, falling as it sounds.
    pub fn thud(&self, freq: f32, volume: f32, pan: f32) {
        self.with(|s| s.push(Voice::Thud { phase: 0.0, freq, t: 0.0, decay: 0.09, vol: volume, pan }));
    }

    /// A hiss of air or gas (a valve, the hatch, a breach venting):
    /// `bright` 0 (a dull rush) .. 1 (a sharp jet).
    pub fn hiss(&self, seconds: f32, volume: f32, bright: f32, pan: f32) {
        self.with(|s| s.push(Voice::Hiss { t: 0.0, dur: seconds, vol: volume, bright: bright.clamp(0.0, 1.0), pan, low: 0.0, band: 0.0 }));
    }

    /// Bits of something rattling off the hull for `seconds`.
    pub fn rattle(&self, seconds: f32, volume: f32, pan: f32) {
        self.with(|s| s.push(Voice::Rattle { t: 0.0, dur: seconds, vol: volume, pan, next: 0.0, env: 0.0, low: 0.0, band: 0.0 }));
    }

    /// What `f` plays, starting `delay` seconds from now (a sequence: the
    /// machine's motor, then the can dropping, then it opening).
    pub fn after(&self, delay: f32, f: impl FnOnce(&Audio)) {
        self.with(|s| s.delay = delay.max(0.0));
        f(self);
        self.with(|s| s.delay = 0.0);
    }

    /// A small electric motor running for `seconds` (a buzz, its pitch
    /// rising as it gets going).
    pub fn motor(&self, seconds: f32, volume: f32) {
        self.with(|s| s.push(Voice::Motor { t: 0.0, dur: seconds, vol: volume, phase: 0.0, lp: 0.0 }));
    }

    /// A drink fizzing for `seconds`: tiny bubbles bursting, fewer as it goes flat.
    pub fn fizz(&self, seconds: f32, volume: f32) {
        self.with(|s| s.push(Voice::Fizz { t: 0.0, dur: seconds, vol: volume, env: 0.0, hp: 0.0, last: 0.0 }));
    }

    /// A crunchy bite: a burst of cracking, brittle and dry.
    pub fn crunch(&self, volume: f32) {
        self.with(|s| s.push(Voice::Crunch { t: 0.0, vol: volume, env: 0.0, next: 0.0, low: 0.0, band: 0.0 }));
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
    Motor { t: f32, dur: f32, vol: f32, phase: f32, lp: f32 },
    Fizz { t: f32, dur: f32, vol: f32, env: f32, hp: f32, last: f32 },
    Crunch { t: f32, vol: f32, env: f32, next: f32, low: f32, band: f32 },
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
    air: Smoothed,
    air_bright: Smoothed,
    gusty: bool,
    gust: f32,
    gust_to: f32,
    air_lpf: (f32, f32),
    air_low: f32,
    breath: Smoothed,
    breath_t: f32,
    breath_f: (f32, f32),
    distant: Smoothed,
    distant_pan: Smoothed,
    distant_lp: (f32, f32),
    music: Music,
    /// One-shots played now start this long after (see `Audio::after`).
    delay: f32,
}

/// The score: bright pads drifting through a major cycle, opening and
/// closing slowly; a soft arpeggio of the chord through a long echo (two
/// cycles on, one off, so it never nags); a gentle pulse on the root; a bell
/// now and then; and under tension the pads darken and a driving bass comes in.
struct Music {
    level: Smoothed,
    tension: Smoothed,
    swell: f32,
    t: f32,
    /// Seconds since it started (the pads' slow sweep).
    clock: f32,
    chord: usize,
    /// Chords played (the arpeggio's on and off).
    played: usize,
    pads: Vec<Pad>,
    bells: Vec<Bell>,
    next_bell: f32,
    plucks: Vec<Pluck>,
    step: usize,
    step_t: f32,
    arp: Smoothed,
    echo: Vec<f32>,
    echo_at: usize,
    pulse_env: f32,
    pulse_phase: f32,
    beat: f32,
    bass_env: f32,
    bass_phase: f32,
    bass_lp: f32,
    rng: u32,
}

/// One note of a pad: three detuned saws, low-passed, a slow envelope.
struct Pad {
    freq: f32,
    phase: [f32; 3],
    env: f32,
    on: bool,
    lp: f32,
}

/// A bell: a sine carrier, a sine modulating it (FM), dying away.
struct Bell {
    freq: f32,
    t: f32,
    carrier: f32,
    modulator: f32,
    vol: f32,
}

/// An arpeggio note: a soft sine with a touch of its octave, quick in, dying away.
struct Pluck {
    freq: f32,
    t: f32,
    phase: f32,
    vol: f32,
}

/// The chords (MIDI notes): a bright major cycle in D (Imaj7, IVmaj7, vi, V).
const CHORDS: [[u8; 4]; 4] = [[50, 57, 61, 66], [43, 55, 59, 66], [47, 54, 59, 62], [45, 57, 61, 64]];
/// Each chord this long (s).
const CHORD_TIME: f32 = 10.0;
/// The arpeggio's pace (beats a minute; it plays eighths).
const TEMPO: f32 = 104.0;
/// Which of the chord's notes the arpeggio takes, step by step (4: the root an octave up).
const PATTERN: [usize; 8] = [0, 1, 2, 3, 4, 3, 2, 1];

fn midi(n: f32) -> f32 {
    440.0 * 2f32.powf((n - 69.0) / 12.0)
}

impl Music {
    fn new() -> Self {
        Music {
            level: Smoothed::default(),
            tension: Smoothed::default(),
            swell: 0.0,
            t: CHORD_TIME,
            clock: 0.0,
            chord: CHORDS.len() - 1,
            played: 0,
            pads: Vec::new(),
            bells: Vec::new(),
            next_bell: 4.0,
            plucks: Vec::new(),
            step: 0,
            step_t: 0.0,
            arp: Smoothed::default(),
            echo: Vec::new(),
            echo_at: 0,
            pulse_env: 0.0,
            pulse_phase: 0.0,
            beat: 0.0,
            bass_env: 0.0,
            bass_phase: 0.0,
            bass_lp: 0.0,
            rng: 0x9e37_79b9,
        }
    }

    fn rand(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        self.rng as f32 / u32::MAX as f32
    }

    fn sample(&mut self, rate: f32) -> f32 {
        let dt = 1.0 / rate;
        let level = self.level.next(dt * 0.5);
        let tension = self.tension.next(dt * 0.3);
        if level < 1e-4 && self.pads.is_empty() {
            return 0.0;
        }
        self.clock += dt;
        self.swell = (self.swell - dt / 6.0).max(0.0);
        // The next chord: the old notes let go, the new ones come in.
        self.t += dt;
        if self.t >= CHORD_TIME {
            self.t = 0.0;
            self.chord = (self.chord + 1) % CHORDS.len();
            self.played += 1;
            for p in &mut self.pads {
                p.on = false;
            }
            for &n in &CHORDS[self.chord] {
                self.pads.push(Pad { freq: midi(n as f32), phase: [0.0, 0.33, 0.66], env: 0.0, on: true, lp: 0.0 });
            }
        }
        let notes = CHORDS[self.chord];
        let mut out = 0.0;
        // Pads: open, breathing slowly brighter and darker (tension darkens them; an arrival opens them up).
        let sweep = 0.5 + 0.5 * (self.clock * TAU / 37.0).sin();
        let cutoff = (0.035 + 0.035 * sweep + 0.03 * self.swell - 0.03 * tension).max(0.006);
        for p in &mut self.pads {
            let target = if p.on { 1.0 } else { 0.0 };
            let speed = if p.on { 1.0 / 2.5 } else { 1.0 / 4.0 };
            p.env += (target - p.env) * speed * dt;
            let mut saw = 0.0;
            for (k, ph) in p.phase.iter_mut().enumerate() {
                let detune = 1.0 + (k as f32 - 1.0) * 0.006;
                *ph = (*ph + p.freq * detune * dt).fract();
                saw += *ph * 2.0 - 1.0;
            }
            p.lp += (saw / 3.0 - p.lp) * cutoff;
            out += p.lp * p.env * 0.16;
        }
        self.pads.retain(|p| p.on || p.env > 1e-3);
        // The arpeggio: eighths up and down the chord, an octave up; two cycles of
        // the chords on, one off (fading), and out of the way under tension.
        let cycle = (self.played / CHORDS.len()) % 3;
        let arp = self.arp.next(dt * 0.25);
        self.arp.target = if cycle < 2 && self.played > 0 { 1.0 - tension } else { 0.0 };
        self.step_t += dt;
        if self.step_t >= 60.0 / TEMPO / 2.0 {
            self.step_t = 0.0;
            let k = PATTERN[self.step % PATTERN.len()];
            let n = if k == 4 { notes[0] as f32 + 24.0 } else { notes[k] as f32 + 12.0 };
            // (Accents on the beat; now and then a rest.)
            let vol = if self.step % 2 == 0 { 1.0 } else { 0.7 };
            if self.rand() > 0.08 && arp > 0.01 {
                self.plucks.push(Pluck { freq: midi(n), t: 0.0, phase: 0.0, vol: vol * arp });
            }
            // A soft pulse on the root each beat.
            if self.step % 2 == 0 {
                self.pulse_env = 1.0;
            }
            self.step += 1;
        }
        let mut dry = 0.0;
        for p in &mut self.plucks {
            p.phase = (p.phase + p.freq * dt).fract();
            let v = (p.phase * TAU).sin() + 0.25 * (p.phase * 2.0 * TAU).sin();
            dry += v * (-p.t / 0.32).exp() * (p.t / 0.006).min(1.0) * p.vol * 0.035;
            p.t += dt;
        }
        self.plucks.retain(|p| p.t < 2.0);
        // Through a long echo (a dotted eighth, fading over a few repeats).
        let len = ((60.0 / TEMPO * 0.75) * rate) as usize;
        if self.echo.len() != len {
            self.echo = vec![0.0; len.max(1)];
            self.echo_at = 0;
        }
        let back = self.echo[self.echo_at];
        self.echo[self.echo_at] = dry + back * 0.42;
        self.echo_at = (self.echo_at + 1) % self.echo.len();
        out += dry + back * 0.55;
        // The pulse: the root low and round, swelling and easing with each beat.
        self.pulse_env *= 1.0 - dt / 0.35;
        self.pulse_phase = (self.pulse_phase + midi(notes[0] as f32 - 12.0) * dt).fract();
        out += (self.pulse_phase * TAU).sin() * self.pulse_env * (1.0 - self.pulse_env).max(0.0) * 4.0 * 0.05 * (1.0 - tension);
        // A bell now and then, from the chord an octave or two up (fewer when tense).
        self.next_bell -= dt;
        if self.next_bell <= 0.0 {
            let n = notes[(self.rand() * 4.0) as usize % 4] as f32 + if self.rand() > 0.5 { 12.0 } else { 24.0 };
            let vol = 0.5 + 0.5 * self.rand();
            self.bells.push(Bell { freq: midi(n), t: 0.0, carrier: 0.0, modulator: 0.0, vol });
            self.next_bell = 2.5 + 4.0 * self.rand() + 6.0 * tension;
        }
        for b in &mut self.bells {
            b.modulator = (b.modulator + b.freq * 3.5 * dt).fract();
            let index = 0.8 * (-b.t * 3.0).exp();
            b.carrier = (b.carrier + b.freq * dt).fract();
            let v = (b.carrier * TAU + index * (b.modulator * TAU).sin()).sin();
            out += v * (-b.t / 1.8).exp() * (b.t / 0.06).min(1.0) * b.vol * 0.016;
            b.t += dt;
        }
        self.bells.retain(|b| b.t < 8.0);
        // Under tension, the bass: the chord's root, eighth notes at 96 a minute.
        if tension > 0.02 {
            self.beat += dt;
            if self.beat >= 60.0 / 96.0 / 2.0 {
                self.beat = 0.0;
                self.bass_env = 1.0;
            }
            self.bass_env *= 1.0 - dt / 0.18;
            let root = midi(notes[0] as f32 - 12.0);
            self.bass_phase = (self.bass_phase + root * dt).fract();
            self.bass_lp += ((self.bass_phase * 2.0 - 1.0) - self.bass_lp) * 0.03;
            out += self.bass_lp * self.bass_env * tension * 0.35;
        }
        // (As loud overall as the old score: there, not in the way.)
        out * level * 1.7 * (1.0 + 0.6 * self.swell)
    }
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
            air: Smoothed::default(),
            air_bright: Smoothed::default(),
            gusty: false,
            gust: 0.5,
            gust_to: 0.5,
            air_lpf: (0.0, 0.0),
            air_low: 0.0,
            breath: Smoothed::default(),
            breath_t: 0.0,
            breath_f: (0.0, 0.0),
            distant: Smoothed::default(),
            distant_pan: Smoothed::default(),
            distant_lp: (0.0, 0.0),
            music: Music::new(),
            delay: 0.0,
        }
    }

    /// A one-shot in, after the delay set (see `Audio::after`).
    fn push(&mut self, v: Voice) {
        let v = if self.delay > 0.0 { Voice::Later { wait: self.delay, then: Box::new(v) } } else { v };
        self.voices.push(v);
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
            put((hum * 0.12 + self.air_lp * 0.22) * amb, 0.0, 0.3, &mut l, &mut r, &mut send);
        }

        // The thrusters: each a soft, low rush through its own band, the
        // valve's envelope easing open and closed (no sharp "pssht" on every
        // little correction); many at once no louder than a few.
        let firing = self.jets.iter().filter(|v| v.jet.level > 0.06).count().max(1) as f32;
        let share = 1.0 / firing.sqrt();
        for k in 0..self.jets.len() {
            let n = self.white();
            let v = &mut self.jets[k];
            let j = v.jet;
            let on = j.level > 0.06;
            if on && !v.open {
                v.tick = 1.0;
            }
            v.open = on;
            let target = if on { j.level.sqrt() } else { 0.0 };
            let rate_env = if target > v.env { 1.0 / (0.06 * rate) } else { 1.0 / (0.2 * rate) };
            v.env += (target - v.env) * rate_env;
            if v.env < 1e-4 && v.tick < 1e-4 {
                continue;
            }
            // (Each jet a little different; lift jets bigger and lower; nearer the cabin, brighter.)
            let spread = 1.0 + 0.12 * (((v.seed * 2_654_435_761) >> 24) as f32 / 255.0 - 0.5);
            let centre = if j.lift { 320.0 } else { 450.0 + 250.0 * j.near } * spread;
            svf(&mut v.low, &mut v.band, n, centre, 0.7, rate);
            v.rush += (n - v.rush) * 0.02;
            // (The small thrusters quiet beside the lift jets: they are small.)
            let gain = (0.35 + 0.65 * j.near) * if j.lift { 0.5 } else { 0.05 };
            let mut s = (v.band * 0.3 + v.rush * 1.0) * v.env * gain * share;
            // The valve: a soft knock as it opens.
            if v.tick > 1e-4 {
                s += v.tick * (v.low * 2.0).clamp(-1.0, 1.0) * 0.06;
                v.tick *= 1.0 - 1.0 / (0.004 * rate);
            }
            put(s, j.pan * 0.85, 0.25, &mut l, &mut r, &mut send);
        }

        // Air: over the hull a steady rush, brighter the faster; outdoors,
        // wind rising and falling in gusts.
        let air = self.air.next(glide * 0.5);
        let bright = self.air_bright.next(glide * 0.5);
        if air > 1e-4 {
            let n = self.white();
            let mut level = air;
            if self.gusty {
                // (A slow random walk between gusts.)
                if (self.gust - self.gust_to).abs() < 0.01 {
                    self.gust_to = 0.2 + 0.8 * self.white().abs();
                }
                self.gust += (self.gust_to - self.gust) * 0.4 * dt;
                level *= self.gust;
            }
            let cutoff = 150.0 + 2500.0 * bright + if self.gusty { 400.0 * self.gust } else { 0.0 };
            svf(&mut self.air_lpf.0, &mut self.air_lpf.1, n, cutoff, 0.8, rate);
            self.air_low += (n - self.air_low) * 0.01;
            let v = (self.air_lpf.0 * 1.5 + self.air_low * 5.0) * level;
            put(v, 0.0, 0.1, &mut l, &mut r, &mut send);
        }

        // Breathing in a suit: in (rising), a pause, out (falling), and again.
        let breath = self.breath.next(glide * 0.5);
        if breath > 1e-4 {
            let n = self.white();
            self.breath_t = (self.breath_t + dt) % 4.2;
            let t = self.breath_t;
            let (env, freq) = if t < 1.3 {
                ((t / 1.3 * PI).sin(), 900.0 + 500.0 * t)
            } else if t > 1.7 && t < 3.6 {
                (((t - 1.7) / 1.9 * PI).sin() * 0.8, 800.0 - 150.0 * (t - 1.7))
            } else {
                (0.0, 800.0)
            };
            svf(&mut self.breath_f.0, &mut self.breath_f.1, n, freq, 2.0, rate);
            put(self.breath_f.1 * env * env * breath * 0.8, 0.0, 0.05, &mut l, &mut r, &mut send);
        }

        // Others' engines through the air: a far, low roar.
        let distant = self.distant.next(glide * 0.3);
        let dpan = self.distant_pan.next(glide * 0.3);
        if distant > 1e-4 {
            let n = self.white();
            self.distant_lp.0 += (n - self.distant_lp.0) * 0.02;
            self.distant_lp.1 += (self.distant_lp.0 - self.distant_lp.1) * 0.02;
            put(self.distant_lp.1 * 6.0 * distant, dpan, 0.05, &mut l, &mut r, &mut send);
        }

        // The score: wide (a little of it each side late through the room).
        let m = self.music.sample(rate);
        if m != 0.0 {
            put(m, 0.0, 0.7, &mut l, &mut r, &mut send);
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
                Voice::Motor { t, dur, vol, phase, lp } => {
                    // (A buzzy saw, spinning up to speed, through a low-pass.)
                    let k = *t / *dur;
                    let f = 70.0 + 50.0 * (*t / 0.15).min(1.0);
                    *phase = (*phase + f * dt).fract();
                    *lp += ((*phase * 2.0 - 1.0) + 0.3 * noise - *lp) * 0.15;
                    let env = (*t / 0.03).min(1.0) * ((1.0 - k) / 0.1).min(1.0);
                    *t += dt;
                    (*lp * env * *vol, 0.1, 0.3, *t >= *dur)
                }
                Voice::Fizz { t, dur, vol, env, hp, last } => {
                    // (Bubbles: tiny random clicks, high-passed, thinning out.)
                    let k = *t / *dur;
                    if noise.abs() > 0.97 + 0.025 * k {
                        *env = 0.5 + 0.5 * noise.abs();
                    }
                    *env *= 1.0 - 1.0 / (0.0015 * rate);
                    *hp = 0.7 * (*hp + noise - *last);
                    *last = noise;
                    *t += dt;
                    (*hp * *env * *vol * (1.0 - k), 0.1, 0.2, *t >= *dur)
                }
                Voice::Crunch { t, vol, env, next, low, band } => {
                    // (A bite: dense brittle cracks for a fifth of a second, then crumbs.)
                    if *t >= *next {
                        *env = 0.5 + 0.5 * noise.abs();
                        *next = *t + if *t < 0.18 { 0.004 + 0.01 * noise.abs() } else { 0.03 + 0.05 * noise.abs() };
                    }
                    *env *= 1.0 - 1.0 / (0.003 * rate);
                    svf(low, band, noise, 1800.0, 0.8, rate);
                    let tail = if *t < 0.18 { 1.0 } else { (1.0 - (*t - 0.18) / 0.25).max(0.0) * 0.4 };
                    *t += dt;
                    ((*band + noise * 0.3) * *env * *vol * tail, 0.0, 0.15, *t >= 0.43)
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
