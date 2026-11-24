pub fn obfuscate_font(data: &mut [u8], guid: &[u8; 16]) {
    for i in 0..32.min(data.len()) {
        data[i] ^= guid[15 - (i % 16)];
    }
}

#[cfg(test)]
pub fn deobfuscate_font(data: &mut [u8], guid: &[u8; 16]) {
    obfuscate_font(data, guid);
}

pub fn font_guid_key(n: usize) -> [u8; 16] {
    let mut key = [0u8; 16];
    let seed = 0xA7C9_E1B3u64
        .wrapping_add(n as u64)
        .wrapping_mul(0x9E37_79B9);
    key[0..8].copy_from_slice(&seed.to_le_bytes());
    key[8..16].copy_from_slice(&seed.wrapping_mul(0x85EB_CA6B).to_le_bytes());
    key
}
