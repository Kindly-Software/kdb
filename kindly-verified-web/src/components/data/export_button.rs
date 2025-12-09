//! ExportButton - Export results component (T4+T0)
//!
//! Leptos wrapper for ExportResultsCapsule with PDF/JSON/CSV export
//! and Q34 audit trail support.

use leptos::prelude::*;
use leptos::task::spawn_local;
use std::sync::Arc;

use crate::capsules::{ExportResultsCapsule, ExportFormat};
use crate::utils::styles::*;
use leptos::prelude::Show;

/// ExportButton - Export detection results to multiple formats
///
/// # Props
///
/// - `detection_results` - Results to export
/// - `filename` - Base filename for export (without extension)
/// - `on_export_complete` - Callback when export finishes
///
/// # Example
///
/// ```rust,ignore
/// use leptos::prelude::*;
/// use crate::components::data::ExportButton;
///
/// #[component]
/// pub fn Example() -> impl IntoView {
///     let results = vec![/* detection results */];
///
///     let handle_complete = move |format: String| {
///         log::info!("Exported to {}", format);
///     };
///
///     view! {
///         <ExportButton
///             detection_results=results
///             filename="detection_results"
///             on_export_complete=Callback::new(move |f| handle_complete(f))
///         />
///     }
/// }
/// ```
#[component]
#[allow(dead_code)]
pub fn ExportButton(
    #[prop(optional)] detection_results: Option<Vec<String>>,
    #[prop(optional)] filename: Option<String>,
    #[prop(optional)] on_export_complete: Option<Callback<String>>,
) -> impl IntoView {
    // Create capsule instance
    let capsule = Arc::new(ExportResultsCapsule::new(ExportFormat::PDF));

    // Create reactive signals for export state
    let (exporting, set_exporting) = signal(false);
    let (export_format, set_export_format) = signal(ExportFormat::PDF);
    let (error_message, set_error_message) = signal(None);

    let results = detection_results.unwrap_or_default();
    let file_basename = filename.unwrap_or_else(|| "detection_results".to_string());

    // Clone results for use in both closure and view
    let results_for_view = results.clone();

    // Handle export
    let handle_export = move |format: ExportFormat| {
        if results.is_empty() {
            set_error_message.set(Some("No results to export".to_string()));
            return;
        }

        set_exporting.set(true);
        set_error_message.set(None);

        let capsule_clone = capsule.clone();
        let filename_clone = file_basename.clone();
        let results_clone = results.clone();

        spawn_local(async move {
            match capsule_clone.export_results(
                &results_clone,
                &filename_clone,
                format,
            ).await {
                Ok(export_bytes) => {
                    let export_msg = format!("Exported {} bytes", export_bytes.len());
                    log::info!("{}", export_msg);
                    if let Some(callback) = on_export_complete {
                        callback.run(export_msg.clone());
                    }
                    set_error_message.set(None);
                }
                Err(e) => {
                    let error_msg = format!("Export failed: {:?}", e);
                    log::error!("{}", error_msg);
                    set_error_message.set(Some(error_msg));
                }
            }
            set_exporting.set(false);
        });
    };

    // Format selection buttons
    let button_class = move |selected: bool| {
        if selected {
            format!(
                "padding: {} {};
                 border-radius: 8px;
                 background: linear-gradient(135deg, #663399, #FFD700);
                 color: white;
                 border: 1px solid #FFD700;
                 cursor: pointer;
                 font-weight: 600;
                 transition: all 0.2s ease;
                 flex: 1;",
                SPACING_SM, SPACING_MD
            )
        } else {
            format!(
                "padding: {} {};
                 border-radius: 8px;
                 background: rgba(102, 51, 153, 0.2);
                 color: #FFD700;
                 border: 1px solid rgba(255, 215, 0, 0.4);
                 cursor: pointer;
                 font-weight: 600;
                 transition: all 0.2s ease;
                 flex: 1;
                 :hover {{
                    background: rgba(102, 51, 153, 0.3);
                    border-color: rgba(255, 215, 0, 0.6);
                 }}",
                SPACING_SM, SPACING_MD
            )
        }
    };

    let container_style = format!(
        "{}
         border-radius: 12px;
         padding: {};
         display: flex;
         flex-direction: column;
         gap: {};",
        glassmorphism(GlassBlur::Medium, 0.15),
        SPACING_LG,
        SPACING_MD
    );

    let format_selector_style = "
        display: flex;
        gap: 0.5rem;
        margin-bottom: 1rem;
    ";

    let export_button_style = format!(
        "padding: {} {};
         border-radius: 8px;
         background: linear-gradient(135deg, #FFD700, #DAA520);
         color: #1a0033;
         border: none;
         cursor: {};
         font-weight: 700;
         font-size: 1rem;
         transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
         opacity: {};",
        SPACING_MD, SPACING_LG,
        if exporting.get() { "not-allowed" } else { "pointer" },
        if exporting.get() { "0.6" } else { "1.0" }
    );

    let (error_style, _) = signal(format!(
        "{}
         padding: {};
         border-radius: 8px;
         background: rgba(239, 68, 68, 0.2);
         border: 1px solid rgba(239, 68, 68, 0.4);
         color: #EF4444;",
        text_body(),
        SPACING_MD
    ));

    view! {
        <div style=container_style>
            <div style=text_heading_md()>
                "Export Results"
            </div>

            <div style=format_selector_style>
                <button
                    style=button_class(export_format.get() == ExportFormat::PDF)
                    on:click=move |_| set_export_format.set(ExportFormat::PDF)
                    disabled=exporting.get()
                >
                    "PDF"
                </button>
                <button
                    style=button_class(export_format.get() == ExportFormat::JSON)
                    on:click=move |_| set_export_format.set(ExportFormat::JSON)
                    disabled=exporting.get()
                >
                    "JSON"
                </button>
                <button
                    style=button_class(export_format.get() == ExportFormat::CSV)
                    on:click=move |_| set_export_format.set(ExportFormat::CSV)
                    disabled=exporting.get()
                >
                    "CSV"
                </button>
            </div>

            <button
                style=export_button_style
                on:click=move |_| handle_export(export_format.get())
                disabled=exporting.get() || results_for_view.is_empty()
            >
                {move || {
                    if exporting.get() {
                        "Exporting...".to_string()
                    } else {
                        format!("Export as {}", match export_format.get() {
                            ExportFormat::PDF => "PDF",
                            ExportFormat::JSON => "JSON",
                            ExportFormat::CSV => "CSV",
                        })
                    }
                }}
            </button>

            <Show when=move || error_message.get().is_some()>
                {move || {
                    let error = error_message.get().unwrap();
                    view! {
                        <div style=move || error_style.get()>
                            {error}
                        </div>
                    }
                }}
            </Show>

            <div style=text_caption()>
                "All exports include Q34 audit trails for compliance verification"
            </div>
        </div>
    }
}
