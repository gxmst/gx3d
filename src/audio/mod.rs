use kira::sound::static_sound::StaticSoundData;
use kira::{AudioManager, AudioManagerSettings, DefaultBackend};
use std::io::Cursor;

pub struct AudioSystem {
    pub manager: AudioManager,
}

impl AudioSystem {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let manager = AudioManager::<DefaultBackend>::new(AudioManagerSettings::default())?;
        Ok(Self { manager })
    }

    pub fn play_sound(
        &mut self,
        sound_data: StaticSoundData,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let _ = self.manager.play(sound_data)?;
        Ok(())
    }

    pub fn create_gunshot() -> StaticSoundData {
        // Generate a simple procedural gunshot as WAV bytes
        let sample_rate: u32 = 44100;
        let duration = 0.12_f32;
        let num_samples = (sample_rate as f32 * duration) as u32;
        let num_channels: u16 = 1;
        let bits_per_sample: u16 = 16;
        let byte_rate = sample_rate * num_channels as u32 * bits_per_sample as u32 / 8;
        let block_align = num_channels * bits_per_sample / 8;
        let data_size = num_samples * num_channels as u32 * bits_per_sample as u32 / 8;

        let mut wav_data = Vec::new();

        // WAV header
        wav_data.extend_from_slice(b"RIFF");
        wav_data.extend_from_slice(&(36 + data_size).to_le_bytes());
        wav_data.extend_from_slice(b"WAVE");
        wav_data.extend_from_slice(b"fmt ");
        wav_data.extend_from_slice(&16u32.to_le_bytes()); // chunk size
        wav_data.extend_from_slice(&1u16.to_le_bytes()); // PCM
        wav_data.extend_from_slice(&num_channels.to_le_bytes());
        wav_data.extend_from_slice(&sample_rate.to_le_bytes());
        wav_data.extend_from_slice(&byte_rate.to_le_bytes());
        wav_data.extend_from_slice(&block_align.to_le_bytes());
        wav_data.extend_from_slice(&bits_per_sample.to_le_bytes());
        wav_data.extend_from_slice(b"data");
        wav_data.extend_from_slice(&data_size.to_le_bytes());

        // Simple deterministic pseudo-random using LCG
        let mut rng_state: u32 = 12345;
        let mut next_random = || -> f32 {
            rng_state = rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
            (rng_state as f32 / u32::MAX as f32) * 2.0 - 1.0
        };

        // Generate samples
        for i in 0..num_samples {
            let t = i as f32 / sample_rate as f32;

            // Noise burst (attack)
            let noise = if t < 0.008 {
                next_random() * (1.0 - t / 0.008)
            } else {
                next_random() * 0.1 * (-((t - 0.008) * 30.0)).exp()
            };

            // Low frequency thump
            let thump = if t < 0.06 {
                (t * 150.0 * std::f32::consts::PI * 2.0).sin() * (1.0 - t / 0.06) * 0.7
            } else {
                0.0
            };

            // Click/snap
            let click = if t < 0.003 {
                (t * 3000.0 * std::f32::consts::PI * 2.0).sin() * (1.0 - t / 0.003) * 0.4
            } else {
                0.0
            };

            let envelope = (-t * 15.0).exp();
            let sample = (noise + thump + click) * envelope;
            let sample_i16 = (sample.clamp(-1.0, 1.0) * 32000.0) as i16;
            wav_data.extend_from_slice(&sample_i16.to_le_bytes());
        }

        StaticSoundData::from_cursor(Cursor::new(wav_data)).expect("Failed to create gunshot sound")
    }

    pub fn create_explosion() -> StaticSoundData {
        let sample_rate: u32 = 44_100;
        let duration = 0.55_f32;
        let num_samples = (sample_rate as f32 * duration) as u32;
        let num_channels: u16 = 1;
        let bits_per_sample: u16 = 16;
        let byte_rate = sample_rate * num_channels as u32 * bits_per_sample as u32 / 8;
        let block_align = num_channels * bits_per_sample / 8;
        let data_size = num_samples * num_channels as u32 * bits_per_sample as u32 / 8;
        let mut wav_data = Vec::with_capacity(data_size as usize + 44);
        wav_data.extend_from_slice(b"RIFF");
        wav_data.extend_from_slice(&(36 + data_size).to_le_bytes());
        wav_data.extend_from_slice(b"WAVEfmt ");
        wav_data.extend_from_slice(&16u32.to_le_bytes());
        wav_data.extend_from_slice(&1u16.to_le_bytes());
        wav_data.extend_from_slice(&num_channels.to_le_bytes());
        wav_data.extend_from_slice(&sample_rate.to_le_bytes());
        wav_data.extend_from_slice(&byte_rate.to_le_bytes());
        wav_data.extend_from_slice(&block_align.to_le_bytes());
        wav_data.extend_from_slice(&bits_per_sample.to_le_bytes());
        wav_data.extend_from_slice(b"data");
        wav_data.extend_from_slice(&data_size.to_le_bytes());

        let mut rng_state: u32 = 0xC0FFEE;
        for i in 0..num_samples {
            rng_state = rng_state
                .wrapping_mul(1_664_525)
                .wrapping_add(1_013_904_223);
            let noise = (rng_state as f32 / u32::MAX as f32) * 2.0 - 1.0;
            let t = i as f32 / sample_rate as f32;
            let envelope = (-t * 6.2).exp();
            let sub = (t * 54.0 * std::f32::consts::TAU).sin() * (-t * 4.0).exp();
            let crack = if t < 0.025 {
                noise * (1.0 - t / 0.025)
            } else {
                noise * 0.35
            };
            let sample = (sub * 0.78 + crack * 0.62) * envelope;
            wav_data
                .extend_from_slice(&((sample.clamp(-1.0, 1.0) * 31_000.0) as i16).to_le_bytes());
        }
        StaticSoundData::from_cursor(Cursor::new(wav_data))
            .expect("Failed to create explosion sound")
    }
}
