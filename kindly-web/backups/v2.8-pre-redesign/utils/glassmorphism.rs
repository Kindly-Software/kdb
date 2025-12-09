//! Glassmorphism utilities - Frosted glass effects
//!
//! Plain function-based implementation (no capsules, trade secret protected)

/// Glassmorphism blur levels
pub enum BlurLevel {
    Sm = 8,   // 8px
    Md = 16,  // 16px
    Lg = 24,  // 24px
    Xl = 32,  // 32px
}

/// Generate frosted glass navbar style
pub fn navbar_style() -> String {
    format!(
        "backdrop-filter: blur({}px) saturate(180%); \
         -webkit-backdrop-filter: blur({}px) saturate(180%); \
         background: rgba(102, 51, 153, 0.4); \
         border-bottom: 1px solid rgba(255, 255, 255, 0.1); \
         box-shadow: 0 8px 32px rgba(45, 0, 82, 0.5), \
                     0 4px 16px rgba(75, 0, 130, 0.3), \
                     0 2px 8px rgba(102, 51, 153, 0.15), \
                     inset 0 1px 0 rgba(255, 237, 78, 0.3);",
        BlurLevel::Lg as u8,
        BlurLevel::Lg as u8
    )
}

/// Generate glass card style
pub fn card_style() -> String {
    format!(
        "backdrop-filter: blur({}px) saturate(180%); \
         -webkit-backdrop-filter: blur({}px) saturate(180%); \
         background: rgba(102, 51, 153, 0.2); \
         border: 1px solid rgba(255, 237, 78, 0.3); \
         border-radius: 16px; \
         box-shadow: 0 20px 25px -5px rgba(112, 60, 139, 0.3);",
        BlurLevel::Md as u8,
        BlurLevel::Md as u8
    )
}

/// Generate blur filter for given level
pub fn blur_level(level: BlurLevel) -> String {
    format!("backdrop-filter: blur({}px);", level as u8)
}

/// Generate hero gradient background
pub fn hero_gradient() -> String {
    "background: linear-gradient(135deg, #4B0082 0%, #E6D5F5 100%);".to_string()
}

/// Generate metallic gold gradient text
pub fn gold_gradient_text() -> String {
    "background: linear-gradient(135deg, #FFD700 0%, #FFED4E 100%); \
     -webkit-background-clip: text; \
     -webkit-text-fill-color: transparent; \
     background-clip: text; \
     font-weight: 800; \
     letter-spacing: -0.02em;".to_string()
}

/// Generate multi-layer Byzantine Royal background
pub fn byzantine_background() -> String {
    "background: \
       radial-gradient(ellipse at top, rgba(75, 0, 130, 0.4), transparent 50%), \
       radial-gradient(ellipse at bottom right, rgba(138, 43, 226, 0.3), transparent 60%), \
       linear-gradient(135deg, #2D0052 0%, #4B0082 50%, #663399 100%);".to_string()
}

/// Generate premium gold button style
pub fn gold_button_style() -> String {
    "background: linear-gradient(135deg, #FFD700 0%, #FFA500 100%); \
     box-shadow: \
       0 4px 14px rgba(255, 215, 0, 0.4), \
       inset 0 1px 0 rgba(255, 255, 255, 0.3); \
     border: none; \
     color: #2D0052; \
     font-weight: 700; \
     transition: all 0.3s ease;".to_string()
}

/// Generate gold button hover style
pub fn gold_button_hover_style() -> String {
    "background: linear-gradient(135deg, #FFD700 0%, #FFA500 100%); \
     box-shadow: \
       0 8px 20px rgba(255, 215, 0, 0.5), \
       inset 0 1px 0 rgba(255, 255, 255, 0.4); \
     transform: translateY(-2px); \
     color: #2D0052;".to_string()
}

/// Generate purple secondary button style
pub fn purple_button_style() -> String {
    "background: rgba(102, 51, 153, 0.6); \
     backdrop-filter: blur(8px) saturate(180%); \
     -webkit-backdrop-filter: blur(8px) saturate(180%); \
     border: 1px solid rgba(255, 237, 78, 0.3); \
     box-shadow: \
       0 4px 14px rgba(102, 51, 153, 0.4), \
       inset 0 1px 0 rgba(255, 255, 255, 0.2); \
     color: #FFED4E; \
     font-weight: 700; \
     transition: all 0.3s ease;".to_string()
}

