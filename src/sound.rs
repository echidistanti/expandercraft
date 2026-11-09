//! The notification sound played after a substitution: a short two-tone chime, synthesized at startup.

use std::sync::OnceLock;

const SAMPLE_RATE: u32 = 22_050;

pub fn notification_wav() -> &'static [u8] {
    static WAV: OnceLock<Vec<u8>> = OnceLock::new();
    WAV.get_or_init(build_chime)
}

fn build_chime() -> Vec<u8> {
    // two short notes (E6 then A6) with an exponential decay
    let notes = [(1318.5_f32, 0.0_f32), (1760.0, 0.055)];
    let duration = 0.22_f32;
    let n = (SAMPLE_RATE as f32 * duration) as usize;
    let mut samples = vec![0f32; n];
    for (freq, start) in notes {
        let s0 = (start * SAMPLE_RATE as f32) as usize;
        for (i, sample) in samples.iter_mut().enumerate().skip(s0) {
            let t = (i - s0) as f32 / SAMPLE_RATE as f32;
            let attack = (t / 0.004).min(1.0);
            let env = attack * (-t * 28.0).exp();
            let v = (2.0 * std::f32::consts::PI * freq * t).sin() + 0.25 * (4.0 * std::f32::consts::PI * freq * t).sin();
            *sample += 0.22 * env * v;
        }
    }
    let data: Vec<u8> = samples.iter().flat_map(|s| ((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16).to_le_bytes()).collect();
    let mut wav = Vec::with_capacity(44 + data.len());
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&1u16.to_le_bytes()); // mono
    wav.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    wav.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&(data.len() as u32).to_le_bytes());
    wav.extend_from_slice(&data);
    wav
}

/// Play the notification sound, or the custom sound file if one is configured and exists.
pub fn play(custom: Option<&str>) {
    match custom.filter(|p| !p.is_empty() && std::path::Path::new(p).is_file()) {
        Some(path) => crate::platform::play_wav_file(path),
        None => crate::platform::play_wav_bytes(notification_wav()),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn wav_header() {
        let w = super::notification_wav();
        assert_eq!(&w[0..4], b"RIFF");
        assert_eq!(&w[8..12], b"WAVE");
        assert_eq!(u32::from_le_bytes(w[4..8].try_into().unwrap()) as usize, w.len() - 8);
    }
}
