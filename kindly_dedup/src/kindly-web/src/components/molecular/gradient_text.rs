use leptos::children::Children;
use leptos::prelude::*;

#[component]
pub fn GradientText(
    #[prop(optional)] from: Option<&'static str>,
    #[prop(optional)] to: Option<&'static str>,
    children: Children,
) -> impl IntoView {
    let style = format!(
        "background: linear-gradient(135deg, {} 0%, {} 100%); -webkit-background-clip: text; -webkit-text-fill-color: transparent; background-clip: text;",
        from.unwrap_or("#3b82f6"),
        to.unwrap_or("#8b5cf6")
    );

    view! {
        <span class="gradient-text" style={style}>
            {children()}
        </span>
    }
}
