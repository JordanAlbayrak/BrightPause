// Break sounds are synthesised here and played natively, so they start with the break window.
// The web view's AudioContext stayed suspended until the next click or key press.

const RATE: u32 = 44_100;

fn notes(name: &str) -> Option<(&'static [(f32, f32)], f32, bool)> {
    match name {
        "chime" => Some((&[(660.0, 0.0), (880.0, 0.18)], 1.1, false)),
        "bell" => Some((&[(523.0, 0.0)], 2.2, false)),
        "soft" => Some((&[(392.0, 0.0), (494.0, 0.25)], 1.1, true)),
        _ => None,
    }
}

pub fn wav(name: &str, volume: f32) -> Option<Vec<u8>> {
    let (notes, tail, triangle) = notes(name)?;
    if volume <= 0.0 {
        return None;
    }
    let peak = 0.25 * volume.min(1.0);
    let floor = 0.0001f32;
    let end = notes.iter().map(|n| n.1).fold(0.0, f32::max) + tail + 0.05;
    let len = (end * RATE as f32) as usize;
    let mut mix = vec![0f32; len];
    for &(freq, at) in notes {
        for (i, s) in mix.iter_mut().enumerate() {
            let t = i as f32 / RATE as f32 - at;
            if t < 0.0 || t > tail + 0.05 {
                continue;
            }
            let gain = if t < 0.02 {
                floor * (peak / floor).powf(t / 0.02)
            } else if t < tail {
                peak * (floor / peak).powf((t - 0.02) / (tail - 0.02))
            } else {
                floor
            };
            let phase = (freq * t).fract();
            let wave = if triangle { 1.0 - 4.0 * (phase - 0.5).abs() } else { (phase * std::f32::consts::TAU).sin() };
            *s += wave * gain;
        }
    }
    let data_len = (len * 2) as u32;
    let mut out = Vec::with_capacity(44 + len * 2);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in mix {
        out.extend_from_slice(&((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16).to_le_bytes());
    }
    Some(out)
}

pub fn play(name: &str, volume: f32) {
    let Some(buf) = wav(name, volume) else { return };
    std::thread::spawn(move || {
        #[cfg(windows)]
        unsafe {
            use windows::core::PCWSTR;
            use windows::Win32::Media::Audio::{PlaySoundW, SND_MEMORY, SND_NODEFAULT, SND_SYNC};
            let _ = PlaySoundW(PCWSTR(buf.as_ptr() as *const u16), None, SND_MEMORY | SND_SYNC | SND_NODEFAULT);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_has_header_and_expected_length() {
        let w = wav("chime", 0.5).unwrap();
        assert_eq!(&w[0..4], b"RIFF");
        assert_eq!(&w[8..16], b"WAVEfmt ");
        let samples = (w.len() - 44) / 2;
        assert_eq!(samples, ((0.18 + 1.1 + 0.05) * RATE as f32) as usize);
        let loudest = w[44..].chunks(2).map(|c| i16::from_le_bytes([c[0], c[1]]).unsigned_abs()).max().unwrap();
        assert!(loudest > 3000 && loudest < 12000, "{loudest}");
    }

    #[test]
    fn silent_or_unknown_gives_nothing() {
        assert!(wav("none", 0.5).is_none());
        assert!(wav("chime", 0.0).is_none());
    }
}
