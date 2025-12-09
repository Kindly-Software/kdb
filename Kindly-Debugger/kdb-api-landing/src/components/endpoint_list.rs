//! Endpoint List Component
//!
//! Displays all 10 KDB API endpoints with interactive examples.

use leptos::prelude::*;
use kindly_ui::{GlassmorphicCard, CodeBlock};
use kindly_ui::theme::colors::*;

struct Endpoint {
    method: &'static str,
    path: &'static str,
    description: &'static str,
    request_body: Option<&'static str>,
    response_example: &'static str,
}

const ENDPOINTS: [Endpoint; 11] = [
    Endpoint {
        method: "GET",
        path: "/v1/debug/stats",
        description: "Get server statistics and health metrics",
        request_body: None,
        response_example: r#"{
  "total_requests": 1234,
  "total_errors": 5,
  "audit_entries": 890
}"#,
    },
    Endpoint {
        method: "POST",
        path: "/v1/debug/attach",
        description: "Attach to a running process by PID",
        request_body: Some(r#"{"pid": 12345}"#),
        response_example: r#"{
  "success": true,
  "pid": 12345,
  "message": "Attached to process"
}"#,
    },
    Endpoint {
        method: "DELETE",
        path: "/v1/debug/detach",
        description: "Detach from the current debugging session",
        request_body: None,
        response_example: r#"{
  "success": true,
  "pid": 12345,
  "message": "Detached from process"
}"#,
    },
    Endpoint {
        method: "POST",
        path: "/v1/debug/breakpoint",
        description: "Set a breakpoint at a memory address",
        request_body: Some(r#"{"address": "0x401000"}"#),
        response_example: r#"{
  "success": true,
  "breakpoint_id": 0,
  "address": "0x00000000401000"
}"#,
    },
    Endpoint {
        method: "POST",
        path: "/v1/debug/continue",
        description: "Continue execution until next breakpoint",
        request_body: None,
        response_example: r#"{
  "success": true,
  "message": "Continued execution"
}"#,
    },
    Endpoint {
        method: "POST",
        path: "/v1/debug/snapshot",
        description: "Capture time-travel snapshot of current state",
        request_body: None,
        response_example: r#"{
  "success": true,
  "snapshot_id": 42,
  "rip": "0x00000000401234"
}"#,
    },
    Endpoint {
        method: "POST",
        path: "/v1/debug/step-back",
        description: "Step backward in time (time-travel replay)",
        request_body: None,
        response_example: r#"{
  "success": true,
  "snapshot_id": 41,
  "rip": "0x00000000401230"
}"#,
    },
    Endpoint {
        method: "POST",
        path: "/v1/debug/step-forward",
        description: "Step forward one instruction",
        request_body: None,
        response_example: r#"{
  "success": true,
  "rip": "0x00000000401238"
}"#,
    },
    Endpoint {
        method: "GET",
        path: "/v1/debug/stack",
        description: "Get stack trace with frame addresses",
        request_body: None,
        response_example: r#"{
  "success": true,
  "frames": ["0x401234", "0x401100", "0x400890"],
  "depth": 3
}"#,
    },
    Endpoint {
        method: "GET",
        path: "/v1/debug/registers",
        description: "Read CPU register values",
        request_body: None,
        response_example: r#"{
  "success": true,
  "registers": {
    "rip": "0x401234",
    "rsp": "0x7fff12340000",
    "rbp": "0x7fff12340010"
  }
}"#,
    },
    Endpoint {
        method: "POST",
        path: "/v1/debug/audit-verify",
        description: "Verify audit trail hash-chain integrity",
        request_body: None,
        response_example: r#"{
  "success": true,
  "verified": true,
  "entries": 890,
  "root_hash": "0x9e3779b97f4a7c15"
}"#,
    },
];

