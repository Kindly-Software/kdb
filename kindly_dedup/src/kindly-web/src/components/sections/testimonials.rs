use crate::components::molecular::{SectionContainer, Testimonial};
use leptos::prelude::*;

#[component]
pub fn Testimonials() -> impl IntoView {
    view! {
        <SectionContainer id="testimonials" class="testimonials-section">
            <h2 class="section-title">"What Developers Say"</h2>
            <div class="testimonials-carousel">
                <Testimonial
                    quote="The computational capsule architecture eliminated all our lock contention issues. We saw 3-10× speedups in budget validation with predictable tail latency."
                    author="Sarah Chen"
                    role="CTO, AI Startup"
                />
                <Testimonial
                    quote="Hash chain integrity verification gave us SOX compliance out of the box. The forensic analysis tools saved us weeks during our last audit."
                    author="Michael Rodriguez"
                    role="Security Engineer, FinTech"
                />
                <Testimonial
                    quote="Zero dependencies and compile-time verification made integration seamless. The B32 benchmarking framework proved all performance claims."
                    author="Emily Zhang"
                    role="Principal Engineer, Enterprise SaaS"
                />
            </div>
        </SectionContainer>
    }
}
