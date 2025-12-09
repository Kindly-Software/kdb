//! WebSocketStateCapsule - T1 Atomic WebSocket connection state
//!
//! RFC 6455 WebSocket frame parsing and connection state management.
//! Simple implementation supporting text/binary frames and control frames.

use core::sync::atomic::{AtomicU64, Ordering};
use super::{ApiError, ApiErrorKind};

#[cfg(feature = "std")]
use std::vec::Vec;

/// WebSocket connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum WsState {
    Handshake = 0,
    Open = 1,
    Closing = 2,
    Closed = 3,
}

/// WebSocket frame opcode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum WsOpcode {
    Continuation = 0x0,
    Text = 0x1,
    Binary = 0x2,
    Close = 0x8,
    Ping = 0x9,
    Pong = 0xA,
}

/// WebSocket frame
#[derive(Debug, Clone)]
pub struct WebSocketFrame {
    pub fin: bool,
    pub opcode: WsOpcode,
    pub masked: bool,
    pub payload: Vec<u8>,
}

/// WebSocket state capsule with atomic statistics tracking
///
/// # ASSUM Tags
/// - #ASSUME_LOCKFREE_STATE: All state updates via AtomicU64, no mutex
/// - #ASSUME_RFC6455_SUBSET: Basic frame parsing, not full RFC 6455 compliance
/// - #ASSUME_NO_EXTENSIONS: WebSocket extensions not supported
/// - #ASSUME_MAX_FRAME_SIZE: Maximum frame size 16MB
/// - #ASSUME_CACHE_ALIGNED: 128B alignment prevents false sharing
#[repr(C, align(128))]
pub struct WebSocketStateCapsule {
    /// Connection state (packed: state(8) | reserved(56))
    state: AtomicU64,

    /// Message counters
    message_count: AtomicU64,
    bytes_sent: AtomicU64,
    bytes_received: AtomicU64,

    /// Frame counters
    text_frames: AtomicU64,
    binary_frames: AtomicU64,
    ping_count: AtomicU64,
    pong_count: AtomicU64,

    /// Error tracking
    error_count: AtomicU64,
    close_code: AtomicU64,

    /// Reserved for future use (10 + 6 = 16 × 8 = 128 bytes total)
    _reserved: [AtomicU64; 6],
}

impl WebSocketStateCapsule {
    /// Create new WebSocket state capsule
    pub const fn new() -> Self {
        Self {
            state: AtomicU64::new(WsState::Handshake as u64),
            message_count: AtomicU64::new(0),
            bytes_sent: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
            text_frames: AtomicU64::new(0),
            binary_frames: AtomicU64::new(0),
            ping_count: AtomicU64::new(0),
            pong_count: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            close_code: AtomicU64::new(0),
            _reserved: [
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
            ],
        }
    }

