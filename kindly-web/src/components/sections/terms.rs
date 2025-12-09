//! Terms of Service - Kindly Debugger SaaS
//!
//! Comprehensive legal terms for the Kindly Debugger SaaS product.
//! Byzantine Royal purple design with glassmorphic sections.

use leptos::prelude::*;
use crate::utils::glassmorphism::{byzantine_background, card_style, gold_gradient_text};

/// Terms section with numbered clause
#[derive(Clone)]
struct TermsSection {
    number: u8,
    title: &'static str,
    clauses: Vec<TermsClause>,
}

/// Individual clause within a section
#[derive(Clone)]
struct TermsClause {
    number: &'static str,
    content: &'static str,
}

/// Terms of Service component
#[component]
pub fn Terms() -> impl IntoView {
    let sections = get_terms_sections();

    view! {
        <section
            class="terms-of-service"
            id="terms"
            style=move || format!(
                "{}; \
                 padding: 120px 2rem 80px 2rem; \
                 position: relative; \
                 min-height: 100vh;",
                byzantine_background()
            )
        >
            <div style="max-width: 900px; margin: 0 auto;">
                // Header
                <div class="terms-header" style="text-align: center; margin-bottom: 3rem;">
                    <h1
                        style=move || format!(
                            "{}; \
                             font-size: clamp(2rem, 5vw, 3.5rem); \
                             margin-bottom: 1rem;",
                            gold_gradient_text()
                        )
                    >
                        "Terms of Service"
                    </h1>
                    <p
                        style="color: rgba(255, 255, 255, 0.85); \
                               font-size: 1.125rem; \
                               line-height: 1.6; \
                               margin-bottom: 0.5rem;"
                    >
                        "Kindly Debugger - Time-Travel Debugging SaaS"
                    </p>
                    <p
                        style="color: rgba(255, 215, 0, 0.8); \
                               font-size: 0.95rem; \
                               font-weight: 600;"
                    >
                        "Effective Date: December 4, 2025"
                    </p>
                </div>

                // Introduction
                <div
                    class="terms-intro"
                    style=move || format!(
                        "{}; \
                         padding: 2rem; \
                         margin-bottom: 2rem;",
                        card_style()
                    )
                >
                    <p style="color: rgba(255, 255, 255, 0.9); line-height: 1.8; margin-bottom: 1rem;">
                        "These Terms of Service (\"Terms\") constitute a legally binding agreement between you (\"User,\" \"you,\" or \"your\") and Kindly Software, Inc., a Delaware corporation (\"Kindly,\" \"we,\" \"us,\" or \"our\"), governing your access to and use of the Kindly Debugger service, including the time-travel debugging platform, MCP protocol integrations, and related software (collectively, the \"Service\")."
                    </p>
                    <p style="color: rgba(255, 255, 255, 0.9); line-height: 1.8;">
                        "BY ACCESSING OR USING THE SERVICE, YOU ACKNOWLEDGE THAT YOU HAVE READ, UNDERSTOOD, AND AGREE TO BE BOUND BY THESE TERMS. IF YOU DO NOT AGREE TO THESE TERMS, YOU MAY NOT ACCESS OR USE THE SERVICE."
                    </p>
                </div>

                // All sections
                <div class="terms-sections" style="display: flex; flex-direction: column; gap: 2rem;">
                    {sections
                        .into_iter()
                        .map(|section| {
                            view! {
                                <div
                                    class="terms-section"
                                    style=move || format!(
                                        "{}; \
                                         padding: 2rem;",
                                        card_style()
                                    )
                                >
                                    <h2
                                        style="color: #FFD700; \
                                               font-size: 1.5rem; \
                                               font-weight: 700; \
                                               margin-bottom: 1.5rem; \
                                               border-bottom: 1px solid rgba(255, 215, 0, 0.3); \
                                               padding-bottom: 0.75rem;"
                                    >
                                        {format!("{}. {}", section.number, section.title)}
                                    </h2>
                                    <div class="clauses" style="display: flex; flex-direction: column; gap: 1rem;">
                                        {section.clauses
                                            .into_iter()
                                            .map(|clause| {
                                                view! {
                                                    <div class="clause" style="padding-left: 1rem;">
                                                        <p style="color: rgba(255, 255, 255, 0.9); line-height: 1.7;">
                                                            <span style="color: #FFED4E; font-weight: 600; margin-right: 0.5rem;">
                                                                {format!("{}", clause.number)}
                                                            </span>
                                                            {clause.content}
                                                        </p>
                                                    </div>
                                                }
                                            })
                                            .collect_view()}
                                    </div>
                                </div>
                            }
                        })
                        .collect_view()}
                </div>

                // Contact Information
                <div
                    class="terms-contact"
                    style=move || format!(
                        "{}; \
                         padding: 2rem; \
                         margin-top: 2rem; \
                         text-align: center;",
                        card_style()
                    )
                >
                    <h3 style="color: #FFD700; font-size: 1.25rem; font-weight: 600; margin-bottom: 1rem;">
                        "Contact Information"
                    </h3>
                    <p style="color: rgba(255, 255, 255, 0.85); line-height: 1.6; margin-bottom: 0.5rem;">
                        "For questions about these Terms, please contact:"
                    </p>
                    <p style="color: rgba(255, 255, 255, 0.9); line-height: 1.8;">
                        "Kindly Software, Inc."<br/>
                        "Email: "
                        <a
                            href="mailto:legal@kindly.software"
                            style="color: #FFD700; text-decoration: underline; font-weight: 600;"
                        >
                            "legal@kindly.software"
                        </a>
                    </p>
                </div>

                // Version Notice
                <div style="text-align: center; margin-top: 2rem; color: rgba(255, 255, 255, 0.6); font-size: 0.875rem;">
                    <p>"Version 1.0 | Last Updated: December 4, 2025"</p>
                </div>
            </div>
        </section>
    }
}

