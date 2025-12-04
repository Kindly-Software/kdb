#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct RiskComponents {
    pub intrusion_risk: u16,
    pub license_risk: u16,
    pub session_risk: u16,
    pub rate_limit_risk: u16,
    pub anomaly_risk: u16,
    pub totp_risk: u16,
    pub pid_access_risk: u16,
    pub _reserved: u16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RiskScore {
    pub total_risk: u16,
    pub component_risks: RiskComponents,
    pub _reserved: [u16; 7],
}

fn main() {
    use std::mem::{size_of, align_of};
    println!("RiskComponents: {} bytes, align {}", size_of::<RiskComponents>(), align_of::<RiskComponents>());
    println!("RiskScore: {} bytes, align {}", size_of::<RiskScore>(), align_of::<RiskScore>());
    
    // Test that structs can be instantiated
    let components = RiskComponents::default();
    let score = RiskScore::from_components(components);
}

impl RiskComponents {
    fn aggregate_risk(&self) -> u16 {
        0
    }
}

impl RiskScore {
    fn from_components(components: RiskComponents) -> Self {
        let total_risk = components.aggregate_risk();
        Self {
            total_risk,
            component_risks: components,
            _reserved: [0; 7],
        }
    }
}
