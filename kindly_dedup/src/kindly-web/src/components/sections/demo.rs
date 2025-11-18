use crate::components::common::{Button, ButtonSize, ButtonVariant};
use crate::components::molecular::SectionContainer;
use leptos::prelude::*;

#[component]
pub fn Demo() -> impl IntoView {
    view! {
        <SectionContainer id="demo" class="demo-section">
            <h2 class="section-title">"Try It Yourself"</h2>
            <p class="section-subtitle">
                "Download the demo binary and validate performance claims on your hardware"
            </p>

            <div class="demo-cta">
                <Button
                    variant=ButtonVariant::Primary
                    size=ButtonSize::Large
                    full_width=false
                >
                    "📥 Download Demo Binary"
                </Button>
                <p class="demo-limit">
                    "5M document limit • Hardware-bound protection • No registration required"
                </p>
            </div>

            <div class="demo-tiers">
                <h3 class="tiers-title">"Demo Validation Tiers"</h3>
                <div class="tiers-grid">
                    <div class="tier-card">
                        <div class="tier-header">
                            <span class="tier-number">"1"</span>
                            <h4>"Accuracy Proof"</h4>
                        </div>
                        <ul class="tier-specs">
                            <li>"100K documents"</li>
                            <li>"100% F1 score validation"</li>
                            <li>"~17 minutes runtime"</li>
                            <li>"2 GB RAM minimum"</li>
                        </ul>
                    </div>
                    <div class="tier-card tier-featured">
                        <div class="tier-header">
                            <span class="tier-number">"2"</span>
                            <h4>"Production Speed"</h4>
                        </div>
                        <ul class="tier-specs">
                            <li>"1M documents"</li>
                            <li>"60K+ docs/sec measured"</li>
                            <li>"~17 seconds runtime"</li>
                            <li>"4 GB RAM minimum"</li>
                        </ul>
                    </div>
                    <div class="tier-card">
                        <div class="tier-header">
                            <span class="tier-number">"3"</span>
                            <h4>"Massive Scale"</h4>
                        </div>
                        <ul class="tier-specs">
                            <li>"10M documents"</li>
                            <li>"912K docs/sec @ 16 cores"</li>
                            <li>"~11 seconds runtime"</li>
                            <li>"8 GB RAM minimum"</li>
                        </ul>
                    </div>
                </div>
            </div>

            <div class="system-requirements">
                <h3 class="requirements-title">"System Requirements"</h3>
                <table class="requirements-table">
                    <thead>
                        <tr>
                            <th>"Component"</th>
                            <th>"Minimum"</th>
                            <th>"Recommended"</th>
                        </tr>
                    </thead>
                    <tbody>
                        <tr>
                            <td>"CPU"</td>
                            <td>"x86-64 with SSE4.2"</td>
                            <td>"AMD Ryzen 9 / Intel Core i9"</td>
                        </tr>
                        <tr>
                            <td>"RAM"</td>
                            <td>"2 GB (Tier 1)"</td>
                            <td>"16 GB (all tiers)"</td>
                        </tr>
                        <tr>
                            <td>"Cores"</td>
                            <td>"1 (single-threaded)"</td>
                            <td>"8-16 (parallel mode)"</td>
                        </tr>
                        <tr>
                            <td>"OS"</td>
                            <td>"Linux / macOS / Windows"</td>
                            <td>"Linux (fastest)"</td>
                        </tr>
                        <tr>
                            <td>"Disk"</td>
                            <td>"100 MB"</td>
                            <td>"10 GB (persistent mode)"</td>
                        </tr>
                    </tbody>
                </table>
            </div>

            <div class="performance-by-hardware">
                <h3 class="hardware-title">"Expected Performance by Hardware"</h3>
                <table class="hardware-table">
                    <thead>
                        <tr>
                            <th>"CPU"</th>
                            <th>"Cores"</th>
                            <th>"Single-Threaded"</th>
                            <th>"Multi-Threaded"</th>
                        </tr>
                    </thead>
                    <tbody>
                        <tr>
                            <td>"AMD Ryzen 9 6900HX"</td>
                            <td>"16"</td>
                            <td>"60K docs/sec"</td>
                            <td>"912K docs/sec"</td>
                        </tr>
                        <tr>
                            <td>"Intel Core i9-12900K"</td>
                            <td>"16"</td>
                            <td>"55-65K docs/sec"</td>
                            <td>"850K-950K docs/sec"</td>
                        </tr>
                        <tr>
                            <td>"AMD Ryzen 7 5800X"</td>
                            <td>"8"</td>
                            <td>"50-60K docs/sec"</td>
                            <td>"400K-500K docs/sec"</td>
                        </tr>
                        <tr>
                            <td>"Intel Core i5-12600K"</td>
                            <td>"10"</td>
                            <td>"45-55K docs/sec"</td>
                            <td>"450K-550K docs/sec"</td>
                        </tr>
                    </tbody>
                </table>
                <p class="hardware-note">
                    "⚠️ Performance estimates based on B32 validated baseline. "
                    "Actual results depend on CPU microarchitecture, RAM speed, and system load. "
                    "Demo measures your hardware's actual performance."
                </p>
            </div>

            <div class="demo-protection">
                <h3 class="protection-title">"Demo Protection"</h3>
                <ul class="protection-features">
                    <li>"🔒 Hardware-bound licensing (survives reinstallation)"</li>
                    <li>"🛡️ 4-layer META_CAPSULE protection"</li>
                    <li>"📊 Q34 audit trail (compliance-ready)"</li>
                    <li>"⚡ Zero performance overhead (<0.3%)"</li>
                    <li>"✅ 5M document limit enforced"</li>
                </ul>
            </div>
        </SectionContainer>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_demo_compiles() {
        // Ensures component compiles
    }
}