#[component]
pub fn EndpointList() -> impl IntoView {
    let section_style = "
        padding: 4rem 2rem;
        position: relative;
        z-index: 1;
    ";

    let container_style = "
        max-width: 1200px;
        margin: 0 auto;
    ";

    let header_style = "
        text-align: center;
        margin-bottom: 3rem;
    ";

    let title_style = format!(
        "font-family: {};
         font-size: clamp(2rem, 4vw, 2.5rem);
         font-weight: 700;
         color: {};
         margin-bottom: 0.5rem;",
        FONT_HEADING,
        TEXT_PRIMARY
    );

    let subtitle_style = format!(
        "font-size: 1.125rem;
         color: {};",
        TEXT_SECONDARY
    );

    let grid_style = "
        display: grid;
        grid-template-columns: repeat(auto-fit, minmax(350px, 1fr));
        gap: 1.5rem;
    ";

    view! {
        <section id="endpoints" style=section_style>
            <div style=container_style>
                <div style=header_style>
                    <h2 style=title_style>"API Endpoints"</h2>
                    <p style=subtitle_style>"11 REST endpoints for time-travel debugging"</p>
                </div>

                <div style=grid_style>
                    {ENDPOINTS.iter().map(|endpoint| {
                        view! { <EndpointCard endpoint=endpoint /> }
                    }).collect::<Vec<_>>()}
                </div>
            </div>
        </section>
    }
}

#[component]
fn EndpointCard(endpoint: &'static Endpoint) -> impl IntoView {
    let (expanded, set_expanded) = signal(false);

    let method_badge_style = match endpoint.method {
        "GET" => "background: rgba(0, 150, 255, 0.2); color: #00BFFF; border: 1px solid rgba(0, 150, 255, 0.4);",
        "POST" => "background: rgba(76, 175, 80, 0.2); color: #66FF66; border: 1px solid rgba(76, 175, 80, 0.4);",
        "DELETE" => "background: rgba(244, 67, 54, 0.2); color: #FF6666; border: 1px solid rgba(244, 67, 54, 0.4);",
        _ => "background: rgba(255, 255, 255, 0.1); color: #fff; border: 1px solid rgba(255, 255, 255, 0.2);",
    };

    let method_badge_full_style = format!(
        "{} padding: 0.25rem 0.75rem; border-radius: 6px; font-weight: 600; font-size: 0.75rem;",
        method_badge_style
    );

    let path_style = format!(
        "font-family: {};
         color: {};
         font-size: 1rem;
         margin: 0.75rem 0;",
        FONT_CODE,
        GOLD_PRIMARY
    );

    let description_style = format!(
        "color: {};
         font-size: 0.9375rem;
         line-height: 1.6;
         margin-bottom: 1rem;",
        TEXT_SECONDARY
    );

    let toggle_button_style = format!(
        "background: rgba(255, 215, 0, 0.1);
         border: 1px solid rgba(255, 215, 0, 0.3);
         color: {};
         padding: 0.5rem 1rem;
         border-radius: 8px;
         cursor: pointer;
         font-size: 0.875rem;
         font-weight: 500;
         transition: all 0.2s ease;
         width: 100%;
         margin-top: 0.5rem;",
        GOLD_PRIMARY
    );

    let curl_command = if let Some(body) = endpoint.request_body {
        format!(
            "curl -X {} https://api.kindly.software{} \\\n  -H 'Content-Type: application/json' \\\n  -d '{}'",
            endpoint.method, endpoint.path, body
        )
    } else {
        format!(
            "curl -X {} https://api.kindly.software{}",
            endpoint.method, endpoint.path
        )
    };

    view! {
        <GlassmorphicCard hoverable=true>
            <div style="display: flex; align-items: center; gap: 0.75rem; margin-bottom: 0.5rem;">
                <span style=method_badge_full_style>{endpoint.method}</span>
            </div>
            <div style=path_style>{endpoint.path}</div>
            <p style=description_style>{endpoint.description}</p>

            <button
                style=toggle_button_style
                class="endpoint-toggle"
                on:click=move |_| set_expanded.update(|v| *v = !*v)
            >
                {move || if expanded.get() { "▼ Hide Example" } else { "▶ Show Example" }}
            </button>

            {move || expanded.get().then(|| view! {
                <div style="margin-top: 1rem;">
                    <div style="margin-bottom: 0.75rem; color: rgba(255,255,255,0.8); font-size: 0.875rem; font-weight: 600;">
                        "Request:"
                    </div>
                    <CodeBlock code=curl_command.clone() language="bash".to_string() />

                    <div style="margin-bottom: 0.75rem; margin-top: 1.5rem; color: rgba(255,255,255,0.8); font-size: 0.875rem; font-weight: 600;">
                        "Response:"
                    </div>
                    <CodeBlock code=endpoint.response_example.to_string() language="json".to_string() />
                </div>
            })}
        </GlassmorphicCard>

        <style>
            ".endpoint-toggle:hover {
                background: rgba(255, 215, 0, 0.2) !important;
                border-color: rgba(255, 215, 0, 0.5) !important;
            }"
        </style>
    }
}
