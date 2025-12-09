//! DetectionHistory - Persistent detection storage component (T9+T1)
//!
//! Leptos wrapper for DetectionHistoryCapsule with IndexedDB persistence
//! and Q34 audit trail support.

use leptos::prelude::*;
use leptos::task::spawn_local;
use std::sync::Arc;

use crate::capsules::{DetectionHistoryCapsule, DetectionEntry};
use crate::utils::styles::*;
use leptos::prelude::Show;

/// DetectionHistory - Persistent detection history browser
///
/// # Props
///
/// - `on_entry_selected` - Callback when user selects a detection entry
/// - `show_comparisons` - Whether to show duplicate comparison view
///
/// # Example
///
/// ```rust,ignore
/// use leptos::prelude::*;
/// use crate::components::data::DetectionHistory;
///
/// #[component]
/// pub fn Example() -> impl IntoView {
///     let handle_select = move |entry: DetectionEntry| {
///         log::info!("Selected: {:?}", entry);
///     };
///
///     view! {
///         <DetectionHistory
///             on_entry_selected=Callback::new(move |e| handle_select(e))
///             show_comparisons=true
///         />
///     }
/// }
/// ```
#[component]
#[allow(dead_code)]
pub fn DetectionHistory(
    #[prop(optional)] on_entry_selected: Option<Callback<DetectionEntry>>,
    #[prop(optional)] show_comparisons: Option<bool>,
) -> impl IntoView {
    // Create capsule instance with IndexedDB persistence
    let capsule = Arc::new(
        DetectionHistoryCapsule::new()
            .expect("Failed to initialize detection history capsule")
    );

    // Create reactive signals for history state
    let (entries, set_entries) = signal(Vec::<DetectionEntry>::new());
    let (selected_entry, set_selected_entry) = signal(None);
    let (loading, set_loading) = signal(false);

    // Clone capsule for use in effect
    let capsule_for_effect = capsule.clone();

    // Load history from IndexedDB on mount
    Effect::new(move |_| {
        set_loading.set(true);
        let capsule_clone = capsule_for_effect.clone();

        // Spawn async load task
        spawn_local(async move {
            match capsule_clone.load_all_entries().await {
                Ok(loaded_entries) => {
                    set_entries.set(loaded_entries);
                }
                Err(e) => {
                    log::error!("Failed to load detection history: {:?}", e);
                }
            }
            set_loading.set(false);
        });
    });

    // Handle entry selection
    let handle_entry_click = move |index: usize| {
        if let Some(entry) = entries.get().get(index).cloned() {
            set_selected_entry.set(Some(index));
            if let Some(callback) = on_entry_selected {
                callback.run(entry);
            }
        }
    };

    // Get comparisons for selected entry
    let capsule_for_memo = capsule.clone();
    let comparisons = Memo::new(move |_| {
        if let Some(idx) = selected_entry.get() {
            entries.get().get(idx).and_then(|e| {
                capsule_for_memo.get_comparisons(&e.id).ok()
            })
        } else {
            None
        }
    });

    // Container styles
    let container_style = format!(
        "{}
         border-radius: 12px;
         padding: {};
         min-height: 400px;
         display: flex;
         flex-direction: column;
         gap: {};",
        glassmorphism(GlassBlur::Medium, 0.15),
        SPACING_LG,
        SPACING_MD
    );

    let header_style = format!(
        "{}
         padding: {};
         border-bottom: 1px solid rgba(255, 215, 0, 0.2);
         margin-bottom: {};",
        text_heading_lg(),
        SPACING_MD,
        SPACING_MD
    );

    let list_style = "
        flex: 1;
        overflow-y: auto;
        display: flex;
        flex-direction: column;
        gap: 0.5rem;
    ";

    let entry_style = move |is_selected: bool| {
        let base = format!(
            "padding: {};
             border-radius: 8px;
             cursor: pointer;
             transition: all 0.2s ease;
             border: 1px solid rgba(255, 215, 0, {});
             background: rgba(102, 51, 153, {});",
            SPACING_MD,
            if is_selected { "1" } else { "0.2" },
            if is_selected { "0.3" } else { "0.1" }
        );

        if is_selected {
            format!(
                "{}
                 box-shadow: 0 0 20px rgba(255, 215, 0, 0.3);",
                base
            )
        } else {
            format!(
                "{}
                 :hover {{
                    background: rgba(102, 51, 153, 0.2);
                    border-color: rgba(255, 215, 0, 0.4);
                 }}",
                base
            )
        }
    };

    let (entry_timestamp_style, _) = signal(format!(
        "{}
         color: rgba(255, 255, 255, 0.6);
         margin-bottom: 0.5rem;",
        text_caption()
    ));

    let entry_confidence_style = "
        color: #FFD700;
        font-weight: 600;
    ";

    let (comparison_style, _) = signal(format!(
        "{}
         padding: {};
         border-radius: 8px;
         margin-top: {};
         max-height: 200px;
         overflow-y: auto;",
        glassmorphism(GlassBlur::Light, 0.1),
        SPACING_MD,
        SPACING_MD
    ));

    let (loading_style, _) = signal(format!(
        "{}
         text-align: center;
         padding: {};",
        text_body(),
        SPACING_2XL
    ));

    view! {
        <div style=container_style>
            <div style=header_style>
                "Detection History"
            </div>

            <Show
                when=move || loading.get()
                fallback=move || {
                    view! {
                        <Show
                            when=move || entries.get().is_empty()
                            fallback=move || {
                                view! {
                                    <div style=list_style>
                                        <For
                                            each=move || entries.get().into_iter().enumerate()
                                            key=|(idx, _)| *idx
                                            children=move |(idx, entry)| {
                                                let is_selected = selected_entry.get() == Some(idx);
                                                let handle_click = move |_| handle_entry_click(idx);
                                                view! {
                                                    <div
                                                        style=entry_style(is_selected)
                                                        on:click=handle_click
                                                    >
                                                        <div style=move || entry_timestamp_style.get()>
                                                            {entry.timestamp}
                                                        </div>
                                                        <div style=entry_confidence_style>
                                                            {format!("Confidence: {:.1}%", entry.max_confidence * 100.0)}
                                                        </div>
                                                    </div>
                                                }
                                            }
                                        />
                                    </div>
                                }
                            }
                        >
                            <div style=move || loading_style.get()>
                                "No detections yet. Upload an image to get started."
                            </div>
                        </Show>
                    }
                }
            >
                <div style=move || loading_style.get()>
                    "Loading history..."
                </div>
            </Show>

            <Show
                when=move || show_comparisons.unwrap_or(true) && comparisons.get().is_some()
            >
                {move || {
                    view! {
                        <Show when=move || comparisons.get().is_some()>
                            {move || {
                                let comps = comparisons.get().unwrap();
                                let top_comps: Vec<_> = comps.iter().take(3).cloned().collect();
                                view! {
                                    <div style=move || comparison_style.get()>
                                <div style=text_caption()>
                                    {format!("Found {} similar detections", comps.len())}
                                </div>
                                <div style="margin-top: 0.5rem;">
                                    <For
                                        each=move || top_comps.clone().into_iter().enumerate()
                                        key=|(i, _)| *i
                                        children=move |(_, comp)| {
                                            view! {
                                                <div style="font-size: 0.875rem; color: rgba(255, 255, 255, 0.7); padding: 0.25rem 0;">
                                                    {format!(
                                                        "Similarity: {:.1}%",
                                                        comp.similarity_score * 100.0
                                                    )}
                                                </div>
                                            }
                                        }
                                    />
                                </div>
                            </div>
                        }
                            }}
                        </Show>
                    }
                }}
            </Show>
        </div>
    }
}
