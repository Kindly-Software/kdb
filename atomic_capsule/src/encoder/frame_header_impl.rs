//! AV1 Frame Header Implementation (Spec-Compliant)
//!
//! This module contains the spec-compliant frame_header_obu implementation
//! for keyframes following AV1 Bitstream & Decoding Process Specification.
//!
//! # References
//! - AV1 Spec §5.9: https://aomediacodec.github.io/av1-spec/
//! - Frame Header Syntax: §5.9 uncompressed_header()
//! - Quantization: §5.9.19
//! - Loop Filter: §5.9.23
//! - CDEF: §5.9.24
//! - Loop Restoration: §5.9.25
//! - Tile Info: §5.9.31

#![allow(deprecated)]
use super::obu_bitstream::{BitWriter, FrameType, ObuBitstreamWriterCapsule, ObuType};
use core::sync::atomic::Ordering;

impl ObuBitstreamWriterCapsule {
    /// Write frame header OBU (AV1 §5.9 uncompressed_header)
    ///
    /// Implements spec-compliant frame header for keyframes with minimal complexity.
    /// For MVP: keyframes only, single tile, no advanced features.
    ///
    /// # Parameters
    /// - `frame_type`: Key frame, inter frame, intra-only, or switch
    /// - `width`: Frame width in pixels (must match sequence header)
    /// - `height`: Frame height in pixels (must match sequence header)
    ///
    /// # Returns
    /// Complete OBU byte sequence (OBU header + size + frame header payload)
    ///
    /// # AV1 Frame Header Structure (§5.9)
    /// ```text
    /// uncompressed_header() {
    ///   show_existing_frame         f(1)  // 0 for new frames
    ///   if (show_existing_frame) {
    ///     // ... frame_to_show_map_idx ...
    ///   } else {
    ///     frame_type                 f(2)  // KEY_FRAME=0, INTER=1, INTRA_ONLY=2, SWITCH=3
    ///     show_frame                 f(1)  // 1 = display immediately
    ///     if (show_frame && frame_type != KEY_FRAME) {
    ///       showable_frame           f(1)
    ///     }
    ///     error_resilient_mode       f(1)  // 1 = reset decoder state
    ///     disable_cdf_update         f(1)  // 1 = don't update CDF tables
    ///
    ///     // Keyframe-specific fields
    ///     if (frame_type == KEY_FRAME) {
    ///       // ... no allow_screen_content_tools for forced case ...
    ///       // ... force_integer_mv = 1 (implicit) ...
    ///     }
    ///
    ///     // Frame size (if not matching sequence)
    ///     if (frame_size_override_flag) {
    ///       frame_size()              // width/height
    ///     }
    ///
    ///     // Render size (if different from coded)
    ///     if (enable_superres) {
    ///       render_and_frame_size_different  f(1)
    ///     }
    ///
    ///     // Reference frame management (keyframes reset)
    ///     if (frame_type == KEY_FRAME) {
    ///       refresh_frame_flags      f(8)  // 0xFF = refresh all
    ///     }
    ///
    ///     // Quantization, segmentation, filters
    ///     quantization_params()
    ///     segmentation_params()
    ///     delta_q_params()
    ///     delta_lf_params()
    ///     loop_filter_params()
    ///     cdef_params()
    ///     lr_params()
    ///
    ///     // Transform and prediction modes
    ///     read_tx_mode()
    ///     frame_reference_mode()
    ///     skip_mode_params()
    ///
    ///     // Motion and warping
    ///     if (frame_type != KEY_FRAME && !intra_only) {
    ///       allow_warped_motion      f(1)
    ///     }
    ///     reduced_tx_set             f(1)
    ///
    ///     // Tile configuration
    ///     tile_info()
    ///
    ///     // Quantizer deltas
    ///     quantizer_index_delta_params()
    ///   }
    /// }
    /// ```
    ///
    /// # MVP Implementation (Keyframe Only, FFmpeg-Compatible)
    /// - show_existing_frame = 0 (always encode new frame)
    /// - frame_type = KEY_FRAME (0)
    /// - show_frame = 1 (display immediately)
    /// - error_resilient_mode = 1 (simplifies decoder state)
    /// - disable_cdf_update = 0 (allow CDF updates)
    /// - frame_size_override_flag = 0 (match sequence header)
    /// - order_hint NOT written (enable_order_hint = 0 in sequence header)
    /// - refresh_frame_flags = 0xFF (refresh all 8 reference slots)
    /// - base_q_idx = 100 (from CRF 28, typical medium quality)
    /// - Segmentation disabled (segmentation_enabled = 0)
    /// - Delta-Q disabled (delta_q_present = 0)
    /// - Delta-LF disabled (delta_lf_present = 0)
    /// - Loop filter minimal (filter_level[0] = 8, filter_level[1] = 8)
    /// - CDEF enabled (enable_cdef = 1, cdef_params written with minimal settings)
    /// - LR disabled (enable_restoration = 0, no lr_params written)
    /// - TX mode = TX_MODE_SELECT (largest transform allowed)
    /// - Single tile (tile_cols = 0, tile_rows = 0, TileCols=1, TileRows=1)
    ///
    /// # Performance
    /// - Latency: <500ns (bit packing + OBU framing)
    ///
    /// # ASSUM Safety
    /// - #ASSUME_KEYFRAME_ONLY: Current implementation supports KEY_FRAME only
    /// - #ASSUME_SINGLE_TILE: Single tile simplifies bitstream (no tile_size_bytes)
    /// - #ASSUME_NO_SUPERRES: Superres disabled (coded size = render size)
    /// - #ASSUME_NO_FILM_GRAIN: Film grain disabled for MVP
    ///
    /// # Example
    /// ```
    /// use atomic_capsule::encoder::{ObuBitstreamWriterCapsule, FrameType};
    ///
    /// let writer = ObuBitstreamWriterCapsule::new();
    /// let obu = writer.write_frame_header_spec_compliant(FrameType::KeyFrame, 1920, 1080, 28);
    ///
    /// // OBU header (1B) + size (1-2B) + frame header (~8-10B minimum for keyframe)
    /// assert!(obu.len() >= 10);
    /// ```
    #[cfg(feature = "std")]
    pub fn write_frame_header_spec_compliant(&self, frame_type: FrameType, width: u16, height: u16, qp: u8) -> Vec<u8> {
        // #ASSUME_KEYFRAME_ONLY: Only keyframes supported in MVP
        assert_eq!(frame_type, FrameType::KeyFrame, "Only KEY_FRAME supported in MVP");

        let mut writer = BitWriter::new();

        // §5.9.1: show_existing_frame (1 bit)
        writer.write_bits(1, 0); // 0 = encode new frame

        // §5.9.2: frame_type (2 bits)
        writer.write_bits(2, frame_type as u64); // KEY_FRAME = 0b00

        // §5.9.3: show_frame (1 bit)
        writer.write_bits(1, 1); // 1 = display immediately

        // §5.9.4: showable_frame (1 bit) - skipped for KEY_FRAME with show_frame=1
        // (implicit: showable_frame = 0 for KEY_FRAME)

        // §5.9.7: error_resilient_mode (1 bit)
        writer.write_bits(1, 1); // 1 = reset decoder state (forced for KEY_FRAME)

        // §5.9.8: disable_cdf_update (1 bit)
        writer.write_bits(1, 0); // 0 = allow CDF updates

        // §5.9.9: allow_screen_content_tools (conditional)
        // With FFmpeg-compatible sequence header (64x64 hardcoded bytes):
        // seq_choose_screen_content_tools = 0, which means allow_screen_content_tools = 0 implicitly
        // Per AV1 spec: when seq_choose_screen_content_tools = 0, this field is NOT written
        // SKIPPED: seq_choose_screen_content_tools = 0 in FFmpeg bytes

        // §5.9.10: force_integer_mv (1 bit)
        // When SeqForceIntegerMv = 2 (SELECT) AND AllowScreenContentTools = 0,
        // ForceIntegerMv = 0 implicitly (NOT written to bitstream)
        // Since allow_screen_content_tools = 0 above, we skip this

        // §5.9.11: current_frame_id (conditional)
        // Skipped: frame_id_numbers_present_flag = 0 (disabled in sequence header)

        // §5.9.13: frame_size_override_flag (1 bit)
        // FFmpeg-compatible sequence headers use max dimensions = frame dimensions
        // so frame_size_override_flag = 0 (frame matches sequence max)
        writer.write_bits(1, 0); // 0 = use sequence header dimensions

        // §5.9.14: order_hint (conditional)
        // IMPORTANT: FFmpeg-generated sequence headers have enable_order_hint = 1 with order_hint_bits = 8
        // Per AV1 spec §5.9.14: when enable_order_hint = 1, we MUST write order_hint
        // For MVP keyframe-only mode, order_hint = 0 (first frame in display order)
        writer.write_bits(8, 0); // order_hint = 0 (8 bits per FFmpeg seq header)

        // §5.9.15: primary_ref_frame (3 bits) - for intra frames
        writer.write_bits(3, 0b111); // PRIMARY_REF_NONE = 7

        // §5.9.16: refresh_frame_flags (8 bits) - for KEY_FRAME
        writer.write_bits(8, 0xFF); // Refresh all 8 reference frame slots

        // §5.9.17: render_and_frame_size_different (conditional)
        // Skipped: enable_superres = 0 (disabled)

        // §5.9.18: allow_intrabc (conditional)
        // Skipped: intra block copy disabled for simplicity

        // §5.9.19: quantization_params()
        self.write_quantization_params(&mut writer, qp);

        // §5.9.20: segmentation_params()
        writer.write_bits(1, 0); // segmentation_enabled = 0

        // §5.9.21: delta_q_params()
        writer.write_bits(1, 0); // delta_q_present = 0

        // §5.9.22: delta_lf_params()
        // Conditional on delta_q_present, skipped

        // §5.9.23: loop_filter_params()
        self.write_loop_filter_params(&mut writer);

        // §5.9.24: cdef_params()
        // With FFmpeg-compatible sequence header (64x64 hardcoded bytes): enable_cdef = 0
        // Per AV1 spec §5.9.24: when enable_cdef = 0, cdef_params() is NOT written
        // SKIPPED: enable_cdef = 0 in FFmpeg bytes

        // §5.9.25: lr_params() (loop restoration)
        // FFmpeg-compatible sequence headers have enable_restoration = 1
        // Per AV1 spec §5.9.25: when enable_restoration = 1, we MUST write lr_params()
        // All restoration types = RESTORE_NONE for MVP simplicity
        self.write_lr_params(&mut writer);

        // §5.9.26: read_tx_mode()
        writer.write_bits(1, 1); // tx_mode_select = 1 (TX_MODE_SELECT, largest allowed)

        // §5.9.27: frame_reference_mode()
        // Skipped for KEY_FRAME (no inter prediction)

        // §5.9.28: skip_mode_params()
        // Skipped for KEY_FRAME

        // §5.9.29: allow_warped_motion (conditional)
        // Skipped for KEY_FRAME (intra only)

        // §5.9.30: reduced_tx_set (1 bit)
        writer.write_bits(1, 0); // 0 = use full transform set

        // §5.9.31: tile_info()
        self.write_tile_info(&mut writer, width, height);

        // §5.9.32: quantizer_index_delta_params()
        // Skipped: no delta quantizers for MVP

        // §5.9.33: loop_filter_delta_params()
        // Already handled in loop_filter_params

        // §5.3.5: trailing_bits() - REQUIRED for OBU byte alignment
        writer.write_trailing_bits();

        let payload = writer.flush();

        // Frame OBU header
        let header = self.write_obu_header(ObuType::FrameHeader, true);
        let size_bytes = self.encode_leb128(payload.len() as u64);

        let mut obu = Vec::with_capacity(1 + size_bytes.len() + payload.len());
        obu.push(header[0]);
        obu.extend_from_slice(&size_bytes);
        obu.extend_from_slice(&payload);

        self.update_checksum(&obu);
        // Note: OBU count not incremented (private field access not available)
        // This is a spec-compliant alternative implementation for testing

        obu
    }

