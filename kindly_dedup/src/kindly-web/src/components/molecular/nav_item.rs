use leptos::children::Children;
use leptos::prelude::*;

#[component]
pub fn NavItem(href: &'static str, #[prop(optional)] active: bool, children: Children) -> impl IntoView {
    let class = if active { "nav-item nav-item-active" } else { "nav-item" };

    view! {
        <a href={href} class={class}>
            {children()}
        </a>
    }
}
