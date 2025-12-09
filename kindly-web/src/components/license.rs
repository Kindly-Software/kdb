//! [TRADE SECRET] Kindly Debugger - Proprietary Software License Page
//!
//! Byzantine Royal purple design with comprehensive license terms for kdb debugger.
//! Ed25519 signature verification, tiered licensing, seat-based model.

use leptos::prelude::*;
use crate::utils::glassmorphism::{byzantine_background, gold_gradient_text, dark_card_style};

// ============================================================================
// LICENSE TIER DATA STRUCTURES
// ============================================================================

/// License tier configuration
struct LicenseTier {
    name: &'static str,
    price: &'static str,
    price_period: &'static str,
    description: &'static str,
    requests_per_month: &'static str,
    seats: &'static str,
    snapshot_retention: &'static str,
    features: &'static [&'static str],
    support: &'static str,
    is_featured: bool,
    badge: Option<&'static str>,
}

/// All license tiers for Kindly Debugger
const LICENSE_TIERS: &[LicenseTier] = &[
    LicenseTier {
        name: "Hobby",
        price: "Free",
        price_period: "forever",
        description: "For personal projects and learning",
        requests_per_month: "25",
        seats: "1",
        snapshot_retention: "24 hours",
        features: &[
            "Basic time-travel debugging",
            "Single process attachment",
            "Console interface (CLI)",
            "Community documentation",
        ],
        support: "GitHub Issues",
        is_featured: false,
        badge: None,
    },
    LicenseTier {
        name: "Starter",
        price: "$15",
        price_period: "/month",
        description: "For individual developers",
        requests_per_month: "100",
        seats: "1",
        snapshot_retention: "7 days",
        features: &[
            "Enhanced audit logging",
            "Multi-process attachment",
            "Breakpoint persistence",
            "Stack trace export",
            "Q34 hash-chain integrity",
        ],
        support: "Email (48h SLA)",
        is_featured: false,
        badge: None,
    },
    LicenseTier {
        name: "Developer",
        price: "$39",
        price_period: "/month",
        description: "For professional developers",
        requests_per_month: "500",
        seats: "1",
        snapshot_retention: "30 days",
        features: &[
            "Remote debugging API",
            "MCP server integration",
            "Bi-directional replay",
            "Memory region inspection",
            "Register state capture",
            "Symbol table integration",
        ],
        support: "Priority Email (24h SLA)",
        is_featured: true,
        badge: Some("MOST POPULAR"),
    },
    LicenseTier {
        name: "Professional",
        price: "$199",
        price_period: "/month",
        description: "For teams and organizations",
        requests_per_month: "Unlimited",
        seats: "5",
        snapshot_retention: "90 days",
        features: &[
            "Team collaboration features",
            "Centralized audit dashboard",
            "SOC2/GDPR compliance reports",
            "Custom snapshot policies",
            "API rate limit controls",
            "Dedicated support channel",
        ],
        support: "Slack/Discord (4h SLA)",
        is_featured: false,
        badge: None,
    },
    LicenseTier {
        name: "Enterprise",
        price: "Custom",
        price_period: "",
        description: "For regulated industries",
        requests_per_month: "Unlimited",
        seats: "Unlimited",
        snapshot_retention: "Custom",
        features: &[
            "HIPAA/SOX/FINRA compliance",
            "On-premise deployment",
            "Dedicated infrastructure",
            "Custom integrations",
            "Hardware security modules",
            "24/7 incident response",
            "Quarterly security audits",
        ],
        support: "Dedicated Account Team",
        is_featured: false,
        badge: Some("CONTACT SALES"),
    },
];

// ============================================================================
// LICENSE PAGE COMPONENT
// ============================================================================