/// Generate all terms sections with their clauses
fn get_terms_sections() -> Vec<TermsSection> {
    vec![
        // Section 1: Acceptance of Terms
        TermsSection {
            number: 1,
            title: "ACCEPTANCE OF TERMS",
            clauses: vec![
                TermsClause {
                    number: "1.1",
                    content: "By creating an account, accessing the Service, or clicking \"I Agree,\" you represent that you are at least 18 years of age and have the legal capacity to enter into this agreement.",
                },
                TermsClause {
                    number: "1.2",
                    content: "If you are accepting these Terms on behalf of a company, organization, or other legal entity, you represent and warrant that you have the authority to bind such entity to these Terms.",
                },
                TermsClause {
                    number: "1.3",
                    content: "We reserve the right to modify these Terms at any time. Material changes will be communicated via email or prominent notice on the Service at least thirty (30) days prior to taking effect. Continued use after such notice constitutes acceptance of the modified Terms.",
                },
            ],
        },
        // Section 2: Account Registration
        TermsSection {
            number: 2,
            title: "ACCOUNT REGISTRATION AND RESPONSIBILITIES",
            clauses: vec![
                TermsClause {
                    number: "2.1",
                    content: "You must provide accurate, current, and complete registration information and maintain the accuracy of such information throughout your use of the Service.",
                },
                TermsClause {
                    number: "2.2",
                    content: "You are responsible for maintaining the confidentiality of your account credentials and for all activities that occur under your account. You must immediately notify us of any unauthorized use of your account.",
                },
                TermsClause {
                    number: "2.3",
                    content: "Each account is for a single user only. Account sharing, including sharing of API keys or MCP tokens, is prohibited except as expressly permitted under Enterprise tier agreements.",
                },
                TermsClause {
                    number: "2.4",
                    content: "We reserve the right to suspend or terminate accounts that remain inactive for more than twelve (12) consecutive months.",
                },
            ],
        },
        // Section 3: Acceptable Use Policy
        TermsSection {
            number: 3,
            title: "ACCEPTABLE USE POLICY",
            clauses: vec![
                TermsClause {
                    number: "3.1",
                    content: "You agree to use the Service only for lawful purposes and in accordance with these Terms. You shall not use the Service to debug, analyze, reverse engineer, decompile, or otherwise examine the software, systems, or services of third parties without their express authorization.",
                },
                TermsClause {
                    number: "3.2",
                    content: "Prohibited activities include, but are not limited to: (a) competitive analysis or benchmarking of competing debugging products; (b) systematic scraping or data extraction; (c) circumventing usage limits, rate limiting, or access controls; (d) interfering with or disrupting the Service infrastructure.",
                },
                TermsClause {
                    number: "3.3",
                    content: "You shall not use the Service to process, store, or transmit any content that: (a) infringes intellectual property rights; (b) contains malware or malicious code; (c) violates applicable laws or regulations; (d) constitutes unsolicited commercial communications.",
                },
                TermsClause {
                    number: "3.4",
                    content: "MCP Protocol Usage: The Model Context Protocol (MCP) integration is provided for legitimate debugging purposes only. Using MCP connections to probe, attack, or exploit AI systems or their underlying infrastructure is strictly prohibited.",
                },
                TermsClause {
                    number: "3.5",
                    content: "Violation of this Acceptable Use Policy may result in immediate suspension or termination of your account without refund, and may be reported to appropriate law enforcement authorities.",
                },
            ],
        },
        // Section 4: Service Availability and SLA
        TermsSection {
            number: 4,
            title: "SERVICE AVAILABILITY AND SLA",
            clauses: vec![
                TermsClause {
                    number: "4.1",
                    content: "Free Tier: The Service is provided \"as is\" without any uptime commitment. We may limit, suspend, or discontinue Free Tier access at any time without notice.",
                },
                TermsClause {
                    number: "4.2",
                    content: "Paid Tiers (Pro and Enterprise): We commit to 99.9% monthly uptime (\"SLA\"), calculated as: ((Total Minutes - Downtime Minutes) / Total Minutes) * 100. Scheduled maintenance, announced at least 72 hours in advance, is excluded from downtime calculations.",
                },
                TermsClause {
                    number: "4.3",
                    content: "SLA Credits: If monthly uptime falls below 99.9%, eligible paid users may request service credits: 99.0%-99.9% = 10% credit; 95.0%-99.0% = 25% credit; below 95.0% = 50% credit. Credits must be requested within 30 days and are applied to future billing only.",
                },
                TermsClause {
                    number: "4.4",
                    content: "Force Majeure: We shall not be liable for any failure to meet the SLA due to circumstances beyond our reasonable control, including natural disasters, acts of government, telecommunications failures, or cyberattacks.",
                },
            ],
        },
        // Section 5: Intellectual Property
        TermsSection {
            number: 5,
            title: "INTELLECTUAL PROPERTY",
            clauses: vec![
                TermsClause {
                    number: "5.1",
                    content: "Kindly Ownership: The Service, including all software, algorithms, user interfaces, documentation, and the proprietary time-travel debugging technology (including Q34 audit trail systems and lockfree capsule architecture), is owned exclusively by Kindly Software, Inc. and protected by intellectual property laws.",
                },
                TermsClause {
                    number: "5.2",
                    content: "User Data Ownership: You retain all ownership rights to your debug session data, source code, and any other content you upload or create using the Service (\"User Data\"). We claim no intellectual property rights over User Data.",
                },
                TermsClause {
                    number: "5.3",
                    content: "License to User Data: You grant us a limited, non-exclusive license to process, store, and transmit User Data solely for the purpose of providing the Service to you. This license terminates upon termination of your account.",
                },
                TermsClause {
                    number: "5.4",
                    content: "Feedback: Any suggestions, ideas, or feedback you provide regarding the Service may be used by us without any obligation to you.",
                },
                TermsClause {
                    number: "5.5",
                    content: "Trade Secrets: The computational capsule architecture, lockfree coordination patterns, and time-travel debugging algorithms constitute trade secrets. Any attempt to reverse engineer, decompile, or extract these proprietary technologies is strictly prohibited.",
                },
            ],
        },
        // Section 6: Debug Data and Audit Trails
        TermsSection {
            number: 6,
            title: "DEBUG DATA AND AUDIT TRAILS",
            clauses: vec![
                TermsClause {
                    number: "6.1",
                    content: "Debug Session Data: All debug session data, including execution traces, snapshots, and time-travel history, is owned by you. You may export your data at any time using the provided export tools.",
                },
                TermsClause {
                    number: "6.2",
                    content: "Snapshot Retention: Time-travel debugging snapshots are retained according to your tier: Free Tier: 7 days; Pro Tier: 30 days; Enterprise Tier: Unlimited (as specified in your Enterprise agreement). Snapshots are automatically deleted after the retention period unless exported.",
                },
                TermsClause {
                    number: "6.3",
                    content: "Q34 Audit Trail Immutability: Our Service maintains cryptographic hash-chain audit trails for compliance purposes (Q34 framework). IMPORTANT: Due to the immutable nature of these audit trails, we cannot delete or modify audit log entries. Audit trails include: timestamps, user actions, session identifiers, and integrity hashes. Audit trails do NOT include: source code, variable values, or debug content.",
                },
                TermsClause {
                    number: "6.4",
                    content: "Data Deletion Requests: Upon account termination, we will delete all User Data within 30 days, except for audit trail entries which are retained for legal and compliance purposes (minimum 7 years). You may request a copy of your audit trail entries prior to account deletion.",
                },
                TermsClause {
                    number: "6.5",
                    content: "Enterprise Data Residency: Enterprise customers may specify data residency requirements (US, EU, or custom). Data will be stored and processed exclusively in the specified region(s).",
                },
            ],
        },
        // Section 7: Payment Terms
        TermsSection {
            number: 7,
            title: "PAYMENT TERMS",
            clauses: vec![
                TermsClause {
                    number: "7.1",
                    content: "Billing Cycles: Paid subscriptions are billed either monthly or annually, as selected at the time of purchase. Annual subscriptions receive a discount as displayed on our pricing page.",
                },
                TermsClause {
                    number: "7.2",
                    content: "Automatic Renewal: Subscriptions automatically renew at the end of each billing period unless cancelled at least 24 hours before the renewal date. Pricing may be adjusted upon renewal with 30 days prior notice.",
                },
                TermsClause {
                    number: "7.3",
                    content: "Payment Methods: We accept major credit cards, debit cards, and other payment methods as displayed during checkout. You authorize us to charge your payment method for all fees incurred.",
                },
                TermsClause {
                    number: "7.4",
                    content: "Refunds: Monthly subscriptions: No refunds for partial months. Annual subscriptions: Pro-rated refund available within the first 30 days if you are unsatisfied with the Service. Enterprise agreements: Refund terms as specified in your Enterprise agreement.",
                },
                TermsClause {
                    number: "7.5",
                    content: "Taxes: Fees are exclusive of applicable taxes. You are responsible for all taxes, levies, or duties imposed by taxing authorities, except for taxes based on Kindly's net income.",
                },
                TermsClause {
                    number: "7.6",
                    content: "Late Payment: Accounts with overdue payments may be suspended after 7 days and terminated after 30 days. We reserve the right to charge interest on overdue amounts at the rate of 1.5% per month.",
                },
            ],
        },
        // Section 8: Termination and Suspension
        TermsSection {
            number: 8,
            title: "TERMINATION AND SUSPENSION",
            clauses: vec![
                TermsClause {
                    number: "8.1",
                    content: "Termination by You: You may terminate your account at any time through the account settings or by contacting support. Termination does not entitle you to a refund except as provided in Section 7.4.",
                },
                TermsClause {
                    number: "8.2",
                    content: "Termination by Kindly: We may terminate or suspend your account immediately, without prior notice, for: (a) breach of these Terms; (b) violation of the Acceptable Use Policy; (c) non-payment; (d) as required by law; (e) extended inactivity.",
                },
                TermsClause {
                    number: "8.3",
                    content: "Effect of Termination: Upon termination: (a) your right to use the Service ceases immediately; (b) we may delete your User Data after 30 days; (c) any outstanding payment obligations remain due; (d) Sections 5, 6.3, 9, 10, 11, and 12 survive termination.",
                },
                TermsClause {
                    number: "8.4",
                    content: "Data Export: You have 30 days from termination notice to export your User Data. After this period, we have no obligation to maintain or provide access to your data.",
                },
            ],
        },
        // Section 9: Limitation of Liability
        TermsSection {
            number: 9,
            title: "LIMITATION OF LIABILITY",
            clauses: vec![
                TermsClause {
                    number: "9.1",
                    content: "TO THE MAXIMUM EXTENT PERMITTED BY LAW, KINDLY SHALL NOT BE LIABLE FOR ANY INDIRECT, INCIDENTAL, SPECIAL, CONSEQUENTIAL, OR PUNITIVE DAMAGES, INCLUDING BUT NOT LIMITED TO LOSS OF PROFITS, DATA, BUSINESS OPPORTUNITIES, OR GOODWILL, ARISING OUT OF OR RELATED TO THESE TERMS OR THE SERVICE.",
                },
                TermsClause {
                    number: "9.2",
                    content: "OUR TOTAL LIABILITY FOR ANY CLAIMS ARISING FROM OR RELATED TO THE SERVICE SHALL NOT EXCEED THE GREATER OF: (A) THE AMOUNTS PAID BY YOU TO KINDLY IN THE TWELVE (12) MONTHS PRECEDING THE CLAIM; OR (B) ONE HUNDRED US DOLLARS ($100).",
                },
                TermsClause {
                    number: "9.3",
                    content: "THE SERVICE IS PROVIDED \"AS IS\" AND \"AS AVAILABLE\" WITHOUT WARRANTIES OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE, NON-INFRINGEMENT, OR THAT THE SERVICE WILL BE UNINTERRUPTED OR ERROR-FREE.",
                },
                TermsClause {
                    number: "9.4",
                    content: "SOME JURISDICTIONS DO NOT ALLOW THE EXCLUSION OF CERTAIN WARRANTIES OR LIMITATIONS ON LIABILITY. IN SUCH JURISDICTIONS, OUR LIABILITY SHALL BE LIMITED TO THE MAXIMUM EXTENT PERMITTED BY LAW.",
                },
            ],
        },
        // Section 10: Indemnification
        TermsSection {
            number: 10,
            title: "INDEMNIFICATION",
            clauses: vec![
                TermsClause {
                    number: "10.1",
                    content: "You agree to indemnify, defend, and hold harmless Kindly, its officers, directors, employees, agents, and affiliates from and against any claims, liabilities, damages, losses, and expenses (including reasonable attorneys' fees) arising out of or related to: (a) your use of the Service; (b) your violation of these Terms; (c) your violation of any third-party rights; (d) your User Data.",
                },
                TermsClause {
                    number: "10.2",
                    content: "Kindly reserves the right to assume the exclusive defense of any matter subject to indemnification by you, in which case you will cooperate with us in asserting any available defenses.",
                },
            ],
        },
        // Section 11: Dispute Resolution
        TermsSection {
            number: 11,
            title: "DISPUTE RESOLUTION",
            clauses: vec![
                TermsClause {
                    number: "11.1",
                    content: "Governing Law: These Terms shall be governed by and construed in accordance with the laws of the State of Delaware, United States, without regard to its conflict of law provisions.",
                },
                TermsClause {
                    number: "11.2",
                    content: "Informal Resolution: Before initiating formal proceedings, you agree to first contact us at legal@kindly.software to attempt to resolve any dispute informally. We will attempt to resolve disputes within 30 days of receiving notice.",
                },
                TermsClause {
                    number: "11.3",
                    content: "Arbitration: Any dispute not resolved informally shall be resolved by binding arbitration administered by the American Arbitration Association (AAA) under its Commercial Arbitration Rules. The arbitration shall take place in Wilmington, Delaware, unless the parties agree otherwise.",
                },
                TermsClause {
                    number: "11.4",
                    content: "Class Action Waiver: YOU AND KINDLY AGREE THAT ANY DISPUTE RESOLUTION PROCEEDINGS WILL BE CONDUCTED ONLY ON AN INDIVIDUAL BASIS AND NOT IN A CLASS, CONSOLIDATED, OR REPRESENTATIVE ACTION.",
                },
                TermsClause {
                    number: "11.5",
                    content: "Exceptions: Notwithstanding the above, either party may seek injunctive relief in any court of competent jurisdiction for infringement of intellectual property rights or breach of confidentiality obligations.",
                },
                TermsClause {
                    number: "11.6",
                    content: "Enterprise Exception: Enterprise customers with executed Master Service Agreements may have alternative dispute resolution procedures as specified in their agreements.",
                },
            ],
        },
        // Section 12: General Provisions
        TermsSection {
            number: 12,
            title: "GENERAL PROVISIONS",
            clauses: vec![
                TermsClause {
                    number: "12.1",
                    content: "Entire Agreement: These Terms, together with our Privacy Policy and any applicable Enterprise agreements, constitute the entire agreement between you and Kindly regarding the Service and supersede all prior agreements.",
                },
                TermsClause {
                    number: "12.2",
                    content: "Severability: If any provision of these Terms is held invalid or unenforceable, that provision shall be modified to the minimum extent necessary, and the remaining provisions shall continue in full force and effect.",
                },
                TermsClause {
                    number: "12.3",
                    content: "Waiver: No waiver of any term shall be deemed a further or continuing waiver of such term or any other term. Our failure to enforce any provision shall not constitute a waiver of that provision.",
                },
                TermsClause {
                    number: "12.4",
                    content: "Assignment: You may not assign or transfer these Terms without our prior written consent. We may assign these Terms without restriction.",
                },
                TermsClause {
                    number: "12.5",
                    content: "Notices: We may provide notices to you via email, in-app notification, or posting on the Service. Notices to us must be sent to legal@kindly.software and shall be effective upon receipt.",
                },
                TermsClause {
                    number: "12.6",
                    content: "Export Compliance: You agree to comply with all applicable export laws and regulations. The Service may not be exported or re-exported to any country subject to US trade sanctions or to any person or entity on any US government restricted parties list.",
                },
                TermsClause {
                    number: "12.7",
                    content: "Government Use: If you are a US government entity, the Service is provided as \"Commercial Computer Software\" and \"Commercial Computer Software Documentation\" under FAR 12.212 and DFARS 227.7202.",
                },
            ],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terms_compiles() {
        // Ensures component compiles
    }

    #[test]
    fn test_terms_renders() {
        let _ = Terms();
    }

    #[test]
    fn test_sections_not_empty() {
        let sections = get_terms_sections();
        assert!(!sections.is_empty());
        assert_eq!(sections.len(), 12); // 12 main sections
    }

    #[test]
    fn test_all_sections_have_clauses() {
        let sections = get_terms_sections();
        for section in sections {
            assert!(!section.clauses.is_empty(),
                "Section {} ({}) has no clauses", section.number, section.title);
        }
    }

    #[test]
    fn test_clause_numbering_consistent() {
        let sections = get_terms_sections();
        for section in sections {
            for clause in &section.clauses {
                // Verify clause number starts with section number
                let expected_prefix = format!("{}.", section.number);
                assert!(clause.number.starts_with(&expected_prefix),
                    "Clause {} in section {} does not match section numbering",
                    clause.number, section.number);
            }
        }
    }
}
