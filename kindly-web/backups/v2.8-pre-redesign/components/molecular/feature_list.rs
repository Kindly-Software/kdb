use leptos::prelude::*;

#[component]
pub fn FeatureList(
    items: Vec<&'static str>,
) -> impl IntoView {
    view! {
        <ul class="feature-list">
            {items.into_iter().map(|item| {
                view! {
                    <li class="feature-list-item">
                        <i class="icon icon-check-circle"></i>
                        <span>{item}</span>
                    </li>
                }
            }).collect::<Vec<_>>()}
        </ul>
    }
}
