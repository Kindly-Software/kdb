use crate::components::molecular::SectionContainer;
use leptos::prelude::*;

#[component]
pub fn Comparison() -> impl IntoView {
    view! {
        <SectionContainer id="comparison" class="comparison-section">
            <h2 class="section-title">"Why Computational Capsules?"</h2>
            <div class="comparison-table">
                <div class="comparison-header">
                    <div class="comparison-cell">"Feature"</div>
                    <div class="comparison-cell">"Traditional Mutex"</div>
                    <div class="comparison-cell highlight">"Computational Capsules"</div>
                </div>
                <div class="comparison-row">
                    <div class="comparison-cell">"Budget Check"</div>
                    <div class="comparison-cell">"~200ns"</div>
                    <div class="comparison-cell highlight">"<60ns (3× faster)"</div>
                </div>
                <div class="comparison-row">
                    <div class="comparison-cell">"Lock Contention"</div>
                    <div class="comparison-cell">"Blocks threads"</div>
                    <div class="comparison-cell highlight">"100% lockfree"</div>
                </div>
                <div class="comparison-row">
                    <div class="comparison-cell">"Tail Latency"</div>
                    <div class="comparison-cell">"Unpredictable"</div>
                    <div class="comparison-cell highlight">"Predictable p99"</div>
                </div>
                <div class="comparison-row">
                    <div class="comparison-cell">"Scalability"</div>
                    <div class="comparison-cell">"Degrades under load"</div>
                    <div class="comparison-cell highlight">"Linear to 8 threads"</div>
                </div>
                <div class="comparison-row">
                    <div class="comparison-cell">"Memory Safety"</div>
                    <div class="comparison-cell">"Runtime checks"</div>
                    <div class="comparison-cell highlight">"Compile-time verified"</div>
                </div>
                <div class="comparison-row">
                    <div class="comparison-cell">"Dependencies"</div>
                    <div class="comparison-cell">"Multiple crates"</div>
                    <div class="comparison-cell highlight">"Zero dependencies"</div>
                </div>
            </div>
        </SectionContainer>
    }
}