/// Generate purple button hover style
pub fn purple_button_hover_style() -> String {
    "background: rgba(102, 51, 153, 0.8); \
     backdrop-filter: blur(12px) saturate(180%); \
     -webkit-backdrop-filter: blur(12px) saturate(180%); \
     border: 1px solid rgba(255, 237, 78, 0.5); \
     box-shadow: \
       0 8px 20px rgba(102, 51, 153, 0.6), \
       inset 0 1px 0 rgba(255, 255, 255, 0.3); \
     transform: translateY(-2px); \
     color: #FFD700;".to_string()
}

/// Generate glass card with hover effect
pub fn card_hover_style() -> String {
    format!(
        "backdrop-filter: blur({}px) saturate(180%); \
         -webkit-backdrop-filter: blur({}px) saturate(180%); \
         background: rgba(102, 51, 153, 0.3); \
         border: 1px solid rgba(255, 237, 78, 0.5); \
         border-radius: 16px; \
         box-shadow: \
           0 25px 30px -5px rgba(112, 60, 139, 0.5), \
           inset 0 1px 0 rgba(255, 255, 255, 0.2); \
         transform: translateY(-4px);",
        BlurLevel::Lg as u8,
        BlurLevel::Lg as u8
    )
}

/// Generate responsive navbar blur based on scroll position
pub fn navbar_blur_responsive(scroll_y: u32) -> String {
    let blur = match scroll_y {
        0..=50 => 8,
        51..=200 => 16,
        201..=500 => 24,
        _ => 32,
    };

    format!(
        "backdrop-filter: blur({}px) saturate(180%); \
         -webkit-backdrop-filter: blur({}px) saturate(180%); \
         background: rgba(102, 51, 153, {}); \
         border-bottom: 1px solid rgba(255, 255, 255, 0.1); \
         box-shadow: 0 8px 32px rgba(45, 0, 82, {}), \
                     0 4px 16px rgba(75, 0, 130, 0.3), \
                     0 2px 8px rgba(102, 51, 153, 0.15), \
                     inset 0 1px 0 rgba(255, 237, 78, 0.3); \
         transition: all 0.3s ease;",
        blur,
        blur,
        if scroll_y > 50 { "0.6" } else { "0.4" },
        if scroll_y > 50 { "0.6" } else { "0.5" }
    )
}

/// Generate hero section gradient overlay
pub fn hero_overlay() -> String {
    "background: radial-gradient(ellipse at center, transparent 0%, rgba(45, 0, 82, 0.4) 100%);".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blur_levels() {
        assert_eq!(BlurLevel::Sm as u8, 8);
        assert_eq!(BlurLevel::Md as u8, 16);
        assert_eq!(BlurLevel::Lg as u8, 24);
        assert_eq!(BlurLevel::Xl as u8, 32);
    }

    #[test]
    fn test_navbar_style() {
        let style = navbar_style();
        assert!(style.contains("blur(24px)"));
        assert!(style.contains("rgba(102, 51, 153, 0.4)"));
    }

    #[test]
    fn test_card_style() {
        let style = card_style();
        assert!(style.contains("blur(16px)"));
        assert!(style.contains("border-radius: 16px"));
    }

    #[test]
    fn test_gold_gradient_text() {
        let style = gold_gradient_text();
        assert!(style.contains("linear-gradient"));
        assert!(style.contains("#FFD700"));
    }

    #[test]
    fn test_byzantine_background() {
        let style = byzantine_background();
        assert!(style.contains("radial-gradient"));
        assert!(style.contains("#2D0052"));
    }

    #[test]
    fn test_responsive_blur() {
        let low = navbar_blur_responsive(0);
        let high = navbar_blur_responsive(600);
        assert!(low.contains("blur(8px)"));
        assert!(high.contains("blur(32px)"));
    }
}