/// Proprietary Software License Page for Kindly Debugger
#[component]
pub fn LicensePage() -> impl IntoView {
    view! {
        <div
            id="license"
            style=move || format!(
                "{}; \
                 min-height: 100vh; \
                 padding: 6rem 2rem 4rem;",
                byzantine_background()
            )
        >
            <div style="max-width: 1400px; margin: 0 auto;">
                // ================================================================
                // HEADER
                // ================================================================
                <div style="text-align: center; margin-bottom: 4rem;">
                    <h1
                        style=move || format!(
                            "{}; \
                             font-size: clamp(2rem, 4vw, 3.5rem); \
                             margin-bottom: 1rem;",
                            gold_gradient_text()
                        )
                    >
                        "Kindly Debugger License Agreement"
                    </h1>
                    <p style="color: rgba(255, 255, 255, 0.8); font-size: 1.25rem; max-width: 800px; margin: 0 auto 1rem; line-height: 1.6;">
                        "Proprietary Commercial Software License"
                    </p>
                    <p style="color: rgba(255, 255, 255, 0.6); font-size: 0.95rem; max-width: 700px; margin: 0 auto;">
                        "Effective Date: December 2025 | Version 1.0 | Last Updated: December 3, 2025"
                    </p>
                </div>

                // ================================================================
                // LICENSE GRANT SECTION
                // ================================================================
                <LicenseSection
                    title="1. License Grant"
                    id="grant"
                >
                    <div style="display: grid; gap: 1.5rem;">
                        <LicenseClause number="1.1" title="Grant of Rights">
                            "Subject to the terms and conditions of this Agreement and payment of applicable fees, \
                             Kindly Software Ltd. (\"Licensor\") grants you (\"Licensee\") a limited, non-exclusive, \
                             non-transferable, revocable license to use the Kindly Debugger software (\"Software\") \
                             solely for your internal business purposes in accordance with your selected license tier."
                        </LicenseClause>
                        <LicenseClause number="1.2" title="Seat-Based Licensing">
                            "Each license seat permits one (1) individual developer to use the Software. \
                             Seat assignments may be transferred between developers within your organization \
                             no more than once per calendar month. Concurrent usage by multiple individuals \
                             under a single seat is expressly prohibited."
                        </LicenseClause>
                        <LicenseClause number="1.3" title="License Verification">
                            "The Software requires periodic license verification via Ed25519 digital signatures. \
                             License keys are cryptographically bound to your organization and must be kept confidential. \
                             Tampering with license verification mechanisms constitutes a material breach of this Agreement."
                        </LicenseClause>
                    </div>
                </LicenseSection>

                // ================================================================
                // RESTRICTIONS SECTION
                // ================================================================
                <LicenseSection
                    title="2. Restrictions"
                    id="restrictions"
                >
                    <div style="display: grid; gap: 1.5rem;">
                        <LicenseClause number="2.1" title="Prohibited Activities">
                            "You shall NOT: (a) copy, modify, adapt, translate, or create derivative works of the Software; \
                             (b) reverse engineer, disassemble, decompile, or attempt to derive source code from the Software; \
                             (c) remove, alter, or obscure any proprietary notices, labels, or marks; \
                             (d) sublicense, rent, lease, loan, sell, or distribute the Software to any third party; \
                             (e) use the Software for competitive analysis, benchmarking, or product development \
                             that competes with the Software."
                        </LicenseClause>
                        <LicenseClause number="2.2" title="Trade Secret Protection">
                            "The Software contains valuable trade secrets and confidential information of Licensor. \
                             You acknowledge that unauthorized disclosure or use of such information would cause \
                             irreparable harm. You agree to maintain the confidentiality of the Software and \
                             implement reasonable security measures to prevent unauthorized access."
                        </LicenseClause>
                        <LicenseClause number="2.3" title="Export Compliance">
                            "You shall comply with all applicable export control laws and regulations. \
                             The Software may not be exported or re-exported to sanctioned countries or \
                             individuals prohibited under applicable export control laws."
                        </LicenseClause>
                    </div>
                </LicenseSection>

                // ================================================================
                // PRICING TIERS SECTION
                // ================================================================
                <LicenseSection
                    title="3. License Tiers and Pricing"
                    id="tiers"
                >
                    <div style="margin-bottom: 2rem;">
                        <p style="color: rgba(255, 255, 255, 0.8); line-height: 1.7; margin-bottom: 1.5rem;">
                            "Select the license tier that best matches your usage requirements. \
                             All paid tiers include automatic updates, security patches, and tier-specific support."
                        </p>
                    </div>
                    <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(280px, 1fr)); gap: 1.5rem;">
                        {LICENSE_TIERS.iter().map(|tier| view! {
                            <LicenseTierCard tier=tier />
                        }).collect::<Vec<_>>()}
                    </div>
                </LicenseSection>

                // ================================================================
                // LICENSE KEY FORMAT SECTION
                // ================================================================
                <LicenseSection
                    title="4. License Verification"
                    id="verification"
                >
                    <div style="display: grid; gap: 1.5rem;">
                        <LicenseClause number="4.1" title="License Key Format">
                            "License keys are issued in the following format and must be stored securely:"
                        </LicenseClause>
                        <div style=move || format!(
                            "{}; \
                             padding: 1.5rem; \
                             font-family: 'Courier New', monospace; \
                             font-size: 0.85rem; \
                             overflow-x: auto;",
                            dark_card_style()
                        )>
                            <pre style="color: #FFED4E; margin: 0; white-space: pre-wrap; word-break: break-all;">
{r#"KDB-[TIER]-[TIMESTAMP]-[ORG_HASH]-[SIGNATURE]

Example:
KDB-DEV-20251203-a8f2c91e-Ed25519:base64_signature

Components:
  KDB       - Product identifier (Kindly Debugger)
  TIER      - License tier (HOB|STR|DEV|PRO|ENT)
  TIMESTAMP - Issue date (YYYYMMDD)
  ORG_HASH  - SHA256(organization_id)[0:8]
  SIGNATURE - Ed25519 signature over payload"#}
                            </pre>
                        </div>
                        <LicenseClause number="4.2" title="Verification Protocol">
                            "The Software performs license verification at startup and periodically during operation. \
                             Verification uses Ed25519 public key cryptography to validate license authenticity \
                             without transmitting sensitive data. Offline verification is supported for air-gapped environments."
                        </LicenseClause>
                        <LicenseClause number="4.3" title="License Expiration">
                            "Subscription licenses expire at the end of the billing period if payment is not received. \
                             Upon expiration, the Software will enter read-only mode, preserving existing audit logs \
                             but preventing new debugging sessions. License renewal restores full functionality."
                        </LicenseClause>
                    </div>
                </LicenseSection>

                // ================================================================
                // Q34 AUDIT COMPLIANCE SECTION
                // ================================================================
                <LicenseSection
                    title="5. Audit Compliance (Q34)"
                    id="audit"
                >
                    <div style="display: grid; gap: 1.5rem;">
                        <LicenseClause number="5.1" title="Hash-Chain Audit Trail">
                            "The Software implements Q34-compliant cryptographic hash-chain audit trails. \
                             All debugging operations are logged with tamper-evident SHA-256 checksums, \
                             enabling forensic analysis and regulatory compliance verification."
                        </LicenseClause>
                        <LicenseClause number="5.2" title="Compliance Frameworks">
                            "Enterprise tier includes pre-configured compliance templates for:"
                        </LicenseClause>
                        <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 1rem; padding-left: 1.5rem;">
                            {["SOC 2 Type II", "GDPR Article 30", "HIPAA Security Rule", "SOX Section 404", "FINRA Rule 4511", "PCI DSS 3.2.1"]
                                .iter()
                                .map(|framework| view! {
                                    <div style="display: flex; align-items: center; gap: 0.5rem;">
                                        <span style="color: #10B981; font-size: 1.1rem;">"&#10003;"</span>
                                        <span style="color: rgba(255, 255, 255, 0.85);">{*framework}</span>
                                    </div>
                                })
                                .collect::<Vec<_>>()}
                        </div>
                        <LicenseClause number="5.3" title="MCP-Native Integration">
                            "The Software's Model Context Protocol (MCP) server provides standardized audit interfaces \
                             for AI-assisted development workflows. All MCP operations are logged and included in \
                             audit trails for complete traceability."
                        </LicenseClause>
                    </div>
                </LicenseSection>

                // ================================================================
                // PAYMENT AND BILLING SECTION
                // ================================================================
                <LicenseSection
                    title="6. Payment and Billing"
                    id="payment"
                >
                    <div style="display: grid; gap: 1.5rem;">
                        <LicenseClause number="6.1" title="Subscription Model">
                            "Paid licenses are offered on monthly or annual subscription terms. \
                             Annual subscriptions receive a 20% discount. Payment is due in advance \
                             and processed via Stripe secure payment infrastructure."
                        </LicenseClause>
                        <LicenseClause number="6.2" title="Automatic Renewal">
                            "Subscriptions automatically renew at the end of each billing period \
                             unless cancelled at least 7 days before renewal. You may cancel at any time \
                             through your account dashboard or by contacting support."
                        </LicenseClause>
                        <LicenseClause number="6.3" title="Refund Policy">
                            "Monthly subscriptions: Pro-rated refund within the first 7 days. \
                             Annual subscriptions: Full refund within the first 30 days. \
                             No refunds for partial months or after the refund period."
                        </LicenseClause>
                    </div>
                </LicenseSection>

                // ================================================================
                // TERMINATION SECTION
                // ================================================================
                <LicenseSection
                    title="7. Termination"
                    id="termination"
                >
                    <div style="display: grid; gap: 1.5rem;">
                        <LicenseClause number="7.1" title="Termination for Breach">
                            "Licensor may terminate this Agreement immediately upon written notice \
                             if you breach any term of this Agreement. Upon termination, you must \
                             immediately cease all use of the Software and destroy all copies in your possession."
                        </LicenseClause>
                        <LicenseClause number="7.2" title="Effect of Termination">
                            "Upon termination: (a) all license rights granted hereunder terminate immediately; \
                             (b) you must return or destroy all copies of the Software; \
                             (c) you must certify in writing that you have complied with these obligations; \
                             (d) audit logs may be exported in read-only format for compliance retention."
                        </LicenseClause>
                        <LicenseClause number="7.3" title="Survival">
                            "Sections 2 (Restrictions), 8 (Warranty Disclaimer), 9 (Limitation of Liability), \
                             and 10 (General Provisions) shall survive any termination of this Agreement."
                        </LicenseClause>
                    </div>
                </LicenseSection>

                // ================================================================
                // WARRANTY DISCLAIMER SECTION
                // ================================================================
                <LicenseSection
                    title="8. Warranty Disclaimer"
                    id="warranty"
                >
                    <div style=move || format!(
                        "{}; \
                         padding: 2rem; \
                         border-left: 4px solid #EF4444;",
                        dark_card_style()
                    )>
                        <p style="color: rgba(255, 255, 255, 0.9); line-height: 1.8; text-transform: uppercase; font-weight: 600;">
                            "THE SOFTWARE IS PROVIDED \"AS IS\" WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, \
                             INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE, \
                             AND NONINFRINGEMENT. LICENSOR DOES NOT WARRANT THAT THE SOFTWARE WILL BE ERROR-FREE, \
                             UNINTERRUPTED, OR FREE OF VIRUSES OR OTHER HARMFUL COMPONENTS. THE ENTIRE RISK AS TO THE \
                             QUALITY AND PERFORMANCE OF THE SOFTWARE IS WITH YOU."
                        </p>
                    </div>
                </LicenseSection>

                // ================================================================
                // LIMITATION OF LIABILITY SECTION
                // ================================================================
                <LicenseSection
                    title="9. Limitation of Liability"
                    id="liability"
                >
                    <div style=move || format!(
                        "{}; \
                         padding: 2rem; \
                         border-left: 4px solid #F59E0B;",
                        dark_card_style()
                    )>
                        <p style="color: rgba(255, 255, 255, 0.9); line-height: 1.8; text-transform: uppercase; font-weight: 600;">
                            "IN NO EVENT SHALL LICENSOR BE LIABLE FOR ANY INDIRECT, INCIDENTAL, SPECIAL, CONSEQUENTIAL, \
                             OR PUNITIVE DAMAGES, INCLUDING BUT NOT LIMITED TO LOSS OF PROFITS, DATA, OR USE, \
                             REGARDLESS OF THE CAUSE OF ACTION OR THE FORM OF ACTION, WHETHER IN CONTRACT, TORT \
                             (INCLUDING NEGLIGENCE), STRICT LIABILITY, OR OTHERWISE, EVEN IF LICENSOR HAS BEEN ADVISED \
                             OF THE POSSIBILITY OF SUCH DAMAGES. LICENSOR'S TOTAL LIABILITY SHALL NOT EXCEED THE \
                             AMOUNTS PAID BY YOU IN THE TWELVE (12) MONTHS PRECEDING THE CLAIM."
                        </p>
                    </div>
                </LicenseSection>

                // ================================================================
                // GENERAL PROVISIONS SECTION
                // ================================================================
                <LicenseSection
                    title="10. General Provisions"
                    id="general"
                >
                    <div style="display: grid; gap: 1.5rem;">
                        <LicenseClause number="10.1" title="Governing Law">
                            "This Agreement shall be governed by and construed in accordance with the laws \
                             of the State of Delaware, United States, without regard to its conflict of law provisions. \
                             Any disputes shall be resolved exclusively in the state or federal courts located in Delaware."
                        </LicenseClause>
                        <LicenseClause number="10.2" title="Assignment">
                            "You may not assign or transfer this Agreement or any rights granted hereunder \
                             without the prior written consent of Licensor. Any attempted assignment in violation \
                             of this section shall be null and void."
                        </LicenseClause>
                        <LicenseClause number="10.3" title="Severability">
                            "If any provision of this Agreement is held to be invalid or unenforceable, \
                             such provision shall be modified to the minimum extent necessary to make it valid \
                             and enforceable, and the remaining provisions shall continue in full force and effect."
                        </LicenseClause>
                        <LicenseClause number="10.4" title="Entire Agreement">
                            "This Agreement constitutes the entire agreement between the parties with respect \
                             to the subject matter hereof and supersedes all prior or contemporaneous understandings, \
                             whether written or oral. No modification of this Agreement shall be binding unless \
                             in writing and signed by both parties."
                        </LicenseClause>
                        <LicenseClause number="10.5" title="Notices">
                            "All notices under this Agreement shall be in writing and sent to: \
                             legal@kindly.software or Kindly Software Ltd., Legal Department, \
                             [Address to be provided upon incorporation]."
                        </LicenseClause>
                    </div>
                </LicenseSection>

                // ================================================================
                // ACCEPTANCE SECTION
                // ================================================================
                <div style="background: rgba(75, 0, 130, 0.3); border: 2px solid #FFD700; border-radius: 16px; padding: 3rem; margin-top: 3rem; text-align: center;">
                    <h2
                        style=move || format!(
                            "{}; \
                             font-size: 1.75rem; \
                             margin-bottom: 1.5rem;",
                            gold_gradient_text()
                        )
                    >
                        "License Acceptance"
                    </h2>
                    <p style="color: rgba(255, 255, 255, 0.9); font-size: 1.1rem; line-height: 1.7; max-width: 800px; margin: 0 auto 2rem;">
                        "BY DOWNLOADING, INSTALLING, OR USING THE KINDLY DEBUGGER SOFTWARE, YOU ACKNOWLEDGE \
                         THAT YOU HAVE READ THIS AGREEMENT, UNDERSTAND IT, AND AGREE TO BE BOUND BY ITS TERMS AND CONDITIONS."
                    </p>
                    <div style="display: flex; gap: 1.5rem; justify-content: center; flex-wrap: wrap;">
                        <a
                            href="/pricing"
                            style="display: inline-block; \
                                   background: linear-gradient(135deg, #FFD700 0%, #FFED4E 100%); \
                                   color: #1A0026; \
                                   padding: 1rem 2.5rem; \
                                   border-radius: 12px; \
                                   font-weight: 700; \
                                   font-size: 1.1rem; \
                                   text-decoration: none; \
                                   transition: all 0.3s ease; \
                                   box-shadow: 0 8px 16px rgba(255, 215, 0, 0.3);"
                        >
                            "View Pricing Plans"
                        </a>
                        <a
                            href="mailto:sales@kindly.software"
                            style="display: inline-block; \
                                   background: rgba(255, 215, 0, 0.2); \
                                   color: #FFD700; \
                                   padding: 1rem 2.5rem; \
                                   border: 1px solid rgba(255, 215, 0, 0.4); \
                                   border-radius: 12px; \
                                   font-weight: 700; \
                                   font-size: 1.1rem; \
                                   text-decoration: none; \
                                   transition: all 0.3s ease;"
                        >
                            "Contact Sales"
                        </a>
                    </div>
                </div>

                // ================================================================
                // FOOTER
                // ================================================================
                <div style="text-align: center; margin-top: 4rem; padding-top: 2rem; border-top: 1px solid rgba(255, 215, 0, 0.2);">
                    <p style="color: rgba(255, 255, 255, 0.5); font-size: 0.9rem; margin-bottom: 0.5rem;">
                        "Kindly Debugger is a trademark of Kindly Software Ltd."
                    </p>
                    <p style="color: rgba(255, 255, 255, 0.4); font-size: 0.85rem;">
                        "Copyright 2025 Kindly Software Ltd. All rights reserved."
                    </p>
                </div>
            </div>
        </div>
    }
}

