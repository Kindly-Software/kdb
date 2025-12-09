use leptos::prelude::*;
use crate::components::molecular::{SectionContainer, FeatureCard};

#[component]
pub fn Security() -> impl IntoView {
    let certifications = vec![
        ("SOX", "Sarbanes-Oxley", "Hash chain audit trails for financial compliance"),
        ("SOC2 Type II", "Service Organization Control", "Tamper-proof logging for observation period"),
        ("GDPR", "General Data Protection", "Data access logging and right to be forgotten tracking"),
        ("HIPAA", "Health Insurance Portability", "PHI access logging and breach detection"),
    ];

    view! {
        <SectionContainer id="security" class="security-section">
            <h2 class="section-title">"Enterprise Security & Compliance"</h2>
            <p class="section-subtitle">
                "Built-in compliance audit trails with hash chain integrity verification"
            </p>
            <div class="security-grid">
                {certifications.into_iter().map(|(name, full_name, description)| {
                    view! {
                        <div class="certification-card">
                            <div class="certification-badge">
                                <i class="icon icon-shield-check"></i>
                                <span class="badge-text">{name}</span>
                            </div>
                            <h4 class="certification-title">{full_name}</h4>
                            <p class="certification-description">{description}</p>
                        </div>
                    }
                }).collect::<Vec<_>>()}
            </div>
            <div class="security-features">
                <FeatureCard
                    icon="icon-lock"
                    title="Hash Chain Integrity"
                    description="<2ns tamper detection with 64-bit collision resistance"
                />
                <FeatureCard
                    icon="icon-timeline"
                    title="Forensic Analysis"
                    description="Timeline reconstruction and anomaly detection"
                />
                <FeatureCard
                    icon="icon-export"
                    title="Audit Export"
                    description="JSON, CSV, and binary export formats"
                />
            </div>
        </SectionContainer>
    }
}
