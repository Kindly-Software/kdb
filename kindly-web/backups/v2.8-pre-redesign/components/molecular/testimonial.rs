use leptos::prelude::*;

#[component]
pub fn Testimonial(
    quote: &'static str,
    author: &'static str,
    role: &'static str,
    #[prop(optional)] avatar: Option<&'static str>,
) -> impl IntoView {
    view! {
        <div class="testimonial">
            <div class="testimonial-quote">
                <i class="icon icon-quote"></i>
                <p>{quote}</p>
            </div>
            <div class="testimonial-author">
                {avatar.as_ref().map(|url| {
                    view! {
                        <img src={*url} alt={author} class="author-avatar" />
                    }
                })}
                <div class="author-info">
                    <div class="author-name">{author}</div>
                    <div class="author-role">{role}</div>
                </div>
            </div>
        </div>
    }
}
