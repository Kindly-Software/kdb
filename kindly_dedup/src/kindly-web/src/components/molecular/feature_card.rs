use leptos::prelude::*;

#[component]
pub fn FeatureCard(
    #[prop(optional)] icon: Option<&'static str>,
    title: &'static str,
    description: &'static str,
) -> impl IntoView {
    view! {
        <div class="feature-card">
            {icon.map(|i| {
                view! {
                    <div class="feature-icon">
                        <i class={format!("icon {}", i)}></i>
                    </div>
                }
            })}
            <h3 class="feature-title">{title}</h3>
            <p class="feature-description">{description}</p>
        </div>
    }
}
