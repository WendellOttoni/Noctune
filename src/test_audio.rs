//! Generated, license-free PCM fixtures; no physical sound device is needed.
pub fn wav() -> Vec<u8> {
    let count = 8000u32;
    let mut out = Vec::new();
    out.extend(b"RIFF");
    out.extend((36 + count * 2).to_le_bytes());
    out.extend(b"WAVEfmt ");
    out.extend(16u32.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(8000u32.to_le_bytes());
    out.extend(16000u32.to_le_bytes());
    out.extend(2u16.to_le_bytes());
    out.extend(16u16.to_le_bytes());
    out.extend(b"data");
    out.extend((count * 2).to_le_bytes());
    for sample in 0..count {
        out.extend((sample as i16).to_le_bytes());
    }
    out
}
