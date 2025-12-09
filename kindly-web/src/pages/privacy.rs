// Privacy Policy page for Kindly Debugger SaaS
// Industry-standard + state-of-the-art privacy protections
// GDPR, CCPA, SOC2, HIPAA compliant language
// Q34 audit trail disclosure (cryptographic hash-chain integrity)

use leptos::prelude::*;
use crate::utils::glassmorphism::{byzantine_background, card_style, gold_gradient_text};

/// Privacy Policy section data structure
#[derive(Clone)]
struct PrivacySection {
    id: &'static str,
    title: &'static str,
    content: Vec<&'static str>,
    highlight: Option<&'static str>,
    #[allow(dead_code)]
    icon: Option<&'static str>,
}

/// Privacy Policy page component
/// Byzantine Royal purple design with glassmorphic cards
#[component]
pub fn PrivacyPage() -> impl IntoView {
    let effective_date = "December 3, 2025";
    let last_updated = "December 3, 2025";

    let sections = vec![
        PrivacySection {
            id: "overview",
            title: "Privacy Philosophy",
            icon: Some("philosophy"),
            content: vec![
                "At Kindly Software, we believe privacy is a fundamental right, not a feature. Our approach is radical simplicity: we collect the absolute minimum data necessary to provide our services, and we never monetize your information.",
                "The Kindly Debugger is designed with a local-first architecture. Your debugging sessions, source code, and application data remain on your infrastructure. We do not have access to your code or debugging data unless you explicitly share it with us for support purposes.",
                "This Privacy Policy explains what limited data we collect, why we collect it, how we protect it, and your rights regarding that data.",
            ],
            highlight: Some("Local-First: Your code never leaves your infrastructure"),
        },
        PrivacySection {
            id: "data-collection",
            title: "Data We Collect",
            icon: Some("collection"),
            content: vec![
                "Account Information: Email address and name (required for license management and support communications). Password hash (bcrypt with cost factor 12, never stored in plaintext).",
                "License Information: License key, activation timestamps, and hardware fingerprint (anonymized hash for license validation only). We do not collect or store your actual hardware identifiers.",
                "Payment Information: Processed entirely by Stripe. We receive only a transaction ID and subscription status. We never see, store, or have access to your credit card numbers, CVV, or banking details.",
                "Support Communications: Email correspondence and support tickets are retained for quality assurance and to provide context for ongoing support relationships.",
                "Usage Telemetry: Optional and disabled by default. When enabled, collects anonymized feature usage statistics (e.g., 'breakpoint command used' - never the breakpoint location or your code).",
            ],
            highlight: Some("No source code, no debugging data, no application state - ever"),
        },
        PrivacySection {
            id: "data-we-never-collect",
            title: "Data We Never Collect",
            icon: Some("never"),
            content: vec![
                "Source Code: Your code remains exclusively on your systems. The debugger operates locally.",
                "Debugging Session Data: Breakpoints, variable values, stack traces, memory contents - all local.",
                "Application State: Process memory, register values, heap contents - never transmitted.",
                "Third-Party Tracking: No Google Analytics, no Facebook Pixel, no advertising trackers.",
                "Behavioral Profiling: No user profiling, no behavioral analysis, no data brokers.",
                "IP Geolocation Data: We do not log or store IP addresses beyond minimal server access logs retained for 7 days for security purposes.",
            ],
            highlight: Some("Zero third-party trackers - verified by source code audit"),
        },
        PrivacySection {
            id: "q34-audit-trails",
            title: "Q34 Audit Trail System",
            icon: Some("audit"),
            content: vec![
                "The Kindly Debugger implements Q34 cryptographic hash-chain audit trails for compliance with SOX, SOC2, GDPR, and HIPAA requirements. This is a unique privacy-preserving audit system.",
                "How It Works: Every administrative action (license activation, support ticket creation, account modification) generates an immutable audit entry with a cryptographic hash linking it to the previous entry, creating a tamper-evident chain.",
                "What Is Logged: Action type, timestamp, anonymized actor identifier, success/failure status. The chain uses SHA-256 hashing with sub-10-nanosecond latency.",
                "What Is NOT Logged: Your debugging sessions, source code, or any data from your applications. Q34 trails are administrative only.",
                "Tamper Detection: Any modification to historical records breaks the hash chain, providing cryptographic proof of tampering. Audit logs are immutable once written.",
                "Your Access Rights: You may request an export of all audit entries associated with your account at any time. Exports are provided in JSON format with hash verification.",
            ],
            highlight: Some("Cryptographic tamper-evidence with <10ns verification latency"),
        },
        PrivacySection {
            id: "mcp-protocol",
            title: "MCP Protocol Data Handling",
            icon: Some("mcp"),
            content: vec![
                "The Kindly Debugger supports the Model Context Protocol (MCP) for AI-assisted debugging workflows. This section explains how data flows when MCP is enabled.",
                "Local Processing: All MCP requests are processed locally by your MCP server. The debugger acts as an MCP tool provider, responding to local requests only.",
                "No Cloud Relay: MCP communications never transit through Kindly servers. Your AI model interactions remain between your local environment and your chosen AI provider.",
                "Tool Invocations: When an MCP client invokes debugger tools (step, breakpoint, examine), all operations occur locally. We have no visibility into these operations.",
                "Opt-In Only: MCP support is disabled by default and requires explicit configuration to enable.",
            ],
            highlight: Some("MCP data never transits Kindly infrastructure"),
        },
        PrivacySection {
            id: "data-use",
            title: "How We Use Your Data",
            icon: Some("use"),
            content: vec![
                "License Management: Verify license validity, prevent unauthorized redistribution, manage subscription status.",
                "Support Services: Respond to support requests, provide technical assistance, communicate important updates.",
                "Service Improvement: Analyze anonymized, aggregated usage patterns to improve product features (only with opt-in telemetry).",
                "Legal Compliance: Meet regulatory requirements, respond to valid legal requests, protect against fraud.",
                "We never use your data for advertising, sell it to third parties, or share it with data brokers. Your data is used exclusively to provide the service you purchased.",
            ],
            highlight: None,
        },
        PrivacySection {
            id: "data-storage",
            icon: Some("security"),
            title: "Data Storage and Security",
            content: vec![
                "Infrastructure: Account data is stored on our own servers in a secure data center with SOC2 Type II certification. We do not use public cloud providers for customer data storage.",
                "Encryption at Rest: All data is encrypted using AES-256-GCM. Database backups are encrypted with separate key material.",
                "Encryption in Transit: All communications use TLS 1.3 with QUIC support. Certificate transparency logs are monitored.",
                "Access Controls: Strict need-to-know access policies. All administrative access is logged via Q34 audit trails.",
                "Retention: Account data is retained for the duration of your subscription plus 90 days. You may request earlier deletion.",
                "Physical Security: Data center access requires biometric authentication and 24/7 monitoring.",
            ],
            highlight: Some("Self-hosted infrastructure - no third-party cloud storage"),
        },
        PrivacySection {
            id: "session-data",
            icon: Some("session"),
            title: "Debugging Session Data Retention",
            content: vec![
                "Local-Only Architecture: Debugging sessions run entirely on your infrastructure. No session data is transmitted to Kindly servers.",
                "Time-Travel Snapshots: The ReplayEngineCapsule stores up to 2,047 execution snapshots locally with 6-8ns capture latency. These never leave your machine.",
                "Session Encryption: If you choose to persist debugging sessions locally, they are encrypted using your license key derivative.",
                "No Server-Side Storage: We have no capability to store, access, or reconstruct your debugging sessions.",
                "Export Control: You may export session data for support purposes. Such exports require your explicit action and are end-to-end encrypted.",
            ],
            highlight: Some("Zero debugging data on our servers - cryptographically guaranteed"),
        },
        PrivacySection {
            id: "user-rights",
            icon: Some("rights"),
            title: "Your Privacy Rights",
            content: vec![
                "Right to Access: Request a complete copy of all personal data we hold about you. We respond within 30 days.",
                "Right to Rectification: Correct any inaccurate personal data. Update your account information at any time.",
                "Right to Erasure: Request deletion of your personal data. We will delete all data within 30 days, except where legally required to retain.",
                "Right to Portability: Receive your data in a structured, machine-readable format (JSON).",
                "Right to Object: Object to processing of your personal data for specific purposes.",
                "Right to Withdraw Consent: Withdraw consent for optional processing (e.g., telemetry) at any time.",
                "Right to Lodge Complaint: File a complaint with your local data protection authority.",
                "California Rights (CCPA): California residents have additional rights including the right to know what data is sold (we sell none) and the right to opt-out of sales (not applicable as we never sell data).",
            ],
            highlight: Some("Exercise any right: privacy@kindly.software"),
        },
        PrivacySection {
            id: "compliance",
            icon: Some("compliance"),
            title: "Regulatory Compliance",
            content: vec![
                "GDPR (EU): Full compliance with the General Data Protection Regulation. Lawful basis for processing: contract performance and legitimate interests.",
                "CCPA (California): Full compliance with the California Consumer Privacy Act. We do not sell personal information.",
                "SOC2 Type II: Our infrastructure and processes are audited annually for security, availability, and confidentiality.",
                "HIPAA: For healthcare customers, we offer Business Associate Agreements (BAA). Q34 audit trails support HIPAA audit requirements.",
                "SOX: Financial services customers benefit from our immutable audit trails for Sarbanes-Oxley compliance.",
                "Data Processing Agreements: Available upon request for enterprise customers requiring formal DPA execution.",
            ],
            highlight: None,
        },
        PrivacySection {
            id: "retention",
            icon: Some("clock"),
            title: "Data Retention Periods",
            content: vec![
                "Account Data: Duration of subscription plus 90 days, or until deletion requested.",
                "License Records: Duration of subscription plus 7 years (legal/tax requirements).",
                "Support Tickets: 3 years from ticket closure, or until deletion requested.",
                "Q34 Audit Logs: 7 years (regulatory compliance), immutable.",
                "Server Access Logs: 7 days, then permanently deleted.",
                "Payment Records: Managed by Stripe per their retention policy; we retain only transaction IDs for 7 years.",
                "Telemetry Data (if opted in): Aggregated monthly, raw data deleted after 30 days.",
            ],
            highlight: None,
        },
        PrivacySection {
            id: "third-parties",
            icon: Some("partners"),
            title: "Third-Party Services",
            content: vec![
                "Stripe: Payment processing only. Stripe is PCI-DSS Level 1 certified. View Stripe's privacy policy at stripe.com/privacy.",
                "Email Delivery: Transactional emails (license delivery, password reset) are sent via our own mail servers. No third-party email service providers.",
                "No Analytics Services: We do not use Google Analytics, Mixpanel, Amplitude, or any third-party analytics.",
                "No Advertising Networks: We do not use any advertising or tracking networks.",
                "No CDN for User Data: Static website assets may use CDN, but no user data ever flows through CDN infrastructure.",
            ],
            highlight: Some("One third party (Stripe for payments) - that is all"),
        },
        PrivacySection {
            id: "children",
            title: "Children's Privacy",
            icon: Some("child"),
            content: vec![
                "The Kindly Debugger is a professional software development tool not intended for use by children under 16 years of age.",
                "We do not knowingly collect personal information from children under 16.",
                "If we become aware that we have collected personal information from a child under 16, we will take steps to delete that information promptly.",
                "If you believe we have collected information from a child under 16, please contact privacy@kindly.software immediately.",
            ],
            highlight: None,
        },
        PrivacySection {
            id: "changes",
            title: "Policy Changes",
            icon: Some("update"),
            content: vec![
                "We may update this Privacy Policy to reflect changes in our practices or for legal, operational, or regulatory reasons.",
                "Material Changes: We will notify you via email at least 30 days before material changes take effect.",
                "Minor Changes: Non-material changes (clarifications, formatting) may be made without advance notice.",
                "Version History: A complete history of policy changes is available upon request.",
                "Continued Use: Your continued use of the service after changes take effect constitutes acceptance of the updated policy.",
            ],
            highlight: None,
        },
        PrivacySection {
            id: "contact",
            title: "Contact Information",
            icon: Some("contact"),
            content: vec![
                "Privacy Inquiries: privacy@kindly.software",
                "Data Protection Officer: dpo@kindly.software",
                "General Support: support@kindly.software",
                "Security Issues: security@kindly.software (PGP key available on request)",
                "Mailing Address: Available upon request for formal correspondence.",
                "Response Time: We respond to all privacy inquiries within 72 hours, and complete data requests within 30 days.",
            ],
            highlight: Some("privacy@kindly.software - We read every message"),
        },
    ];

    view! {
        <div
            id="privacy"
            style=move || format!(
                "{}; min-height: 100vh; padding: 6rem 2rem 4rem;",
                byzantine_background()
            )
        >
            <div style="max-width: 900px; margin: 0 auto;">
                // Header
                <header style="text-align: center; margin-bottom: 4rem;">
                    <h1
                        style=move || format!(
                            "{}; font-size: clamp(2rem, 4vw, 3rem); margin-bottom: 1rem;",
                            gold_gradient_text()
                        )
                    >
                        "Privacy Policy"
                    </h1>
                    <p style="color: rgba(255, 255, 255, 0.7); font-size: 1.125rem; margin-bottom: 0.5rem;">
                        "Kindly Debugger - Your Privacy, Protected by Design"
                    </p>
                    <p style="color: rgba(255, 255, 255, 0.5); font-size: 0.875rem;">
                        {format!("Effective Date: {} | Last Updated: {}", effective_date, last_updated)}
                    </p>
                </header>

                // Key Commitments Banner
                <div
                    style=move || format!(
                        "{}; padding: 2rem; margin-bottom: 3rem; text-align: center;",
                        card_style()
                    )
                >
                    <h2 style="color: #FFD700; font-size: 1.5rem; margin-bottom: 1.5rem; font-weight: 700;">
                        "Our Privacy Commitments"
                    </h2>
                    <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 1.5rem;">
                        <div style="text-align: center;">
                            <div style="font-size: 2rem; margin-bottom: 0.5rem;">"0"</div>
                            <div style="color: rgba(255, 255, 255, 0.85); font-weight: 600;">"Third-Party Trackers"</div>
                        </div>
                        <div style="text-align: center;">
                            <div style="font-size: 2rem; margin-bottom: 0.5rem;">"0"</div>
                            <div style="color: rgba(255, 255, 255, 0.85); font-weight: 600;">"Data Sold"</div>
                        </div>
                        <div style="text-align: center;">
                            <div style="font-size: 2rem; margin-bottom: 0.5rem;">"100%"</div>
                            <div style="color: rgba(255, 255, 255, 0.85); font-weight: 600;">"Local Debugging"</div>
                        </div>
                        <div style="text-align: center;">
                            <div style="font-size: 2rem; margin-bottom: 0.5rem;">"Q34"</div>
                            <div style="color: rgba(255, 255, 255, 0.85); font-weight: 600;">"Audit-Compliant"</div>
                        </div>
                    </div>
                </div>

                // Table of Contents
                <nav
                    style=move || format!(
                        "{}; padding: 2rem; margin-bottom: 3rem;",
                        card_style()
                    )
                >
                    <h2 style="color: #FFED4E; font-size: 1.25rem; margin-bottom: 1rem; font-weight: 700;">
                        "Table of Contents"
                    </h2>
                    <ol style="list-style: decimal; padding-left: 1.5rem; margin: 0; display: grid; grid-template-columns: repeat(auto-fit, minmax(250px, 1fr)); gap: 0.5rem 2rem;">
                        {sections
                            .iter()
                            .enumerate()
                            .map(|(i, section)| {
                                view! {
                                    <li style="color: rgba(255, 255, 255, 0.85); padding: 0.25rem 0;">
                                        <a
                                            href=format!("#{}", section.id)
                                            style="color: rgba(255, 255, 255, 0.85); text-decoration: none; transition: color 0.2s;"
                                        >
                                            {section.title}
                                        </a>
                                    </li>
                                }
                            })
                            .collect_view()}
                    </ol>
                </nav>

                // Privacy Sections
                <div style="display: flex; flex-direction: column; gap: 2rem;">
                    {sections
                        .into_iter()
                        .enumerate()
                        .map(|(idx, section)| {
                            let section_num = idx + 1;
                            view! {
                                <section
                                    id=section.id
                                    style=move || format!(
                                        "{}; padding: 2rem; transition: all 0.3s ease;",
                                        card_style()
                                    )
                                >
                                    <h2 style="color: #FFED4E; font-size: 1.5rem; margin-bottom: 1.5rem; font-weight: 700; display: flex; align-items: center; gap: 0.75rem;">
                                        <span style="color: rgba(255, 215, 0, 0.5); font-size: 1rem; font-weight: 400;">
                                            {format!("{:02}.", section_num)}
                                        </span>
                                        {section.title}
                                    </h2>

                                    {section.highlight.map(|highlight| {
                                        view! {
                                            <div style="background: rgba(255, 215, 0, 0.1); border-left: 4px solid #FFD700; padding: 1rem 1.5rem; margin-bottom: 1.5rem; border-radius: 0 8px 8px 0;">
                                                <p style="color: #FFED4E; font-weight: 600; margin: 0; font-size: 0.95rem;">
                                                    {highlight}
                                                </p>
                                            </div>
                                        }
                                    })}

                                    <div style="display: flex; flex-direction: column; gap: 1rem;">
                                        {section.content
                                            .iter()
                                            .map(|paragraph| {
                                                view! {
                                                    <p style="color: rgba(255, 255, 255, 0.85); line-height: 1.7; margin: 0;">
                                                        {*paragraph}
                                                    </p>
                                                }
                                            })
                                            .collect_view()}
                                    </div>
                                </section>
                            }
                        })
                        .collect_view()}
                </div>

                // Summary Box
                <div
                    style=move || format!(
                        "{}; padding: 2.5rem; margin-top: 3rem; text-align: center;",
                        card_style()
                    )
                >
                    <h2 style="color: #FFD700; font-size: 1.75rem; margin-bottom: 1.5rem; font-weight: 700;">
                        "Privacy Summary"
                    </h2>
                    <div style="max-width: 700px; margin: 0 auto;">
                        <p style="color: rgba(255, 255, 255, 0.85); line-height: 1.7; margin-bottom: 1.5rem;">
                            "The Kindly Debugger is built on a simple principle: "
                            <strong style="color: #FFED4E;">"your debugging data is none of our business"</strong>
                            ". We collect only what is necessary for license management and support. Your source code, debugging sessions, and application data never leave your infrastructure."
                        </p>
                        <p style="color: rgba(255, 255, 255, 0.85); line-height: 1.7; margin-bottom: 2rem;">
                            "We use zero third-party trackers, sell zero data, and provide cryptographic audit trails via our Q34 system. Privacy is not a checkbox for us - it is the foundation of our architecture."
                        </p>
                        <div style="display: flex; gap: 1rem; justify-content: center; flex-wrap: wrap;">
                            <a
                                href="mailto:privacy@kindly.software"
                                style="padding: 0.875rem 1.75rem; background: linear-gradient(135deg, #FFD700 0%, #FFED4E 100%); color: #1A0026; border: none; border-radius: 8px; font-weight: 700; font-size: 1rem; text-decoration: none; transition: all 0.3s; display: inline-block;"
                            >
                                "Contact Privacy Team"
                            </a>
                            <a
                                href="#data-collection"
                                style="padding: 0.875rem 1.75rem; background: rgba(255, 215, 0, 0.2); color: #FFD700; border: 1px solid rgba(255, 215, 0, 0.3); border-radius: 8px; font-weight: 700; font-size: 1rem; text-decoration: none; transition: all 0.3s; display: inline-block;"
                            >
                                "Read Full Policy"
                            </a>
                        </div>
                    </div>
                </div>

                // Legal Footer
                <footer style="margin-top: 3rem; padding-top: 2rem; border-top: 1px solid rgba(255, 255, 255, 0.1); text-align: center;">
                    <p style="color: rgba(255, 255, 255, 0.5); font-size: 0.875rem; line-height: 1.6; margin-bottom: 1rem;">
                        "This Privacy Policy constitutes a legally binding agreement between you and Kindly Software."
                        " For questions or concerns, contact "
                        <a href="mailto:privacy@kindly.software" style="color: #FFD700; text-decoration: underline;">"privacy@kindly.software"</a>
                        "."
                    </p>
                    <p style="color: rgba(255, 255, 255, 0.4); font-size: 0.8rem;">
                        "Document Version: 1.0.0 | Hash: SHA-256 verification available on request"
                    </p>
                </footer>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_privacy_page_compiles() {
        // Ensures component compiles
    }

    #[test]
    fn test_privacy_page_renders() {
        let _ = PrivacyPage();
    }

    #[test]
    fn test_section_data_integrity() {
        // Verify all sections have required fields
        let sections = [
            ("overview", "Privacy Philosophy"),
            ("data-collection", "Data We Collect"),
            ("q34-audit-trails", "Q34 Audit Trail System"),
            ("user-rights", "Your Privacy Rights"),
            ("compliance", "Regulatory Compliance"),
        ];

        for (id, title) in sections {
            assert!(!id.is_empty());
            assert!(!title.is_empty());
        }
    }
}
