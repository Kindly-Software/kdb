//! Footer Component
//!
//! Minimal footer with links and branding.

use leptos::prelude::*;

/// Premium footer
#[component]
pub fn Footer() -> impl IntoView {
    let footer_style = "
        padding: 4rem 2rem 2rem;
        position: relative;
        z-index: 1;
        border-top: 1px solid rgba(255, 255, 255, 0.1);
    ";

    let container_style = "
        max-width: 1200px;
        margin: 0 auto;
    ";

    let grid_style = "
        display: grid;
        grid-template-columns: repeat(auto-fit, minmax(min(100%, 180px), 1fr));
        gap: 3rem;
        margin-bottom: 3rem;
    ";

    let brand_column_style = "
        max-width: 300px;
    ";

    let logo_style = "
        font-family: 'Space Grotesk', sans-serif;
        font-size: 1.5rem;
        font-weight: 700;
        color: #fff;
        text-decoration: none;
        display: flex;
        align-items: center;
        gap: 0.5rem;
        margin-bottom: 1rem;
    ";

    let tagline_style = "
        font-size: 0.9375rem;
        color: rgba(255, 255, 255, 0.6);
        line-height: 1.6;
    ";

    let column_title_style = "
        font-family: 'Space Grotesk', sans-serif;
        font-size: 0.875rem;
        font-weight: 600;
        color: rgba(255, 255, 255, 0.5);
        text-transform: uppercase;
        letter-spacing: 0.05em;
        margin-bottom: 1rem;
    ";

    let link_list_style = "
        list-style: none;
        padding: 0;
        margin: 0;
    ";

    let link_item_style = "
        margin-bottom: 0.5rem;
    ";

    let link_style = "
        color: rgba(255, 255, 255, 0.7);
        text-decoration: none;
        font-size: 0.9375rem;
        transition: color 0.2s ease;
    ";

    let bottom_style = "
        display: flex;
        justify-content: space-between;
        align-items: center;
        flex-wrap: wrap;
        gap: 1rem;
        padding-top: 2rem;
        border-top: 1px solid rgba(255, 255, 255, 0.1);
    ";

    let copyright_style = "
        font-size: 0.875rem;
        color: rgba(255, 255, 255, 0.5);
    ";

    let social_style = "
        display: flex;
        gap: 1rem;
    ";

    let social_link_style = "
        color: rgba(255, 255, 255, 0.6);
        font-size: 1.25rem;
        text-decoration: none;
        transition: color 0.2s ease;
    ";

    view! {
        <footer style=footer_style>
            <div style=container_style>
                <div class="footer-grid" style=grid_style>
                    // Brand column
                    <div style=brand_column_style>
                        <a href="/" style=logo_style>
                            <span style="font-size: 1.8rem;">"🐛"</span>
                            <span>"Kindly "</span>
                            <span style="color: #FFD700;">"Debugger"</span>
                        </a>
                        <p style=tagline_style>
                            "The first audit-compliant time-travel debugger. "
                            "Platform-agnostic via MCP."
                        </p>
                    </div>

                    // Product column
                    <div>
                        <h4 style=column_title_style>"Product"</h4>
                        <ul style=link_list_style>
                            <li style=link_item_style>
                                <a href="/#features" style=link_style>"Features"</a>
                            </li>
                            <li style=link_item_style>
                                <a href="/#pricing" style=link_style>"Pricing"</a>
                            </li>
                            <li style=link_item_style>
                                <a href="#docs" style=link_style>"Documentation"</a>
                            </li>
                            <li style=link_item_style>
                                <a href="#signup" style=link_style>"Get Started"</a>
                            </li>
                        </ul>
                    </div>

                    // Resources column
                    <div>
                        <h4 style=column_title_style>"Resources"</h4>
                        <ul style=link_list_style>
                            <li style=link_item_style>
                                <a href="#docs" style=link_style>"Documentation"</a>
                            </li>
                            <li style=link_item_style>
                                <a href="mailto:support@kindly.software" style=link_style>"Support"</a>
                            </li>
                        </ul>
                    </div>

                    // Legal column
                    <div>
                        <h4 style=column_title_style>"Legal"</h4>
                        <ul style=link_list_style>
                            <li style=link_item_style>
                                <a href="#privacy" style=link_style>"Privacy Policy"</a>
                            </li>
                            <li style=link_item_style>
                                <a href="#terms" style=link_style>"Terms of Service"</a>
                            </li>
                            <li style=link_item_style>
                                <a href="#license" style=link_style>"License"</a>
                            </li>
                        </ul>
                    </div>
                </div>

                <div style=bottom_style>
                    <p style=copyright_style>
                        "© 2025 Kindly Software Inc. All rights reserved."
                    </p>
                    <div style=social_style>
                        <a href="mailto:support@kindly.software" style=social_link_style title="Contact" aria-label="Contact">
                            "✉️"
                        </a>
                    </div>
                </div>
            </div>
        </footer>
    }
}
