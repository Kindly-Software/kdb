//! T3 Fixed-Point baseline: Replace fixed-point with f64
//!
//! Fair baseline strategy:
//! - Use f64 (hardware-accelerated floating-point)
//! - Same algorithm, different arithmetic type
//! - Measure determinism + performance tradeoff

/// Example: P&L calculation (Fixed-Point vs F64)
///
/// # Fixed-Point version (T3)
/// ```rust,ignore
/// use fixed::types::I32F32; // Q16.16 fixed-point
///
/// fn calculate_pnl_fixed(price: I32F32, quantity: i32) -> I32F32 {
///     price * I32F32::from_num(quantity)
/// }
/// ```
///
/// # F64 baseline
/// ```rust,ignore
/// fn calculate_pnl_f64(price: f64, quantity: i32) -> f64 {
///     price * (quantity as f64)
/// }
/// ```

/// P&L calculation using f64 (baseline)
///
/// # Arguments
/// * `price` - Price per unit
/// * `quantity` - Quantity (positive for long, negative for short)
///
/// # Returns
/// Profit/Loss in currency units
pub fn calculate_pnl_f64(price: f64, quantity: i32) -> f64 {
    price * (quantity as f64)
}

/// Fixed-point simulation using integer arithmetic
///
/// For Phase 1, we simulate fixed-point behavior using scaled integers:
/// Q16.16 format: 16 bits integer, 16 bits fraction
/// Value = integer_value / 2^16
pub fn calculate_pnl_fixed_simulated(price_scaled: i64, quantity: i32) -> i64 {
    // price_scaled is already in Q16.16 format (multiplied by 2^16)
    // Result in Q16.16 format
    price_scaled * (quantity as i64)
}

/// Convert f64 to Q16.16 fixed-point (scaled integer)
pub fn f64_to_q16_16(value: f64) -> i64 {
    (value * 65536.0) as i64
}

/// Convert Q16.16 fixed-point to f64
pub fn q16_16_to_f64(scaled: i64) -> f64 {
    (scaled as f64) / 65536.0
}

/// Kelly criterion calculation (f64 baseline)
///
/// # Formula
/// f* = (bp - q) / b
///
/// where:
/// - b = odds received on the wager (net odds)
/// - p = probability of winning
/// - q = probability of losing (1 - p)
/// - f* = fraction of bankroll to wager
pub fn kelly_criterion_f64(win_prob: f64, odds: f64) -> f64 {
    let lose_prob = 1.0 - win_prob;
    (odds * win_prob - lose_prob) / odds
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pnl_f64() {
        let price = 100.0;
        let quantity = 1000;
        let pnl = calculate_pnl_f64(price, quantity);

        assert_eq!(pnl, 100_000.0);
    }

    #[test]
    fn test_pnl_fixed_simulated() {
        // Price = 100.0 in Q16.16 format
        let price_scaled = f64_to_q16_16(100.0);
        let quantity = 1000;

        let pnl_scaled = calculate_pnl_fixed_simulated(price_scaled, quantity);
        let pnl_f64 = q16_16_to_f64(pnl_scaled);

        assert!((pnl_f64 - 100_000.0).abs() < 0.01);
    }

    #[test]
    fn test_fixed_point_conversion() {
        let value = 123.456;
        let scaled = f64_to_q16_16(value);
        let recovered = q16_16_to_f64(scaled);

        assert!((recovered - value).abs() < 0.001);
    }

    #[test]
    fn test_kelly_criterion() {
        // 60% win probability, 2:1 odds
        let fraction = kelly_criterion_f64(0.6, 2.0);

        // f* = (2 * 0.6 - 0.4) / 2 = 0.4 (bet 40% of bankroll)
        assert!((fraction - 0.4).abs() < 0.001);
    }
}
