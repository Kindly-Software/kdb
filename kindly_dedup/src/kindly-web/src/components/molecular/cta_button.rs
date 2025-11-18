use leptos::prelude::*;

#[component]
pub fn CtaButton(
    text: &'static str,
    href: &'static str,
    #[prop(optional)] variant: Option<&'static str>,
    #[prop(optional)] size: Option<&'static str>,
) -> impl IntoView {
    let class = format!(
        "cta-button btn-{} btn-size-{}",
        variant.unwrap_or("primary"),
        size.unwrap_or("large")
    );

    view! {
        <a href={href} class={class}>
            <span class="cta-text">{text}</span>
            <i class="icon icon-arrow-right"></i>
        </a>
    }
}