// ============================================================================
// LICENSE SECTION COMPONENT
// ============================================================================

/// Section wrapper for license agreement sections
#[component]
fn LicenseSection(
    title: &'static str,
    id: &'static str,
    children: Children,
) -> impl IntoView {
    view! {
        <section
            id=id
            style=move || format!(
                "{}; \
                 padding: 2.5rem; \
                 margin-bottom: 2rem;",
                dark_card_style()
            )
        >
            <h2 style="color: #FFD700; font-size: 1.5rem; margin-bottom: 1.5rem; font-weight: 700; display: flex; align-items: center; gap: 0.75rem;">
                <span style="display: inline-block; width: 4px; height: 1.5rem; background: linear-gradient(180deg, #FFD700, #D4AF37); border-radius: 2px;"></span>
                {title}
            </h2>
            {children()}
        </section>
    }
}

// ============================================================================
// LICENSE CLAUSE COMPONENT
// ============================================================================

/// Individual clause within a license section
#[component]
fn LicenseClause(
    number: &'static str,
    title: &'static str,
    children: Children,
) -> impl IntoView {
    view! {
        <div style="padding-left: 0;">
            <h3 style="color: #FFED4E; font-size: 1.1rem; margin-bottom: 0.75rem; font-weight: 600;">
                <span style="color: rgba(255, 215, 0, 0.7); margin-right: 0.5rem;">{number}</span>
                {title}
            </h3>
            <p style="color: rgba(255, 255, 255, 0.85); line-height: 1.7; text-align: justify;">
                {children()}
            </p>
        </div>
    }
}