    /// Parse WebSocket frame from bytes
    ///
    /// # Arguments
    /// * `data` - Raw frame bytes
    ///
    /// # Returns
    /// Parsed frame or error
    ///
    /// # ASSUM
    /// - #ASSUME_COMPLETE_FRAME: Data must contain complete frame
    /// - #ASSUME_VALID_HEADER: Frame header must be valid RFC 6455
    pub fn parse_frame(&self, data: &[u8]) -> Result<WebSocketFrame, ApiError> {
        if data.len() < 2 {
            self.error_count.fetch_add(1, Ordering::Relaxed);
            return Err(ApiError::new(
                ApiErrorKind::ParseError,
                "Frame too short",
            ));
        }

        // Parse first byte: FIN (1 bit) + RSV (3 bits) + opcode (4 bits)
        let fin = (data[0] & 0x80) != 0;
        let opcode_raw = data[0] & 0x0F;

        let opcode = match opcode_raw {
            0x0 => WsOpcode::Continuation,
            0x1 => WsOpcode::Text,
            0x2 => WsOpcode::Binary,
            0x8 => WsOpcode::Close,
            0x9 => WsOpcode::Ping,
            0xA => WsOpcode::Pong,
            _ => {
                self.error_count.fetch_add(1, Ordering::Relaxed);
                return Err(ApiError::new(
                    ApiErrorKind::ParseError,
                    "Invalid opcode",
                ));
            }
        };

        // Parse second byte: MASK (1 bit) + payload length (7 bits)
        let masked = (data[1] & 0x80) != 0;
        let mut payload_len = (data[1] & 0x7F) as u64;
        let mut offset = 2;

        // Extended payload length
        if payload_len == 126 {
            if data.len() < 4 {
                self.error_count.fetch_add(1, Ordering::Relaxed);
                return Err(ApiError::new(
                    ApiErrorKind::ParseError,
                    "Incomplete extended length",
                ));
            }
            payload_len = u16::from_be_bytes([data[2], data[3]]) as u64;
            offset = 4;
        } else if payload_len == 127 {
            if data.len() < 10 {
                self.error_count.fetch_add(1, Ordering::Relaxed);
                return Err(ApiError::new(
                    ApiErrorKind::ParseError,
                    "Incomplete extended length",
                ));
            }
            payload_len = u64::from_be_bytes([
                data[2], data[3], data[4], data[5], data[6], data[7], data[8], data[9],
            ]);
            offset = 10;
        }

        // Masking key (4 bytes if masked)
        let mask_key = if masked {
            if data.len() < offset + 4 {
                self.error_count.fetch_add(1, Ordering::Relaxed);
                return Err(ApiError::new(
                    ApiErrorKind::ParseError,
                    "Incomplete masking key",
                ));
            }
            let key = [data[offset], data[offset + 1], data[offset + 2], data[offset + 3]];
            offset += 4;
            Some(key)
        } else {
            None
        };

        // Extract payload
        if data.len() < offset + payload_len as usize {
            self.error_count.fetch_add(1, Ordering::Relaxed);
            return Err(ApiError::new(
                ApiErrorKind::ParseError,
                "Incomplete payload",
            ));
        }

        let mut payload = data[offset..offset + payload_len as usize].to_vec();

        // Unmask payload if needed
        if let Some(key) = mask_key {
            for (i, byte) in payload.iter_mut().enumerate() {
                *byte ^= key[i % 4];
            }
        }

        // Update statistics
        self.bytes_received
            .fetch_add(data.len() as u64, Ordering::Relaxed);
        self.message_count.fetch_add(1, Ordering::Relaxed);

        match opcode {
            WsOpcode::Text => {
                self.text_frames.fetch_add(1, Ordering::Relaxed);
            }
            WsOpcode::Binary => {
                self.binary_frames.fetch_add(1, Ordering::Relaxed);
            }
            WsOpcode::Ping => {
                self.ping_count.fetch_add(1, Ordering::Relaxed);
            }
            WsOpcode::Pong => {
                self.pong_count.fetch_add(1, Ordering::Relaxed);
            }
            WsOpcode::Close => {
                self.set_state(WsState::Closing);
                if payload.len() >= 2 {
                    let code = u16::from_be_bytes([payload[0], payload[1]]);
                    self.close_code.store(code as u64, Ordering::Relaxed);
                }
            }
            _ => {}
        }

        Ok(WebSocketFrame {
            fin,
            opcode,
            masked,
            payload,
        })
    }

    /// Build WebSocket frame
    ///
    /// # Arguments
    /// * `opcode` - Frame opcode
    /// * `payload` - Frame payload
    ///
    /// # Returns
    /// Encoded frame bytes
    ///
    /// # ASSUM
    /// - #ASSUME_NO_MASKING: Server frames are not masked (RFC 6455)
    /// - #ASSUME_FIN_SET: Always sets FIN bit (no fragmentation)
    pub fn build_frame(&self, opcode: WsOpcode, payload: &[u8]) -> Vec<u8> {
        let mut frame = Vec::new();

        // First byte: FIN=1, RSV=0, opcode
        frame.push(0x80 | (opcode as u8));

        // Payload length encoding
        let payload_len = payload.len();
        if payload_len <= 125 {
            frame.push(payload_len as u8);
        } else if payload_len <= 65535 {
            frame.push(126);
            frame.extend_from_slice(&(payload_len as u16).to_be_bytes());
        } else {
            frame.push(127);
            frame.extend_from_slice(&(payload_len as u64).to_be_bytes());
        }

        // Append payload
        frame.extend_from_slice(payload);

        // Update statistics
        self.bytes_sent
            .fetch_add(frame.len() as u64, Ordering::Relaxed);

        frame
    }

