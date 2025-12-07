//! Live Stats Component
//!
//! Real-time server statistics polled from /v1/debug/stats.

use leptos::prelude::*;
use kindly_ui::GlassmorphicCard;
use kindly_ui::theme::colors::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::spawn_local;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Stats {
    pub total_requests: u64,
    pub total_errors: u64,
    pub audit_entries: u64,
}

#[component]
pub fn LiveStats() -> impl IntoView {
    let (stats, set_stats) = signal(None::<Stats>);
    let (error, set_error) = signal(false);

    // Poll stats every 5 seconds
    Effect::new(move |_| {
        let fetch_stats = move || {
            spawn_local(async move {
                match gloo_net::http::Request::get("/v1/debug/stats").send().await {
                    Ok(resp) => {
                        if let Ok(data) = resp.json::<Stats>().await {
                            set_stats.set(Some(data));
                            set_error.set(false);
                        } else {
                            set_error.set(true);
                        }
                    }
                    Err(_) => {
                        set_error.set(true);
                    }
                }
            });
        };

        // Initial fetch
        fetch_stats();

        // Set up interval
        let callback = move || {
            let fetch = fetch_stats.clone();
            fetch();
        };
        let interval = gloo_timers::callback::Interval::new(5000, callback);
        std::mem::forget(interval); // Keep interval alive
    });

    let section_style = "
        padding: 5rem 2rem 4rem;
        position: relative;
        z-index: 1;
    ";

    let container_style = "
        max-width: 1200px;
        margin: 0 auto;
    ";

    let header_style = "
        text-align: center;
        margin-bottom: 2rem;
    ";

    let title_style = format!(
        "font-family: {};
         font-size: clamp(1.75rem, 3vw, 2rem);
         font-weight: 700;
         color: {};
         margin-bottom: 0.5rem;",
        FONT_HEADING,
        TEXT_PRIMARY
    );

    let grid_style = "
        display: grid;
        grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
        gap: 1.5rem;
    ";

    view! {
        <section id="stats" style=section_style>
            <div style=container_style>
                <div style=header_style>
                    <h2 style=title_style>"Live Server Statistics"</h2>
                </div>

                <div style=grid_style>
                    {move || match stats.get() {
                        Some(s) => view! {
                            <>
                                <GlassmorphicCard>
                                    <div style="text-align: center; padding: 1rem;">
                                        <div style="font-family: 'Space Grotesk', sans-serif; font-size: 3rem; font-weight: 700; color: #FFD700; margin-bottom: 0.5rem;">
                                            {s.total_requests}
                                        </div>
                                        <div style="font-size: 0.9375rem; color: rgba(255,255,255,0.7); font-weight: 500;">
                                            "Total Requests"
                                        </div>
                                    </div>
                                </GlassmorphicCard>

                                <GlassmorphicCard>
                                    <div style="text-align: center; padding: 1rem;">
                                        <div style="font-family: 'Space Grotesk', sans-serif; font-size: 3rem; font-weight: 700; color: #FFD700; margin-bottom: 0.5rem;">
                                            {s.total_errors}
                                        </div>
                                        <div style="font-size: 0.9375rem; color: rgba(255,255,255,0.7); font-weight: 500;">
                                            "Errors"
                                        </div>
                                    </div>
                                </GlassmorphicCard>

                                <GlassmorphicCard>
                                    <div style="text-align: center; padding: 1rem;">
                                        <div style="font-family: 'Space Grotesk', sans-serif; font-size: 3rem; font-weight: 700; color: #FFD700; margin-bottom: 0.5rem;">
                                            {s.audit_entries}
                                        </div>
                                        <div style="font-size: 0.9375rem; color: rgba(255,255,255,0.7); font-weight: 500;">
                                            "Audit Entries"
                                        </div>
                                    </div>
                                </GlassmorphicCard>

                                <GlassmorphicCard>
                                    <div style="text-align: center; padding: 1rem;">
                                        <div style="font-family: 'Space Grotesk', sans-serif; font-size: 3rem; font-weight: 700; color: #FFD700; margin-bottom: 0.5rem;">
                                            "●"
                                        </div>
                                        <div style="font-size: 0.9375rem; color: rgba(255,255,255,0.7); font-weight: 500;">
                                            "Status: Live"
                                        </div>
                                    </div>
                                </GlassmorphicCard>
                            </>
                        }.into_any(),
                        None if error.get() => view! {
                            <GlassmorphicCard>
                                <div style="text-align: center; padding: 1rem;">
                                    <div style="color: #FF6666; font-size: 2rem; margin-bottom: 0.5rem;">"⚠"</div>
                                    <div style="font-size: 0.9375rem; color: rgba(255,255,255,0.7); font-weight: 500;">"Loading stats..."</div>
                                </div>
                            </GlassmorphicCard>
                        }.into_any(),
                        None => view! {
                            <GlassmorphicCard>
                                <div style="text-align: center; padding: 1rem;">
                                    <div style="color: rgba(255,255,255,0.5); font-size: 2rem; margin-bottom: 0.5rem;">"⟳"</div>
                                    <div style="font-size: 0.9375rem; color: rgba(255,255,255,0.7); font-weight: 500;">"Loading stats..."</div>
                                </div>
                            </GlassmorphicCard>
                        }.into_any(),
                    }}
                </div>
            </div>
        </section>
    }
}
