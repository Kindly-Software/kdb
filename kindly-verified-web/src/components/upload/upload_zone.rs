use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{DragEvent, Event, FileReader};
use crate::utils::*;

/// Drag-and-drop upload zone component
///
/// Features:
/// - HTML5 drag-and-drop API integration
/// - Click to browse file picker
/// - Visual feedback on drag hover
/// - Supported formats: JPEG, PNG, BMP, GIF, WebP, TIFF, AVIF, HEIC
#[component]
pub fn UploadZone(
    #[prop(into)] on_file_selected: Callback<(Vec<u8>, String)>,
) -> impl IntoView
{
    let (is_dragging, set_is_dragging) = signal(false);
    let (error_message, set_error_message) = signal(None::<String>);
    let file_input_ref = NodeRef::<leptos::html::Input>::new();

    // Handle file selection (from drag or browse)
    let handle_file = {
        move |file: web_sys::File| {
            let file_name = file.name();
            let file_size = file.size();

            // Validate file size (max 50MB)
            if file_size > 50_000_000.0 {
                set_error_message.set(Some("File too large. Maximum size is 50MB.".to_string()));
                return;
            }

            // Validate file type
            let file_type = file.type_();
            if !is_supported_format(&file_type) {
                set_error_message.set(Some(format!(
                    "Unsupported format: {}. Supported: JPEG, PNG, BMP, GIF, WebP, TIFF, AVIF, HEIC",
                    file_type
                )));
                return;
            }

            set_error_message.set(None);

            // Read file as array buffer
            let reader = FileReader::new().expect("Failed to create FileReader");
            let reader_clone = reader.clone();

            let closure = Closure::wrap(Box::new(move |_event: Event| {
                if let Ok(result) = reader_clone.result() {
                    if let Ok(array_buffer) = result.dyn_into::<js_sys::ArrayBuffer>() {
                        let uint8_array = js_sys::Uint8Array::new(&array_buffer);
                        let data = uint8_array.to_vec();

                        log::info!("File loaded: {} ({} bytes)", file_name, data.len());
                        on_file_selected.run((data, file_name.clone()));
                    }
                }
            }) as Box<dyn FnMut(_)>);

            reader
                .add_event_listener_with_callback("load", closure.as_ref().unchecked_ref())
                .expect("Failed to add event listener");
            closure.forget();

            reader
                .read_as_array_buffer(&file)
                .expect("Failed to read file");
        }
    };

    // Handle drag enter
    let on_drag_enter = move |e: DragEvent| {
        e.prevent_default();
        set_is_dragging.set(true);
    };

    // Handle drag over
    let on_drag_over = move |e: DragEvent| {
        e.prevent_default();
    };

    // Handle drag leave
    let on_drag_leave = move |e: DragEvent| {
        e.prevent_default();
        set_is_dragging.set(false);
    };

    // Handle drop
    let on_drop = move |e: DragEvent| {
        e.prevent_default();
        set_is_dragging.set(false);

        if let Some(data_transfer) = e.data_transfer() {
            if let Some(files) = data_transfer.files() {
                if let Some(file) = files.get(0) {
                    handle_file(file);
                }
            }
        }
    };

    // Handle file input change (browse button)
    let on_file_input_change = move |_e: Event| {
        if let Some(input) = file_input_ref.get() {
            if let Some(files) = input.files() {
                if let Some(file) = files.get(0) {
                    handle_file(file);
                }
            }
        }
    };

    // Trigger file input click
    let trigger_file_input = move |_| {
        if let Some(input) = file_input_ref.get() {
            input.click();
        }
    };

    view! {
        <div>
            // Hidden file input
            <input
                type="file"
                node_ref=file_input_ref
                accept="image/jpeg,image/png,image/bmp,image/gif,image/webp,image/tiff,image/avif,image/heic"
                style="display: none;"
                on:change=on_file_input_change
            />

            // Drop zone
            <div
                on:dragenter=on_drag_enter
                on:dragover=on_drag_over
                on:dragleave=on_drag_leave
                on:drop=on_drop
                on:click=trigger_file_input
                style=move || format!(
                    "{}
                     border-radius: 24px;
                     padding: {} {};
                     text-align: center;
                     cursor: pointer;
                     transition: all 0.3s ease;
                     {}
                     {}",
                    if is_dragging.get() {
                        glassmorphism(GlassBlur::Heavy, 0.3)
                    } else {
                        glassmorphism(GlassBlur::Medium, 0.15)
                    },
                    SPACING_4XL,
                    SPACING_2XL,
                    if is_dragging.get() {
                        format!("border: 3px dashed {}; {}", COLOR_GOLD, glow_gold())
                    } else {
                        format!("border: 2px dashed rgba(255, 215, 0, 0.4);")
                    },
                    if is_dragging.get() {
                        "transform: scale(1.02);"
                    } else {
                        ""
                    }
                )
            >
                // Upload icon
                <div style=format!(
                    "font-size: 5rem;
                     margin-bottom: {};
                     opacity: {};",
                    SPACING_LG,
                    if is_dragging.get() { "1" } else { "0.7" }
                )>
                    "📸"
                </div>

                // Upload text
                <h3 style=format!(
                    "{}
                     margin-bottom: {};",
                    text_heading_md(),
                    SPACING_MD
                )>
                    {move || if is_dragging.get() {
                        "Drop image here"
                    } else {
                        "Drag & Drop Image"
                    }}
                </h3>

                <p style=text_body()>
                    "or click to browse"
                </p>

                // Supported formats
                <p style=format!(
                    "{}
                     margin-top: {};
                     color: rgba(255, 215, 0, 0.7);",
                    text_caption(),
                    SPACING_LG
                )>
                    "Supports: JPEG, PNG, BMP, GIF, WebP, TIFF, AVIF, HEIC"
                </p>

                // Error message
                {move || error_message.get().map(|msg| {
                    view! {
                        <div style=format!(
                            "margin-top: {};
                             padding: {} {};
                             background: rgba(239, 68, 68, 0.2);
                             border: 1px solid {};
                             border-radius: 8px;
                             color: white;",
                            SPACING_LG,
                            SPACING_MD,
                            SPACING_LG,
                            COLOR_ERROR
                        )>
                            <p style="font-weight: 600;">
                                "⚠️ " {msg}
                            </p>
                        </div>
                    }
                })}
            </div>

            // Instructions
            <div style=format!(
                "margin-top: {};
                 text-align: center;",
                SPACING_LG
            )>
                <p style=text_caption()>
                    "Your image is processed locally in your browser. No data leaves your device."
                </p>
            </div>
        </div>
    }
}

/// Check if file type is supported
fn is_supported_format(mime_type: &str) -> bool {
    matches!(
        mime_type,
        "image/jpeg"
            | "image/jpg"
            | "image/png"
            | "image/bmp"
            | "image/gif"
            | "image/webp"
            | "image/tiff"
            | "image/avif"
            | "image/heic"
            | "image/heif"
    )
}