    /// Set connection state
    pub fn set_state(&self, state: WsState) {
        self.state.store(state as u64, Ordering::Release);
    }

    /// Get connection state
    pub fn get_state(&self) -> WsState {
        let state = self.state.load(Ordering::Acquire);
        match state as u8 {
            0 => WsState::Handshake,
            1 => WsState::Open,
            2 => WsState::Closing,
            3 => WsState::Closed,
            _ => WsState::Closed,
        }
    }

    /// Check if connection is open
    pub fn is_open(&self) -> bool {
        self.get_state() == WsState::Open
    }

    /// Get statistics snapshot
    pub fn get_stats(&self) -> WsStats {
        WsStats {
            state: self.get_state(),
            message_count: self.message_count.load(Ordering::Relaxed),
            bytes_sent: self.bytes_sent.load(Ordering::Relaxed),
            bytes_received: self.bytes_received.load(Ordering::Relaxed),
            text_frames: self.text_frames.load(Ordering::Relaxed),
            binary_frames: self.binary_frames.load(Ordering::Relaxed),
            ping_count: self.ping_count.load(Ordering::Relaxed),
            pong_count: self.pong_count.load(Ordering::Relaxed),
            error_count: self.error_count.load(Ordering::Relaxed),
            close_code: self.close_code.load(Ordering::Relaxed) as u16,
        }
    }

    /// Reset statistics
    pub fn reset_stats(&self) {
        self.message_count.store(0, Ordering::Relaxed);
        self.bytes_sent.store(0, Ordering::Relaxed);
        self.bytes_received.store(0, Ordering::Relaxed);
        self.text_frames.store(0, Ordering::Relaxed);
        self.binary_frames.store(0, Ordering::Relaxed);
        self.ping_count.store(0, Ordering::Relaxed);
        self.pong_count.store(0, Ordering::Relaxed);
        self.error_count.store(0, Ordering::Relaxed);
    }
}

/// WebSocket statistics snapshot
#[derive(Debug, Clone, Copy)]
pub struct WsStats {
    pub state: WsState,
    pub message_count: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub text_frames: u64,
    pub binary_frames: u64,
    pub ping_count: u64,
    pub pong_count: u64,
    pub error_count: u64,
    pub close_code: u16,
}