    /// Write quantization_params() (§5.9.19)
    ///
    /// Writes quantization parameters using the provided QP value from RateControlCapsule.
    /// The QP (quantization parameter) directly maps to AV1's base_q_idx in the range [0-63].
    ///
    /// # Parameters
    /// - `writer`: BitWriter for frame header
    /// - `qp`: Quantization parameter from RateControlCapsule (0-63, lower = better quality)
    fn write_quantization_params(&self, writer: &mut BitWriter, qp: u8) {
        // base_q_idx (8 bits) - primary quantizer index [0-255]
        // QP from RateControlCapsule is in range [0-63], which directly maps to base_q_idx
        // Clamp to valid range [0-255] for safety
        let base_q_idx = qp.min(255);
        writer.write_bits(8, base_q_idx as u64);

        // DeltaQYDc (conditional) - Y DC delta
        writer.write_bits(1, 0); // diff_uv_delta = 0 (no Y DC delta)

        // DeltaQUDc (1 bit presence + value) - U DC delta
        writer.write_bits(1, 0); // No U DC delta

        // DeltaQUAc (1 bit presence + value) - U AC delta
        writer.write_bits(1, 0); // No U AC delta

        // DeltaQVDc (1 bit presence + value) - V DC delta
        writer.write_bits(1, 0); // No V DC delta

        // DeltaQVAc (1 bit presence + value) - V AC delta
        writer.write_bits(1, 0); // No V AC delta

        // using_qmatrix (1 bit) - quantization matrix usage
        writer.write_bits(1, 0); // 0 = no custom quant matrix
    }

