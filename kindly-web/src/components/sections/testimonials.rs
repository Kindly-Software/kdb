use leptos::prelude::*;
use crate::components::molecular::{SectionContainer, Testimonial};

#[component]
pub fn Testimonials() -> impl IntoView {
    view! {
        <SectionContainer id="testimonials" class="testimonials-section">
            <h2 class="section-title">"What Developers Say"</h2>
            <div class="testimonials-carousel">
                <Testimonial
                    quote="The optimized architecture delivered 3-10× speedups in our data validation pipeline with predictable tail latency. Game-changing performance."
                    author="Sarah Chen"
                    role="CTO, AI Startup"
                />
                <Testimonial
                    quote="Data integrity verification gave us SOX compliance out of the box. The audit tools saved us weeks during our last compliance review."
                    author="Michael Rodriguez"
                    role="Security Engineer, FinTech"
                />
                <Testimonial
                    quote="Zero dependencies and compile-time verification made integration seamless. Rigorous benchmarking proved all performance claims."
                    author="Emily Zhang"
                    role="Principal Engineer, Enterprise SaaS"
                />
            </div>
        </SectionContainer>
    }
}
