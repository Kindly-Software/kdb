use leptos::prelude::*;

#[component]
pub fn HeroHeading(
    title: &'static str,
    #[prop(optional)] subtitle: Option<&'static str>,
    #[prop(optional)] gradient: bool,
) -> impl IntoView {
    view! {
        <div class="hero-heading">
            <h1 class={if gradient { "hero-title hero-title-gradient" } else { "hero-title" }}>
                {title}
            </h1>
            {subtitle.map(|text| {
                view! {
                    <p class="hero-subtitle">{text}</p>
                }
            })}
        </div>
    }
}
