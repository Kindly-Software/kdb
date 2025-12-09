use aid_96::{class, Aid96};
use serde::{Deserialize, Serialize};

/// Simplified tunneling scanner that just records basic barriers per symbol.
#[derive(Default)]
pub struct TunnelingScanner {
    node_hint: u16,
}

impl TunnelingScanner {
    pub fn new(node_hint: u16) -> Self {
        Self { node_hint }
    }

    pub fn derive_opportunity(
        &self,
        symbol: &str,
        current_price: f64,
        barrier_price: f64,
    ) -> TunnelingOpportunity {
        let direction = if barrier_price >= current_price {
            BarrierType::Resistance
        } else {
            BarrierType::Support
        };

        TunnelingOpportunity {
            id: Aid96::new(class::DOS),
            node_hint: self.node_hint,
            symbol: symbol.to_string(),
            current_price,
            barrier_price,
            barrier_type: direction,
            transmission_probability: 0.5,
            expected_profit_bp: (((barrier_price - current_price) / current_price) * 10_000.0).abs()
                as u32,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BarrierType {
    Resistance,
    Support,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TunnelingOpportunity {
    pub id: Aid96,
    pub node_hint: u16,
    pub symbol: String,
    pub current_price: f64,
    pub barrier_price: f64,
    pub barrier_type: BarrierType,
    pub transmission_probability: f64,
    pub expected_profit_bp: u32,
}
