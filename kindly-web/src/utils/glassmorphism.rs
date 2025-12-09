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

/// Generate frosted glass navbar style (purple semi-transparent background)
pub fn navbar_style() -> String {
    format!(
        "backdrop-filter: blur({}px) saturate(150%); \
         -webkit-backdrop-filter: blur({}px) saturate(150%); \
         background: linear-gradient(135deg, rgba(75, 0, 130, 0.85), rgba(102, 51, 153, 0.85)); \
         border-bottom: 1px solid rgba(255, 215, 0, 0.2); \
         box-shadow: 0 8px 32px rgba(75, 0, 130, 0.3), \
                     inset 0 1px 0 rgba(255, 255, 255, 0.5);",
        BlurLevel::Md as u8,
        BlurLevel::Md as u8
    )
}

/// Generate glass card style (darker for redesign)
pub fn card_style() -> String {
    format!(
        "backdrop-filter: blur({}px) saturate(150%); \
         -webkit-backdrop-filter: blur({}px) saturate(150%); \
         background: rgba(26, 0, 40, 0.8); \
         border: 1px solid rgba(75, 0, 130, 0.3); \
         border-radius: 16px; \
         box-shadow: 0 20px 25px -5px rgba(112, 60, 139, 0.3);",
        BlurLevel::Md as u8,
        BlurLevel::Md as u8
    )
}

/// Generate dark section background (60% foundation)
pub fn dark_section_background() -> String {
    "background: linear-gradient(180deg, #0A0014 0%, #1A0028 50%, #2D0052 100%);".to_string()
}

/// Generate dark solid card style
pub fn dark_card_style() -> String {
    "background: rgba(26, 0, 40, 0.95); \
     border: 1px solid rgba(75, 0, 130, 0.4); \
     border-radius: 16px; \
     box-shadow: 0 20px 25px -5px rgba(0, 0, 0, 0.5), \
                 0 0 0 1px rgba(255, 215, 0, 0.1);".to_string()
}

/// Generate featured card with gold border (hero elements)
pub fn featured_card_style() -> String {
    "background: linear-gradient(135deg, rgba(75, 0, 130, 0.4), rgba(102, 51, 153, 0.3)); \
     backdrop-filter: blur(16px) saturate(150%); \
     -webkit-backdrop-filter: blur(16px) saturate(150%); \
     border: 2px solid rgba(255, 215, 0, 0.6); \
     border-radius: 16px; \
     box-shadow: 0 20px 40px -5px rgba(255, 215, 0, 0.3), \
                 inset 0 1px 0 rgba(255, 255, 255, 0.2);".to_string()
}

/// Generate cyan gradient text (tech accent)
pub fn cyan_gradient_text() -> String {
    "background: linear-gradient(135deg, #00D9FF 0%, #0099CC 100%); \
     -webkit-background-clip: text; \
     -webkit-text-fill-color: transparent; \
     background-clip: text; \
     font-weight: 800; \
     letter-spacing: -0.02em;".to_string()
}

/// Generate Byzantine purple gradient text
pub fn purple_gradient_text() -> String {
    "background: linear-gradient(135deg, #8A2BE2 0%, #663399 100%); \
     -webkit-background-clip: text; \
     -webkit-text-fill-color: transparent; \
     background-clip: text; \
     font-weight: 800; \
     letter-spacing: -0.02em;".to_string()
}

/// Generate dark card hover state
pub fn dark_card_hover_style() -> String {
    "background: rgba(26, 0, 40, 1.0); \
     border: 1px solid rgba(255, 215, 0, 0.6); \
     border-radius: 16px; \
     box-shadow: 0 25px 50px -5px rgba(255, 215, 0, 0.2), \
                 0 0 0 1px rgba(255, 215, 0, 0.2); \
     transform: translateY(-4px);".to_string()
}

/// Generate blur filter for given level
pub fn blur_level(level: BlurLevel) -> String {
    format!("backdrop-filter: blur({}px);", level as u8)
}

/// Generate hero gradient background (smooth extended transition)
pub fn hero_gradient() -> String {
    "background: #0A0014; \
     background: radial-gradient(ellipse 100% 120% at center top, #4B0082 0%, #3A006B 15%, #2D0052 35%, #1A0028 55%, #0A0014 75%, #0A0014 100%);".to_string()
}

/// Generate metallic gold gradient text
pub fn gold_gradient_text() -> String {
    "background: linear-gradient(135deg, #D4AF37 0%, #F0D848 100%); \
     -webkit-background-clip: text; \
     -webkit-text-fill-color: transparent; \
     background-clip: text; \
     font-weight: 800; \
     letter-spacing: -0.02em;".to_string()
}

/// Generate multi-layer Byzantine Royal background (darker with purple accents)
pub fn byzantine_background() -> String {
    "background: \
       radial-gradient(ellipse at top, rgba(75, 0, 130, 0.2), transparent 50%), \
       radial-gradient(ellipse at bottom right, rgba(138, 43, 226, 0.15), transparent 60%), \
       linear-gradient(135deg, #0A0014 0%, #1A0028 50%, #2D0052 100%);".to_string()
}

/// Generate premium gold button style (stronger glow)
pub fn gold_button_style() -> String {
    "background: linear-gradient(135deg, #D4AF37 0%, #E69B00 100%); \
     box-shadow: \
       0 8px 24px rgba(212, 175, 55, 0.5), \
       inset 0 1px 0 rgba(255, 255, 255, 0.4); \
     border: none; \
     color: #0A0014; \
     font-weight: 800; \
     transition: all 0.3s ease;".to_string()
}

/// Generate gold button hover style
pub fn gold_button_hover_style() -> String {
    "background: linear-gradient(135deg, #D4AF37 0%, #E69B00 100%); \
     box-shadow: \
       0 8px 20px rgba(212, 175, 55, 0.5), \
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

/// Generate responsive navbar blur based on scroll position (purple semi-transparent background)
pub fn navbar_blur_responsive(scroll_y: u32) -> String {
    let blur = match scroll_y {
        0..=50 => 8,
        51..=200 => 16,
        201..=500 => 24,
        _ => 32,
    };

    let opacity = if scroll_y > 50 { "0.90" } else { "0.85" };

    format!(
        "backdrop-filter: blur({}px) saturate(150%); \
         -webkit-backdrop-filter: blur({}px) saturate(150%); \
         background: linear-gradient(135deg, rgba(75, 0, 130, {}), rgba(102, 51, 153, {})); \
         border-bottom: 1px solid rgba(255, 215, 0, 0.2); \
         box-shadow: 0 8px 32px rgba(75, 0, 130, 0.3), \
                     inset 0 1px 0 rgba(255, 255, 255, 0.5); \
         transition: all 0.3s ease;",
        blur,
        blur,
        opacity,
        opacity
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
        assert!(style.contains("blur(16px)"));
        assert!(style.contains("rgba(75, 0, 130, 0.85)"));  // Purple semi-transparent v2.27
        assert!(style.contains("linear-gradient"));
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
        assert!(style.contains("#D4AF37"));  // Darker gold v2.12
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
        assert!(low.contains("rgba(75, 0, 130, 0.85)"));  // Purple semi-transparent v2.27
        assert!(high.contains("rgba(75, 0, 130, 0.90)"));  // Higher opacity when scrolled
    }
}