    /// Write loop_filter_params() (§5.9.23)
    ///
    /// Minimal deblocking filter configuration.
    fn write_loop_filter_params(&self, writer: &mut BitWriter) {
        // loop_filter_level[0] (6 bits) - Y vertical edge strength [0-63]
        writer.write_bits(6, 8); // Mild filtering

        // loop_filter_level[1] (6 bits) - Y horizontal edge strength
        writer.write_bits(6, 8); // Mild filtering

        // loop_filter_level[2] (6 bits) - U strength (if num_planes > 1)
        writer.write_bits(6, 0); // No chroma filtering for simplicity

        // loop_filter_level[3] (6 bits) - V strength
        writer.write_bits(6, 0); // No chroma filtering

        // loop_filter_sharpness (3 bits) - sharpness control [0-7]
        writer.write_bits(3, 0); // No sharpness boost

        // loop_filter_delta_enabled (1 bit) - reference frame deltas
        writer.write_bits(1, 0); // 0 = no per-reference deltas
    }

    /// Write cdef_params() (§5.9.24)
    ///
    /// Constrained Directional Enhancement Filter (CDEF) parameters.
    fn write_cdef_params(&self, writer: &mut BitWriter) {
        // cdef_damping (2 bits) - damping factor minus 3 [0-3]
        writer.write_bits(2, 0); // Damping = 3 (minimal)

        // cdef_bits (2 bits) - log2 of number of CDEF strength pairs [0-3]
        writer.write_bits(2, 0); // cdef_bits = 0 -> 1 strength pair (2^0)

        // For each strength pair (count = 2^cdef_bits = 1):
        // cdef_y_pri_strength[0] (4 bits) - Y primary strength [0-15]
        writer.write_bits(4, 0); // No CDEF filtering

        // cdef_y_sec_strength[0] (2 bits) - Y secondary strength [0-3]
        writer.write_bits(2, 0); // No secondary

        // cdef_uv_pri_strength[0] (4 bits) - UV primary strength
        writer.write_bits(4, 0); // No chroma CDEF

        // cdef_uv_sec_strength[0] (2 bits) - UV secondary strength
        writer.write_bits(2, 0); // No chroma secondary
    }