// ============================================================================
// LICENSE TIER CARD COMPONENT
// ============================================================================

/// Pricing tier card for license selection
#[component]
fn LicenseTierCard(tier: &'static LicenseTier) -> impl IntoView {
    let border_style = if tier.is_featured {
        "border: 2px solid #FFD700;"
    } else {
        "border: 1px solid rgba(255, 215, 0, 0.3);"
    };

    let scale_style = if tier.is_featured {
        "transform: scale(1.02);"
    } else {
        ""
    };

    view! {
        <div style=format!(
            "background: rgba(75, 0, 130, 0.25); \
             {} \
             border-radius: 16px; \
             padding: 2rem; \
             position: relative; \
             {} \
             transition: all 0.3s ease;",
            border_style,
            scale_style
        )>
            // Badge if present
            {tier.badge.map(|badge| view! {
                <div style=format!(
                    "position: absolute; \
                     top: -0.75rem; \
                     left: 1rem; \
                     background: {}; \
                     color: {}; \
                     padding: 0.375rem 0.75rem; \
                     border-radius: 6px; \
                     font-weight: 700; \
                     font-size: 0.75rem; \
                     letter-spacing: 0.05em;",
                    if tier.name == "Enterprise" { "rgba(102, 51, 153, 0.9)" } else { "linear-gradient(135deg, #FFD700 0%, #FFED4E 100%)" },
                    if tier.name == "Enterprise" { "#FFD700" } else { "#1A0026" }
                )>
                    {badge}
                </div>
            })}

            // Tier name
            <h3 style=format!(
                "color: {}; \
                 font-size: 1.5rem; \
                 margin-top: {}; \
                 margin-bottom: 0.5rem; \
                 font-weight: 800;",
                if tier.is_featured { "#FFD700" } else { "rgba(255, 255, 255, 0.95)" },
                if tier.badge.is_some() { "1rem" } else { "0" }
            )>
                {tier.name}
            </h3>

            // Description
            <p style="color: rgba(255, 255, 255, 0.7); font-size: 0.9rem; margin-bottom: 1.5rem;">
                {tier.description}
            </p>

            // Price
            <div style="margin-bottom: 1.5rem;">
                <span style="font-size: 2.25rem; color: #FFED4E; font-weight: 800;">
                    {tier.price}
                </span>
                <span style="color: rgba(255, 255, 255, 0.6); font-size: 0.95rem;">
                    {tier.price_period}
                </span>
            </div>

            // Usage limits
            <div style="background: rgba(0, 0, 0, 0.2); border-radius: 8px; padding: 1rem; margin-bottom: 1.5rem;">
                <div style="display: flex; justify-content: space-between; margin-bottom: 0.5rem;">
                    <span style="color: rgba(255, 255, 255, 0.7); font-size: 0.85rem;">"Requests/month"</span>
                    <span style="color: #FFED4E; font-weight: 600; font-size: 0.85rem;">{tier.requests_per_month}</span>
                </div>
                <div style="display: flex; justify-content: space-between; margin-bottom: 0.5rem;">
                    <span style="color: rgba(255, 255, 255, 0.7); font-size: 0.85rem;">"Seats"</span>
                    <span style="color: #FFED4E; font-weight: 600; font-size: 0.85rem;">{tier.seats}</span>
                </div>
                <div style="display: flex; justify-content: space-between;">
                    <span style="color: rgba(255, 255, 255, 0.7); font-size: 0.85rem;">"Snapshot retention"</span>
                    <span style="color: #FFED4E; font-weight: 600; font-size: 0.85rem;">{tier.snapshot_retention}</span>
                </div>
            </div>

            // Features list
            <ul style="list-style: none; padding: 0; margin: 0 0 1.5rem 0;">
                {tier.features.iter().map(|feature| view! {
                    <li style="color: rgba(255, 255, 255, 0.85); margin-bottom: 0.5rem; display: flex; align-items: flex-start; gap: 0.5rem; font-size: 0.9rem;">
                        <span style="color: #10B981; flex-shrink: 0;">"&#10003;"</span>
                        <span>{*feature}</span>
                    </li>
                }).collect::<Vec<_>>()}
            </ul>

            // Support level
            <div style="border-top: 1px solid rgba(255, 215, 0, 0.2); padding-top: 1rem; margin-bottom: 1.5rem;">
                <span style="color: rgba(255, 255, 255, 0.6); font-size: 0.85rem;">"Support: "</span>
                <span style="color: rgba(255, 255, 255, 0.9); font-weight: 600; font-size: 0.85rem;">{tier.support}</span>
            </div>

            // CTA button
            {if tier.name == "Hobby" {
                view! {
                    <a
                        href="https://github.com/kindly-software/kdb/releases"
                        style="display: block; \
                               width: 100%; \
                               padding: 0.875rem; \
                               text-align: center; \
                               background: rgba(255, 215, 0, 0.15); \
                               color: #FFD700; \
                               border: 1px solid rgba(255, 215, 0, 0.3); \
                               border-radius: 8px; \
                               font-weight: 700; \
                               font-size: 0.95rem; \
                               text-decoration: none; \
                               transition: all 0.3s ease;"
                    >
                        "Download Free"
                    </a>
                }.into_any()
            } else if tier.name == "Enterprise" {
                view! {
                    <a
                        href="mailto:enterprise@kindly.software"
                        style="display: block; \
                               width: 100%; \
                               padding: 0.875rem; \
                               text-align: center; \
                               background: rgba(102, 51, 153, 0.6); \
                               color: #FFD700; \
                               border: 1px solid rgba(255, 215, 0, 0.4); \
                               border-radius: 8px; \
                               font-weight: 700; \
                               font-size: 0.95rem; \
                               text-decoration: none; \
                               transition: all 0.3s ease;"
                    >
                        "Contact Sales"
                    </a>
                }.into_any()
            } else if tier.is_featured {
                view! {
                    <a
                        href="/pricing"
                        style="display: block; \
                               width: 100%; \
                               padding: 0.875rem; \
                               text-align: center; \
                               background: linear-gradient(135deg, #FFD700 0%, #FFED4E 100%); \
                               color: #1A0026; \
                               border: none; \
                               border-radius: 8px; \
                               font-weight: 700; \
                               font-size: 0.95rem; \
                               text-decoration: none; \
                               transition: all 0.3s ease; \
                               box-shadow: 0 4px 12px rgba(255, 215, 0, 0.3);"
                    >
                        "Get Started"
                    </a>
                }.into_any()
            } else {
                view! {
                    <a
                        href="/pricing"
                        style="display: block; \
                               width: 100%; \
                               padding: 0.875rem; \
                               text-align: center; \
                               background: rgba(255, 215, 0, 0.2); \
                               color: #FFD700; \
                               border: 1px solid rgba(255, 215, 0, 0.3); \
                               border-radius: 8px; \
                               font-weight: 700; \
                               font-size: 0.95rem; \
                               text-decoration: none; \
                               transition: all 0.3s ease;"
                    >
                        "Subscribe"
                    </a>
                }.into_any()
            }}
        </div>
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_license_tiers_count() {
        assert_eq!(LICENSE_TIERS.len(), 5);
    }

    #[test]
    fn test_license_tier_names() {
        let names: Vec<&str> = LICENSE_TIERS.iter().map(|t| t.name).collect();
        assert_eq!(names, vec!["Hobby", "Starter", "Developer", "Professional", "Enterprise"]);
    }

    #[test]
    fn test_featured_tier_is_developer() {
        let featured = LICENSE_TIERS.iter().find(|t| t.is_featured);
        assert!(featured.is_some());
        assert_eq!(featured.unwrap().name, "Developer");
    }

    #[test]
    fn test_hobby_tier_is_free() {
        let hobby = LICENSE_TIERS.iter().find(|t| t.name == "Hobby");
        assert!(hobby.is_some());
        assert_eq!(hobby.unwrap().price, "Free");
    }

    #[test]
    fn test_enterprise_has_unlimited_seats() {
        let enterprise = LICENSE_TIERS.iter().find(|t| t.name == "Enterprise");
        assert!(enterprise.is_some());
        assert_eq!(enterprise.unwrap().seats, "Unlimited");
    }

    #[test]
    fn test_all_tiers_have_features() {
        for tier in LICENSE_TIERS {
            assert!(!tier.features.is_empty(), "Tier {} should have features", tier.name);
        }
    }

    #[test]
    fn test_professional_tier_seats() {
        let pro = LICENSE_TIERS.iter().find(|t| t.name == "Professional");
        assert!(pro.is_some());
        assert_eq!(pro.unwrap().seats, "5");
    }

    #[test]
    fn test_license_page_compiles() {
        // Ensures component compiles
    }
}
