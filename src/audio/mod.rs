use kira::sound::static_sound::{StaticSoundData, StaticSoundHandle};
use kira::sound::Region;
use kira::{AudioManager, AudioManagerSettings, DefaultBackend};
use std::io::Cursor;

const SAMPLE_RATE: u32 = 44_100;

pub struct AudioSystem {
    manager: AudioManager,
    // Both effects are synthesized once at startup; kira clones are Arc-cheap,
    // so playing them per shot costs no allocation or WAV decoding.
    gunshot: StaticSoundData,
    explosion: StaticSoundData,
    hurt: StaticSoundData,
    rain: StaticSoundData,
    rain_handle: Option<StaticSoundHandle>,
    beep: StaticSoundData,
}

impl AudioSystem {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let manager = AudioManager::<DefaultBackend>::new(AudioManagerSettings::default())?;
        let gunshot = StaticSoundData::from_cursor(Cursor::new(pcm16_wav(&gunshot_samples())))?;
        let explosion = StaticSoundData::from_cursor(Cursor::new(pcm16_wav(&explosion_samples())))?;
        let hurt = StaticSoundData::from_cursor(Cursor::new(pcm16_wav(&hurt_samples())))?;
        let rain = StaticSoundData::from_cursor(Cursor::new(pcm16_wav(&rain_samples())))?;
        let beep = StaticSoundData::from_cursor(Cursor::new(pcm16_wav(&beep_samples())))?;
        Ok(Self {
            manager,
            gunshot,
            explosion,
            hurt,
            rain,
            rain_handle: None,
            beep,
        })
    }

    pub fn play_gunshot(&mut self) {
        let _ = self.manager.play(self.gunshot.clone());
    }

    pub fn play_explosion(&mut self) {
        let _ = self.manager.play(self.explosion.clone());
    }

    pub fn play_hurt(&mut self) {
        let _ = self.manager.play(self.hurt.clone());
    }

    pub fn play_beep(&mut self) {
        let _ = self.manager.play(self.beep.clone());
    }

    /// Start the looping rain ambience (no-op if already playing).
    pub fn start_rain_loop(&mut self) {
        if self.rain_handle.is_some() {
            return;
        }
        let sound = self.rain.clone().loop_region(Region::from(..));
        if let Ok(handle) = self.manager.play(sound) {
            self.rain_handle = Some(handle);
        }
    }

    pub fn stop_rain_loop(&mut self) {
        if let Some(mut handle) = self.rain_handle.take() {
            handle.stop(kira::Tween::default());
        }
    }
}

/// Short high-pitched armed-bomb beep: 1250 Hz sine, sharp decay.
fn beep_samples() -> Vec<i16> {
    let duration = 0.09;
    let count = (SAMPLE_RATE as f32 * duration) as usize;
    let mut samples = Vec::with_capacity(count);
    for i in 0..count {
        let t = i as f32 / SAMPLE_RATE as f32;
        let envelope = (1.0 - t / duration).powi(2);
        let value = (t * 1250.0 * std::f32::consts::TAU).sin() * envelope * 0.35;
        samples.push((value * i16::MAX as f32) as i16);
    }
    samples
}

/// Four seconds of rain ambience: pseudo-random noise shaped by overlapping
/// slow amplitude waves so the loop point is not audible.
fn rain_samples() -> Vec<i16> {
    let duration = 4.0;
    let count = (SAMPLE_RATE as f32 * duration) as usize;
    let mut samples = Vec::with_capacity(count);
    let mut noise_state = 0x1234_5678_u32;
    let mut low_pass = 0.0_f32;
    for i in 0..count {
        let t = i as f32 / SAMPLE_RATE as f32;
        noise_state = noise_state
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        let white = ((noise_state >> 8) as f32 / (u32::MAX >> 8) as f32) * 2.0 - 1.0;
        // Gentle low-pass turns hiss into patter; slow waves add gusts.
        low_pass += (white - low_pass) * 0.24;
        let gusts = 0.6
            + 0.25 * (t * 0.9 * std::f32::consts::TAU / duration * duration).sin()
            + 0.15 * (t * 2.3).sin();
        let value = low_pass * gusts * 0.28;
        samples.push((value.clamp(-1.0, 1.0) * i16::MAX as f32) as i16);
    }
    samples
}