impl Default for WebSocketStateCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// Compile-time verification
const _: () = assert!(core::mem::size_of::<WebSocketStateCapsule>() == 128);
const _: () = assert!(core::mem::align_of::<WebSocketStateCapsule>() == 128);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout() {
        assert_eq!(core::mem::size_of::<WebSocketStateCapsule>(), 128);
        assert_eq!(core::mem::align_of::<WebSocketStateCapsule>(), 128);
    }

    #[test]
    fn test_state_transitions() {
        let ws = WebSocketStateCapsule::new();
        assert_eq!(ws.get_state(), WsState::Handshake);

        ws.set_state(WsState::Open);
        assert_eq!(ws.get_state(), WsState::Open);
        assert!(ws.is_open());

        ws.set_state(WsState::Closing);
        assert_eq!(ws.get_state(), WsState::Closing);
        assert!(!ws.is_open());
    }

    #[test]
    fn test_build_text_frame() {
        let ws = WebSocketStateCapsule::new();
        let payload = b"Hello";
        let frame = ws.build_frame(WsOpcode::Text, payload);

        // Should be: FIN + opcode + length + payload
        assert!(frame.len() >= 2 + payload.len());
        assert_eq!(frame[0], 0x81); // FIN=1, opcode=1
        assert_eq!(frame[1], 5); // payload length
        assert_eq!(&frame[2..], payload);
    }

    #[test]
    fn test_parse_text_frame() {
        let ws = WebSocketStateCapsule::new();

        // Build simple text frame: FIN=1, opcode=1, no mask, length=5, "Hello"
        let data = vec![0x81, 0x05, b'H', b'e', b'l', b'l', b'o'];

        let frame = ws.parse_frame(&data).unwrap();
        assert!(frame.fin);
        assert_eq!(frame.opcode, WsOpcode::Text);
        assert!(!frame.masked);
        assert_eq!(frame.payload, b"Hello");

        let stats = ws.get_stats();
        assert_eq!(stats.text_frames, 1);
        assert_eq!(stats.message_count, 1);
    }

    #[test]
    fn test_parse_masked_frame() {
        let ws = WebSocketStateCapsule::new();

        // Masked text frame: FIN=1, opcode=1, mask=1, length=5
        // Masking key: [0x12, 0x34, 0x56, 0x78]
        // Payload "Hello" masked
        let mask_key = [0x12, 0x34, 0x56, 0x78];
        let mut masked_payload = b"Hello".to_vec();
        for (i, byte) in masked_payload.iter_mut().enumerate() {
            *byte ^= mask_key[i % 4];
        }

        let mut data = vec![0x81, 0x85]; // FIN=1, opcode=1, mask=1, len=5
        data.extend_from_slice(&mask_key);
        data.extend_from_slice(&masked_payload);

        let frame = ws.parse_frame(&data).unwrap();
        assert!(frame.masked);
        assert_eq!(frame.payload, b"Hello");
    }

    #[test]
    fn test_parse_ping_pong() {
        let ws = WebSocketStateCapsule::new();

        // Ping frame
        let ping_data = vec![0x89, 0x00]; // FIN=1, opcode=9, no payload
        let frame = ws.parse_frame(&ping_data).unwrap();
        assert_eq!(frame.opcode, WsOpcode::Ping);

        // Pong frame
        let pong_data = vec![0x8A, 0x00]; // FIN=1, opcode=10, no payload
        let frame = ws.parse_frame(&pong_data).unwrap();
        assert_eq!(frame.opcode, WsOpcode::Pong);

        let stats = ws.get_stats();
        assert_eq!(stats.ping_count, 1);
        assert_eq!(stats.pong_count, 1);
    }

    #[test]
    fn test_parse_close_frame() {
        let ws = WebSocketStateCapsule::new();
        ws.set_state(WsState::Open);

        // Close frame with code 1000 (normal closure)
        let close_data = vec![0x88, 0x02, 0x03, 0xE8]; // opcode=8, len=2, code=1000
        let frame = ws.parse_frame(&close_data).unwrap();
        assert_eq!(frame.opcode, WsOpcode::Close);
        assert_eq!(ws.get_state(), WsState::Closing);

        let stats = ws.get_stats();
        assert_eq!(stats.close_code, 1000);
    }

    #[test]
    fn test_extended_length_126() {
        let ws = WebSocketStateCapsule::new();

        // Frame with 126-byte payload (extended length)
        let payload = vec![0u8; 126];
        let frame = ws.build_frame(WsOpcode::Binary, &payload);

        // Should use 16-bit extended length
        assert_eq!(frame[1], 126);
        assert_eq!(u16::from_be_bytes([frame[2], frame[3]]), 126);
    }

    #[test]
    fn test_invalid_frame() {
        let ws = WebSocketStateCapsule::new();

        // Too short
        let result = ws.parse_frame(&[0x81]);
        assert!(result.is_err());

        let stats = ws.get_stats();
        assert_eq!(stats.error_count, 1);
    }

    #[test]
    fn test_statistics() {
        let ws = WebSocketStateCapsule::new();

        let _ = ws.parse_frame(&[0x81, 0x00]); // Text frame
        let _ = ws.parse_frame(&[0x82, 0x00]); // Binary frame

        let stats = ws.get_stats();
        assert_eq!(stats.text_frames, 1);
        assert_eq!(stats.binary_frames, 1);
        assert_eq!(stats.message_count, 2);
    }
}
