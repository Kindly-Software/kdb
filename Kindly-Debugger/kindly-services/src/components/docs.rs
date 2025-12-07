//! Documentation Page
//!
//! Public documentation for Kindly Debugger.

use leptos::prelude::*;

/// Documentation page component
#[component]
pub fn Docs() -> impl IntoView {
    let page_style = "
        min-height: 100vh;
        padding: 8rem 2rem 4rem;
        position: relative;
        z-index: 1;
    ";

    let container_style = "
        max-width: 900px;
        margin: 0 auto;
    ";

    let header_style = "
        text-align: center;
        margin-bottom: 4rem;
    ";

    let title_style = "
        font-family: 'Space Grotesk', sans-serif;
        font-size: clamp(2rem, 5vw, 3rem);
        font-weight: 700;
        color: #fff;
        margin-bottom: 1rem;
    ";

    let subtitle_style = "
        font-size: 1.125rem;
        color: rgba(255, 255, 255, 0.7);
        max-width: 600px;
        margin: 0 auto;
    ";

    let nav_style = "
        background: rgba(255, 255, 255, 0.05);
        backdrop-filter: blur(20px);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 16px;
        padding: 1.5rem;
        margin-bottom: 3rem;
    ";

    let nav_title_style = "
        font-family: 'Space Grotesk', sans-serif;
        font-size: 1rem;
        font-weight: 600;
        color: #FFD700;
        margin-bottom: 1rem;
    ";

    let nav_list_style = "
        list-style: none;
        padding: 0;
        margin: 0;
        display: flex;
        flex-wrap: wrap;
        gap: 0.75rem;
    ";

    let nav_link_style = "
        color: rgba(255, 255, 255, 0.8);
        text-decoration: none;
        padding: 0.5rem 1rem;
        background: rgba(255, 255, 255, 0.05);
        border-radius: 8px;
        font-size: 0.875rem;
        transition: background 0.2s ease, color 0.2s ease;
    ";

    let section_style = "
        background: rgba(255, 255, 255, 0.03);
        backdrop-filter: blur(10px);
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 20px;
        padding: 2rem;
        margin-bottom: 2rem;
    ";

    let section_title_style = "
        font-family: 'Space Grotesk', sans-serif;
        font-size: 1.5rem;
        font-weight: 600;
        color: #FFD700;
        margin-bottom: 1.5rem;
        padding-bottom: 0.75rem;
        border-bottom: 1px solid rgba(255, 215, 0, 0.2);
    ";

    let paragraph_style = "
        color: rgba(255, 255, 255, 0.85);
        line-height: 1.8;
        margin-bottom: 1rem;
        font-size: 1rem;
    ";

    let code_block_style = "
        background: rgba(0, 0, 0, 0.4);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 12px;
        padding: 1rem 1.25rem;
        font-family: 'JetBrains Mono', 'Fira Code', monospace;
        font-size: 0.875rem;
        color: #e2e8f0;
        overflow-x: auto;
        margin-bottom: 1rem;
        white-space: pre-wrap;
    ";

    let list_style = "
        color: rgba(255, 255, 255, 0.85);
        line-height: 1.8;
        margin-bottom: 1rem;
        padding-left: 1.5rem;
    ";

    let list_item_style = "
        margin-bottom: 0.5rem;
    ";

    let subsection_title_style = "
        font-family: 'Space Grotesk', sans-serif;
        font-size: 1.125rem;
        font-weight: 600;
        color: #fff;
        margin: 1.5rem 0 1rem;
    ";

    view! {
        <div style=page_style>
            <div style=container_style>
                // Header
                <div style=header_style>
                    <h1 style=title_style>"Kindly Debugger Documentation"</h1>
                    <p style=subtitle_style>
                        "Time-travel debugging for AI workflows"
                    </p>
                </div>

                // Navigation
                <nav style=nav_style>
                    <div style=nav_title_style>"Quick Navigation"</div>
                    <ul style=nav_list_style>
                        <li><a href="#getting-started" style=nav_link_style>"Getting Started"</a></li>
                        <li><a href="#installation" style=nav_link_style>"Installation"</a></li>
                        <li><a href="#usage" style=nav_link_style>"Basic Usage"</a></li>
                        <li><a href="#mcp" style=nav_link_style>"MCP Integration"</a></li>
                        <li><a href="#api" style=nav_link_style>"API Reference"</a></li>
                        <li><a href="#faq" style=nav_link_style>"FAQ"</a></li>
                    </ul>
                </nav>

                // Getting Started
                <section id="getting-started" style=section_style>
                    <h2 style=section_title_style>"Getting Started"</h2>
                    <p style=paragraph_style>
                        "Kindly Debugger (KDB) is a time-travel debugger built for modern development workflows. "
                        "Step forward, step backward, and replay execution to understand bugs as if they never existed."
                    </p>
                    <p style=paragraph_style>
                        "Key features:"
                    </p>
                    <ul style=list_style>
                        <li style=list_item_style>"Bidirectional execution replay (step forward and backward)"</li>
                        <li style=list_item_style>"Instant snapshots for time-travel debugging"</li>
                        <li style=list_item_style>"MCP protocol integration for AI assistants"</li>
                        <li style=list_item_style>"REST API for remote debugging"</li>
                        <li style=list_item_style>"Audit trails for compliance workflows"</li>
                    </ul>
                </section>

                // Installation
                <section id="installation" style=section_style>
                    <h2 style=section_title_style>"Installation"</h2>

                    <h3 style=subsection_title_style>"Open Source (Basic MCP Handshake)"</h3>
                    <p style=paragraph_style>
                        "The open source version provides core time-travel debugging with basic MCP protocol support:"
                    </p>
                    <pre style=code_block_style>
{"# Clone repository
git clone https://github.com/kindly-software/kdb
cd kdb

# Build from source
cargo build --release

# Run debugger
./target/release/kdb"}
                    </pre>

                    <h3 style=subsection_title_style>"Premium Binary (Pre-built)"</h3>
                    <p style=paragraph_style>
                        "Premium licenses include pre-built binaries with enhanced features:"
                    </p>
                    <pre style=code_block_style>
{"# Download from dashboard
# Extract binary
tar -xzf kdb-linux-x64.tar.gz

# Install
sudo mv kdb /usr/local/bin/

# Verify
kdb --version"}
                    </pre>
                </section>

                // Basic Usage
                <section id="usage" style=section_style>
                    <h2 style=section_title_style>"Basic Usage"</h2>

                    <h3 style=subsection_title_style>"Attach to Process"</h3>
                    <pre style=code_block_style>
{"# Attach to running process
kdb attach <pid>

# Attach with auto-snapshot
kdb attach <pid> --snapshot-interval 1s"}
                    </pre>

                    <h3 style=subsection_title_style>"Set Breakpoints"</h3>
                    <pre style=code_block_style>
{"# Set breakpoint at address
break 0x12345678

# Set breakpoint at function (requires symbols)
break main

# List breakpoints
info breakpoints"}
                    </pre>

                    <h3 style=subsection_title_style>"Time-Travel Commands"</h3>
                    <pre style=code_block_style>
{"# Step forward (like GDB)
step
s

# Step backward (time-travel)
back
b

# Continue execution
continue
c

# Take manual snapshot
snapshot"}
                    </pre>

                    <h3 style=subsection_title_style>"Inspect State"</h3>
                    <pre style=code_block_style>
{"# View stack trace
stack
bt

# View registers
info registers

# Examine memory
x 0x7fff12340000 64

# Detach from process
quit"}
                    </pre>
                </section>

                // MCP Integration
                <section id="mcp" style=section_style>
                    <h2 style=section_title_style>"MCP Integration"</h2>

                    <p style=paragraph_style>
                        "Kindly Debugger supports the Model Context Protocol (MCP) for seamless integration with AI assistants like Claude Code."
                    </p>

                    <h3 style=subsection_title_style>"Configure Claude Code"</h3>
                    <p style=paragraph_style>
                        "Add KDB as an MCP server in your Claude Code configuration:"
                    </p>
                    <pre style=code_block_style>
{"# ~/.config/claude-code/mcp.json
{
  \"mcpServers\": {
    \"kdb\": {
      \"command\": \"kdb\",
      \"args\": [\"--mcp\"],
      \"env\": {}
    }
  }
}"}
                    </pre>

                    <h3 style=subsection_title_style>"Available MCP Tools"</h3>
                    <ul style=list_style>
                        <li style=list_item_style><code>"debugger/attach"</code>" - Attach to process"</li>
                        <li style=list_item_style><code>"debugger/set_breakpoint"</code>" - Add breakpoint"</li>
                        <li style=list_item_style><code>"debugger/continue"</code>" - Resume execution"</li>
                        <li style=list_item_style><code>"debugger/step_forward"</code>" - Step forward"</li>
                        <li style=list_item_style><code>"debugger/step_backward"</code>" - Step backward (time-travel)"</li>
                        <li style=list_item_style><code>"debugger/get_stack_trace"</code>" - View stack"</li>
                        <li style=list_item_style><code>"debugger/get_registers"</code>" - Read registers"</li>
                    </ul>
                </section>

                // API Reference
                <section id="api" style=section_style>
                    <h2 style=section_title_style>"REST API Reference"</h2>

                    <p style=paragraph_style>
                        "Premium licenses include a REST API server for remote debugging (available at "
                        <a href="https://api.kindly.services" style="color: #FFD700;">"api.kindly.services"</a>
                        "):"
                    </p>

                    <h3 style=subsection_title_style>"Attach to Process"</h3>
                    <pre style=code_block_style>
{"POST /v1/debug/attach
Content-Type: application/json

{
  \"pid\": 12345
}

Response:
{
  \"success\": true,
  \"pid\": 12345,
  \"message\": \"Attached to process\"
}"}
                    </pre>

                    <h3 style=subsection_title_style>"Get Stack Trace"</h3>
                    <pre style=code_block_style>
{"GET /v1/debug/stack

Response:
{
  \"success\": true,
  \"frames\": [
    {
      \"address\": \"0x12345678\",
      \"function\": \"main\",
      \"file\": \"src/main.rs\",
      \"line\": 42
    }
  ],
  \"depth\": 5
}"}
                    </pre>

                    <h3 style=subsection_title_style>"Step Backward (Time-Travel)"</h3>
                    <pre style=code_block_style>
{"POST /v1/debug/step-back

Response:
{
  \"success\": true,
  \"snapshot_id\": 127,
  \"address\": \"0x12345670\"
}"}
                    </pre>
                </section>

                // FAQ
                <section id="faq" style=section_style>
                    <h2 style=section_title_style>"Frequently Asked Questions"</h2>

                    <h3 style=subsection_title_style>"What's the difference between open source and premium?"</h3>
                    <p style=paragraph_style>
                        "The open source version provides core time-travel debugging with a basic MCP handshake using standard dependencies. "
                        "Premium licenses add audit trails, remote debugging, compliance features, and pre-built binaries."
                    </p>

                    <h3 style=subsection_title_style>"How does time-travel debugging work?"</h3>
                    <p style=paragraph_style>
                        "KDB captures execution snapshots as you debug. Step backward replays from these snapshots, "
                        "giving you full bidirectional control over program execution."
                    </p>

                    <h3 style=subsection_title_style>"Is KDB compatible with GDB?"</h3>
                    <p style=paragraph_style>
                        "KDB uses familiar GDB-style commands (break, step, continue, bt) but adds time-travel capabilities "
                        "via the 'back' command. It's designed as a modern replacement with AI workflow integration."
                    </p>

                    <h3 style=subsection_title_style>"What platforms are supported?"</h3>
                    <p style=paragraph_style>
                        "Linux x86_64 is the primary platform. macOS and Windows support are planned for future releases."
                    </p>

                    <h3 style=subsection_title_style>"Can AI assistants use KDB?"</h3>
                    <p style=paragraph_style>
                        "Yes! KDB is the first debugger built with MCP protocol support. Claude Code and other MCP-compatible "
                        "AI assistants can debug alongside you with no configuration needed."
                    </p>

                    <h3 style=subsection_title_style>"How do I report bugs?"</h3>
                    <p style=paragraph_style>
                        "Open source users: GitHub Issues at "
                        <a href="https://github.com/kindly-software/kdb" style="color: #FFD700;">"github.com/kindly-software/kdb"</a>
                        <br/>
                        "Premium users: Email "
                        <a href="mailto:support@kindly.services" style="color: #FFD700;">"support@kindly.services"</a>
                        " for priority support."
                    </p>
                </section>

                // Support
                <section style=section_style>
                    <h2 style=section_title_style>"Support"</h2>
                    <p style=paragraph_style>
                        "Need help? We're here for you."
                    </p>
                    <ul style=list_style>
                        <li style=list_item_style>"Email: support@kindly.services"</li>
                        <li style=list_item_style>"GitHub: github.com/kindly-software/kdb"</li>
                        <li style=list_item_style>"API Documentation: api.kindly.services"</li>
                    </ul>
                </section>
            </div>
        </div>
    }
}
