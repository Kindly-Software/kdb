//! BatchUpload - Batch image upload component (T4+T5)
//!
//! Leptos wrapper for BatchUploadCapsule with parallel upload
//! and lockfree work-stealing queue.

use leptos::prelude::*;
use leptos::task::spawn_local;
use std::sync::Arc;
use wasm_bindgen::JsCast;

use crate::capsules::{BatchUploadCapsule, BatchStats};
use crate::utils::styles::*;
use leptos::prelude::Show;

/// BatchUpload - Parallel batch image upload with progress tracking
///
/// # Props
///
/// - `max_files` - Maximum files per batch (default: 100)
/// - `on_upload_complete` - Callback when all files uploaded
/// - `on_progress` - Callback for progress updates
///
/// # Example
///
/// ```rust,ignore
/// use leptos::prelude::*;
/// use crate::components::upload::BatchUpload;
///
/// #[component]
/// pub fn Example() -> impl IntoView {
///     let handle_complete = move |stats: BatchStats| {
///         log::info!("Uploaded {} files", stats.total_uploaded);
///     };
///
///     view! {
///         <BatchUpload
///             max_files=Some(50)
///             on_upload_complete=Callback::new(move |s| handle_complete(s))
///         />
///     }
/// }
/// ```
#[component]
#[allow(dead_code)]
pub fn BatchUpload(
    #[prop(optional)] max_files: Option<usize>,
    #[prop(optional)] on_upload_complete: Option<Callback<BatchStats>>,
    #[prop(optional)] on_progress: Option<Callback<(usize, usize)>>,
) -> impl IntoView {
    // Create capsule instance
    let capsule = Arc::new(BatchUploadCapsule::new(max_files.unwrap_or(100)));

    // Create reactive signals for upload state
    let (uploading, set_uploading) = signal(false);
    let (uploaded_count, set_uploaded_count) = signal(0usize);
    let (total_count, set_total_count) = signal(0usize);
    let (current_stats, set_current_stats) = signal(None);
    let (error_message, set_error_message) = signal(None);

    // Handle file input
    let handle_files_selected = move |event: leptos::ev::Event| {
        let input: web_sys::HtmlInputElement = event
            .target()
            .and_then(|t| t.dyn_into().ok())
            .expect("Failed to get input element");

        if let Some(files) = input.files() {
            let file_count = files.length() as usize;
            set_total_count.set(file_count);
            set_uploaded_count.set(0);
            set_uploading.set(true);
            set_error_message.set(None);

            let capsule_clone = capsule.clone();

            spawn_local(async move {
                for i in 0..file_count {
                    if let Some(file) = files.get(i as u32) {
                        match upload_file(&file).await {
                            Ok(data) => {
                                let _ = capsule_clone.add_file(data);
                                set_uploaded_count.update(|c| *c += 1);

                                if let Some(callback) = on_progress {
                                    callback.run((i + 1, file_count));
                                }
                            }
                            Err(e) => {
                                let error_msg = format!("Upload failed for file {}: {}", i + 1, e);
                                log::error!("{}", error_msg);
                                set_error_message.set(Some(error_msg));
                                break;
                            }
                        }
                    }
                }

                // Get final batch stats
                let stats = capsule_clone.get_stats();
                set_current_stats.set(Some(stats.clone()));
                if let Some(callback) = on_upload_complete {
                    callback.run(stats);
                }

                set_uploading.set(false);
            });
        }
    };

    // Container styles
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

    let header_style = format!(
        "{}
         padding-bottom: {};
         border-bottom: 1px solid rgba(255, 215, 0, 0.2);",
        text_heading_md(),
        SPACING_MD
    );

    let upload_area_style = format!(
        "padding: {};
         border: 2px dashed rgba(255, 215, 0, 0.4);
         border-radius: 8px;
         text-align: center;
         cursor: pointer;
         transition: all 0.2s ease;
         :hover {{
            border-color: rgba(255, 215, 0, 0.6);
            background: rgba(102, 51, 153, 0.15);
         }}",
        SPACING_2XL
    );

    let file_input_style = "
        display: none;
    ";

    let (progress_container_style, _) = signal(format!(
        "display: flex;
         flex-direction: column;
         gap: {};",
        SPACING_SM
    ));

    let progress_bar_style = "
        width: 100%;
        height: 8px;
        background: rgba(255, 215, 0, 0.2);
        border-radius: 4px;
        overflow: hidden;
    ";

    // Create NodeRef for file input
    let file_input_ref = NodeRef::<leptos::html::Input>::new();

    let progress_fill_style = move || {
        let percentage = if total_count.get() > 0 {
            (uploaded_count.get() as f32 / total_count.get() as f32) * 100.0
        } else {
            0.0
        };

        format!(
            "height: 100%;
             width: {}%;
             background: linear-gradient(90deg, #FFD700, #663399);
             transition: width 0.3s ease;",
            percentage
        )
    };

    let (progress_label_style, _) = signal(format!(
        "{}
         display: flex;
         justify-content: space-between;",
        text_caption()
    ));

    let stats_grid_style = "
        display: grid;
        grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
        gap: 1rem;
    ";

    let (stat_box_style, _) = signal(format!(
        "padding: {};
         border-radius: 8px;
         background: rgba(102, 51, 153, 0.2);
         border: 1px solid rgba(255, 215, 0, 0.2);
         text-align: center;",
        SPACING_MD
    ));

    let (error_style, _) = signal(format!(
        "padding: {};
         border-radius: 8px;
         background: rgba(239, 68, 68, 0.2);
         border: 1px solid rgba(239, 68, 68, 0.4);
         color: #EF4444;",
        SPACING_MD
    ));

    view! {
        <div style=container_style>
            <div style=header_style>
                "Batch Upload"
            </div>

            <input
                type="file"
                multiple=true
                style=file_input_style
                on:change=handle_files_selected
                node_ref=file_input_ref
            />

            <div
                style=upload_area_style
                on:click=move |_| {
                    // Trigger file input click
                    if let Some(input) = file_input_ref.get() {
                        input.click();
                    }
                }
            >
                {if uploading.get() {
                    "Uploading..."
                } else {
                    "Click to select files or drag & drop"
                }}
            </div>

            <Show when={move || total_count.get() > 0}>
                <div style=move || progress_container_style.get()>
                    <div style=move || progress_label_style.get()>
                        <span>"Upload Progress"</span>
                        <span>
                            {move || {
                                format!("{}/{}", uploaded_count.get(), total_count.get())
                            }}
                        </span>
                    </div>
                    <div style=progress_bar_style>
                        <div style=progress_fill_style></div>
                    </div>
                </div>
            </Show>

            <Show when=move || current_stats.get().is_some()>
                {move || {
                    let stats = current_stats.get().unwrap();
                    view! {
                        <div style=stats_grid_style>
                            <div style=move || stat_box_style.get()>
                                <div style=text_caption()>"Uploaded"</div>
                                <div style=text_heading_sm()>
                                    {stats.total_uploaded}
                                </div>
                            </div>
                            <div style=move || stat_box_style.get()>
                                <div style=text_caption()>"Failed"</div>
                                <div style=text_heading_sm()>
                                    {stats.total_failed}
                                </div>
                            </div>
                            <div style=move || stat_box_style.get()>
                                <div style=text_caption()>"Total Size"</div>
                                <div style=text_caption()>
                                    {format!("{:.1} MB", stats.total_bytes_uploaded as f32 / 1_000_000.0)}
                                </div>
                            </div>
                        </div>
                    }
                }}
            </Show>

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
        </div>
    }
}

/// Upload a single file (placeholder - returns file size for now)
async fn upload_file(file: &web_sys::File) -> Result<Vec<u8>, String> {
    // For now, just log the file and return placeholder
    log::info!("Uploading file: {}, size: {} bytes", file.name(), file.size());

    // In production, would properly read file using FileReader API
    // For now return placeholder
    Ok(vec![0u8; file.size() as usize])
}
