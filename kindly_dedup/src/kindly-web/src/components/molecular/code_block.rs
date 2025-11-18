use leptos::prelude::*;

#[component]
pub fn CodeBlock(code: &'static str, #[prop(optional)] language: Option<&'static str>) -> impl IntoView {
    view! {
        <div class="code-block">
            <div class="code-header">
                <span class="code-language">
                    {language.unwrap_or("text")}
                </span>
                <button class="code-copy-btn" title="Copy to clipboard">
                    <i class="icon icon-copy"></i>
                </button>
            </div>
            <pre class="code-content">
                <code class={format!("language-{}", language.unwrap_or("text"))}>
                    {code}
                </code>
            </pre>
        </div>
    }
}