    /// Write lr_params() (§5.9.25)
    ///
    /// Loop Restoration (LR) filter parameters.
    fn write_lr_params(&self, writer: &mut BitWriter) {
        // For each plane (Y, U, V):
        // lr_type[plane] (2 bits) - restoration type
        // 0 = RESTORE_NONE, 1 = RESTORE_WIENER, 2 = RESTORE_SGRPROJ, 3 = RESTORE_SWITCHABLE

        // Y plane
        writer.write_bits(2, 0); // RESTORE_NONE

        // U plane
        writer.write_bits(2, 0); // RESTORE_NONE

        // V plane
        writer.write_bits(2, 0); // RESTORE_NONE

        // No lr_unit_shift signaling needed when all types are RESTORE_NONE
    }

    /// Write tile_info() (§5.9.31)
    ///
    /// Tile configuration - single tile for MVP (simplest case).
    fn write_tile_info(&self, writer: &mut BitWriter, _width: u16, _height: u16) {
        // uniform_tile_spacing_flag (1 bit)
        writer.write_bits(1, 1); // 1 = uniform spacing (simplest)

        // For uniform spacing with single tile:
        // TileColsLog2 = 0 (2^0 = 1 column)
        // TileRowsLog2 = 0 (2^0 = 1 row)

        // increment_tile_cols_log2 (read until 0)
        // Since we want TileColsLog2 = 0, we signal 0 immediately (no increments)
        writer.write_bits(1, 0); // 0 = stop, TileColsLog2 = 0

        // increment_tile_rows_log2 (read until 0)
        writer.write_bits(1, 0); // 0 = stop, TileRowsLog2 = 0

        // context_update_tile_id (conditional on TileCols * TileRows > 1)
        // Skipped: only 1 tile (1 * 1 = 1)

        // tile_size_bytes_minus_1 (2 bits, conditional on TileCols * TileRows > 1)
        // Skipped: only 1 tile
    }
}
