use leptos::prelude::*;

#[component]
pub fn StatCard(
    number: &'static str,
    label: &'static str,
    #[prop(optional)] trend: Option<&'static str>,
    #[prop(optional)] trend_positive: bool,
) -> impl IntoView {
    view! {
        <div class="stat-card">
            <div class="stat-number">{number}</div>
            <div class="stat-label">{label}</div>
            {trend.map(|t| {
                let trend_class = if trend_positive {
                    "stat-trend stat-trend-positive"
                } else {
                    "stat-trend stat-trend-negative"
                };
                view! {
                    <div class={trend_class}>
                        <i class={if trend_positive { "icon icon-arrow-up" } else { "icon icon-arrow-down" }}></i>
                        <span>{t}</span>
                    </div>
                }
            })}
        </div>
    }
}