/// Short dull "thump": a 110 Hz sine dropping to 70 Hz with fast decay.
fn hurt_samples() -> Vec<i16> {
    let duration = 0.16;
    let count = (SAMPLE_RATE as f32 * duration) as usize;
    let mut samples = Vec::with_capacity(count);
    for i in 0..count {
        let t = i as f32 / SAMPLE_RATE as f32;
        let progress = t / duration;
        let freq = 110.0 - 40.0 * progress;
        let envelope = (1.0 - progress).powi(2);
        let value = (t * freq * std::f32::consts::TAU).sin() * envelope * 0.6;
        samples.push((value * i16::MAX as f32) as i16);
    }
    samples
}

/// Wrap mono 16-bit PCM samples in a minimal WAV container.
fn pcm16_wav(samples: &[i16]) -> Vec<u8> {
    let num_channels: u16 = 1;
    let bits_per_sample: u16 = 16;
    let byte_rate = SAMPLE_RATE * num_channels as u32 * bits_per_sample as u32 / 8;
    let block_align = num_channels * bits_per_sample / 8;
    let data_size = (samples.len() * 2) as u32;

    let mut wav = Vec::with_capacity(44 + samples.len() * 2);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_size).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&num_channels.to_le_bytes());
    wav.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&block_align.to_le_bytes());
    wav.extend_from_slice(&bits_per_sample.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_size.to_le_bytes());
    for sample in samples {
        wav.extend_from_slice(&sample.to_le_bytes());
    }
    wav
}

/// Deterministic pseudo-random noise source (LCG) for procedural foley.
struct NoiseLcg(u32);

impl NoiseLcg {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

fn gunshot_samples() -> Vec<i16> {
    let duration = 0.12_f32;
    let num_samples = (SAMPLE_RATE as f32 * duration) as u32;
    let mut rng = NoiseLcg(12345);
    let mut samples = Vec::with_capacity(num_samples as usize);
    for i in 0..num_samples {
        let t = i as f32 / SAMPLE_RATE as f32;

        // Noise burst (attack)
        let noise = if t < 0.008 {
            rng.next() * (1.0 - t / 0.008)
        } else {
            rng.next() * 0.1 * (-((t - 0.008) * 30.0)).exp()
        };

        // Low frequency thump
        let thump = if t < 0.06 {
            (t * 150.0 * std::f32::consts::TAU).sin() * (1.0 - t / 0.06) * 0.7
        } else {
            0.0
        };

        // Click/snap
        let click = if t < 0.003 {
            (t * 3000.0 * std::f32::consts::TAU).sin() * (1.0 - t / 0.003) * 0.4
        } else {
            0.0
        };

        let envelope = (-t * 15.0).exp();
        let sample = (noise + thump + click) * envelope;
        samples.push((sample.clamp(-1.0, 1.0) * 32_000.0) as i16);
    }
    samples
}

fn explosion_samples() -> Vec<i16> {
    let duration = 0.55_f32;
    let num_samples = (SAMPLE_RATE as f32 * duration) as u32;
    let mut rng = NoiseLcg(0xC0FFEE);
    let mut samples = Vec::with_capacity(num_samples as usize);
    for i in 0..num_samples {
        let noise = rng.next();
        let t = i as f32 / SAMPLE_RATE as f32;
        let envelope = (-t * 6.2).exp();
        let sub = (t * 54.0 * std::f32::consts::TAU).sin() * (-t * 4.0).exp();
        let crack = if t < 0.025 {
            noise * (1.0 - t / 0.025)
        } else {
            noise * 0.35
        };
        let sample = (sub * 0.78 + crack * 0.62) * envelope;
        samples.push((sample.clamp(-1.0, 1.0) * 31_000.0) as i16);
    }
    samples
}
