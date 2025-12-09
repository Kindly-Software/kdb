use leptos::prelude::*;

/// Byzantine purple horizontal section divider with holographic effect
#[component]
pub fn SectionDivider() -> impl IntoView {
    view! {
        <div
            class="section-divider"
            style="width: 100%; \
                   height: 4px; \
                   background: linear-gradient(90deg, \
                       transparent 0%, \
                       rgba(75, 0, 130, 0.3) 10%, \
                       rgba(138, 43, 226, 0.6) 50%, \
                       rgba(75, 0, 130, 0.3) 90%, \
                       transparent 100%); \
                   box-shadow: \
                       0 0 20px rgba(75, 0, 130, 0.5), \
                       inset 0 0 10px rgba(138, 43, 226, 0.3); \
                   animation: holographic-shimmer 3s ease-in-out infinite; \
                   pointer-events: none;"
        />
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_section_divider_compiles() {
        // Ensures component compiles
    }
}
