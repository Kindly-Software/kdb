pub const ID_SIZE: usize = 12;
pub const MAX_TIME_MS: u64 = (1u64 << 48) - 1;
pub const MAX_COUNTER: u32 = (1u32 << 24) - 1;

pub fn pack(time_ms: u64, node_id: u16, counter: u32, class: u8) -> [u8; ID_SIZE] {
    debug_assert!(time_ms <= MAX_TIME_MS, "time_ms exceeds 48-bit field");
    debug_assert!(counter <= MAX_COUNTER, "counter exceeds 24-bit field");

    let mut bytes = [0u8; ID_SIZE];
    write_u48_be(&mut bytes[0..6], time_ms);
    write_u16_be(&mut bytes[6..8], node_id);
    write_u24_be(&mut bytes[8..11], counter);
    bytes[11] = class;
    bytes
}

pub fn write_u48_be(dst: &mut [u8], value: u64) {
    assert!(dst.len() >= 6, "destination must be at least 6 bytes");
    debug_assert!(value <= MAX_TIME_MS, "value does not fit in 48 bits");
    for (i, byte) in dst.iter_mut().take(6).enumerate() {
        *byte = ((value >> (8 * (5 - i))) & 0xFF) as u8;
    }
}

pub fn read_u48_be(src: &[u8]) -> u64 {
    assert!(src.len() >= 6, "source must be at least 6 bytes");
    let mut value = 0u64;
    for &byte in src.iter().take(6) {
        value = (value << 8) | byte as u64;
    }
    value
}

pub fn write_u24_be(dst: &mut [u8], value: u32) {
    assert!(dst.len() >= 3, "destination must be at least 3 bytes");
    debug_assert!(value <= MAX_COUNTER, "value does not fit in 24 bits");
    for (i, byte) in dst.iter_mut().take(3).enumerate() {
        *byte = ((value >> (8 * (2 - i))) & 0xFF) as u8;
    }
}

pub fn read_u24_be(src: &[u8]) -> u32 {
    assert!(src.len() >= 3, "source must be at least 3 bytes");
    let mut value = 0u32;
    for &byte in src.iter().take(3) {
        value = (value << 8) | byte as u32;
    }
    value
}

pub fn write_u16_be(dst: &mut [u8], value: u16) {
    assert!(dst.len() >= 2, "destination must be at least 2 bytes");
    dst[0] = (value >> 8) as u8;
    dst[1] = value as u8;
}

pub fn read_u16_be(src: &[u8]) -> u16 {
    assert!(src.len() >= 2, "source must be at least 2 bytes");
    ((src[0] as u16) << 8) | (src[1] as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_and_unpack_round_trip() {
        let bytes = pack(0x1234_5678_9ABC, 0xC0DE, 0x00FE_EDF0, 0x42);
        assert_eq!(read_u48_be(&bytes[0..6]), 0x1234_5678_9ABC);
        assert_eq!(read_u16_be(&bytes[6..8]), 0xC0DE);
        assert_eq!(read_u24_be(&bytes[8..11]), 0x00FE_EDF0);
        assert_eq!(bytes[11], 0x42);
    }

    #[test]
    fn read_write_u24_round_trip() {
        let mut buf = [0u8; 3];
        write_u24_be(&mut buf, 0x123456);
        assert_eq!(buf, [0x12, 0x34, 0x56]);
        assert_eq!(read_u24_be(&buf), 0x123456);
    }

    #[test]
    fn read_write_u48_round_trip() {
        let mut buf = [0u8; 6];
        write_u48_be(&mut buf, 0x00AA_BBCC_DDEE);
        assert_eq!(buf, [0x00, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE]);
        assert_eq!(read_u48_be(&buf), 0x00AA_BBCC_DDEE);
    }
}
