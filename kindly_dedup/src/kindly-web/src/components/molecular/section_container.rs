use leptos::children::Children;
use leptos::prelude::*;

#[component]
pub fn SectionContainer(
    #[prop(optional)] id: Option<&'static str>,
    #[prop(optional)] class: Option<&'static str>,
    children: Children,
) -> impl IntoView {
    let container_class = format!("section-container {}", class.unwrap_or(""));

    view! {
        <section id={id} class={container_class}>
            <div class="section-inner">
                {children()}
            </div>
        </section>
    }
}
