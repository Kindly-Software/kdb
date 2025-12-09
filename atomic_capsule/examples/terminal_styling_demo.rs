//! Terminal Styling Demo
//!
//! Demonstrates the StyleCapsule and ColorCapsule for terminal text formatting.
//!
//! Run with:
//! ```bash
//! cargo run --example terminal_styling_demo --features tui-terminal
//! ```

use atomic_capsule::terminal::output::{
    StyleCapsule, Color, BOLD, ITALIC, UNDERLINE, BLINK, REVERSE, STRIKETHROUGH,
};

fn main() {
    println!("\n=== Terminal Styling Demo ===\n");

    // Basic styles
    println!("{}Basic Styles:{}", StyleCapsule::new().bold().to_ansi(), StyleCapsule::reset().to_ansi());

    let bold = StyleCapsule::new().bold();
    println!("{}Bold text{}", bold.to_ansi(), StyleCapsule::reset().to_ansi());

    let italic = StyleCapsule::new().italic();
    println!("{}Italic text{}", italic.to_ansi(), StyleCapsule::reset().to_ansi());

    let underline = StyleCapsule::new().underline();
    println!("{}Underlined text{}", underline.to_ansi(), StyleCapsule::reset().to_ansi());

    let strikethrough = StyleCapsule::new().strikethrough();
    println!("{}Strikethrough text{}", strikethrough.to_ansi(), StyleCapsule::reset().to_ansi());

    // Combined styles
    println!("\n{}Combined Styles:{}", StyleCapsule::new().bold().to_ansi(), StyleCapsule::reset().to_ansi());

    let bold_italic = StyleCapsule::new().bold().italic();
    println!("{}Bold + Italic{}", bold_italic.to_ansi(), StyleCapsule::reset().to_ansi());

    let bold_underline = StyleCapsule::new().bold().underline();
    println!("{}Bold + Underline{}", bold_underline.to_ansi(), StyleCapsule::reset().to_ansi());

    // Standard 16 colors (foreground)
    println!("\n{}Standard 16 Colors (Foreground):{}", StyleCapsule::new().bold().to_ansi(), StyleCapsule::reset().to_ansi());

    let red = StyleCapsule::new().fg(Color::Red);
    println!("{}Red text{}", red.to_ansi(), StyleCapsule::reset().to_ansi());

    let green = StyleCapsule::new().fg(Color::Green);
    println!("{}Green text{}", green.to_ansi(), StyleCapsule::reset().to_ansi());

    let blue = StyleCapsule::new().fg(Color::Blue);
    println!("{}Blue text{}", blue.to_ansi(), StyleCapsule::reset().to_ansi());

    let yellow = StyleCapsule::new().fg(Color::Yellow);
    println!("{}Yellow text{}", yellow.to_ansi(), StyleCapsule::reset().to_ansi());

    let magenta = StyleCapsule::new().fg(Color::Magenta);
    println!("{}Magenta text{}", magenta.to_ansi(), StyleCapsule::reset().to_ansi());

    let cyan = StyleCapsule::new().fg(Color::Cyan);
    println!("{}Cyan text{}", cyan.to_ansi(), StyleCapsule::reset().to_ansi());

    // Bright colors
    println!("\n{}Bright Colors:{}", StyleCapsule::new().bold().to_ansi(), StyleCapsule::reset().to_ansi());

    let bright_red = StyleCapsule::new().fg(Color::BrightRed);
    println!("{}Bright Red{}", bright_red.to_ansi(), StyleCapsule::reset().to_ansi());

    let bright_green = StyleCapsule::new().fg(Color::BrightGreen);
    println!("{}Bright Green{}", bright_green.to_ansi(), StyleCapsule::reset().to_ansi());

    let bright_blue = StyleCapsule::new().fg(Color::BrightBlue);
    println!("{}Bright Blue{}", bright_blue.to_ansi(), StyleCapsule::reset().to_ansi());

    // Background colors
    println!("\n{}Background Colors:{}", StyleCapsule::new().bold().to_ansi(), StyleCapsule::reset().to_ansi());

    let red_bg = StyleCapsule::new().bg(Color::Red).fg(Color::White);
    println!("{}White text on red background{}", red_bg.to_ansi(), StyleCapsule::reset().to_ansi());

    let green_bg = StyleCapsule::new().bg(Color::Green).fg(Color::Black);
    println!("{}Black text on green background{}", green_bg.to_ansi(), StyleCapsule::reset().to_ansi());

    // RGB (24-bit True Color)
    println!("\n{}RGB (24-bit True Color):{}", StyleCapsule::new().bold().to_ansi(), StyleCapsule::reset().to_ansi());

    let orange = StyleCapsule::new().fg(Color::Rgb(255, 128, 0));
    println!("{}Orange text (RGB 255, 128, 0){}", orange.to_ansi(), StyleCapsule::reset().to_ansi());

    let purple = StyleCapsule::new().fg(Color::Rgb(128, 0, 255));
    println!("{}Purple text (RGB 128, 0, 255){}", purple.to_ansi(), StyleCapsule::reset().to_ansi());

    let teal = StyleCapsule::new().fg(Color::Rgb(0, 200, 200));
    println!("{}Teal text (RGB 0, 200, 200){}", teal.to_ansi(), StyleCapsule::reset().to_ansi());

    // 256-color palette
    println!("\n{}256-Color Palette (selected colors):{}", StyleCapsule::new().bold().to_ansi(), StyleCapsule::reset().to_ansi());

    let ansi196 = StyleCapsule::new().fg(Color::Ansi256(196));
    println!("{}Color 196 (bright red){}", ansi196.to_ansi(), StyleCapsule::reset().to_ansi());

    let ansi226 = StyleCapsule::new().fg(Color::Ansi256(226));
    println!("{}Color 226 (bright yellow){}", ansi226.to_ansi(), StyleCapsule::reset().to_ansi());

    let ansi51 = StyleCapsule::new().fg(Color::Ansi256(51));
    println!("{}Color 51 (bright cyan){}", ansi51.to_ansi(), StyleCapsule::reset().to_ansi());

    // Complex combinations
    println!("\n{}Complex Combinations:{}", StyleCapsule::new().bold().to_ansi(), StyleCapsule::reset().to_ansi());

    let fancy = StyleCapsule::new()
        .bold()
        .italic()
        .underline()
        .fg(Color::Rgb(255, 100, 200))
        .bg(Color::Rgb(20, 20, 40));
    println!("{}Bold Italic Underline Pink on Dark Blue{}", fancy.to_ansi(), StyleCapsule::reset().to_ansi());

    // Gradient effect using RGB
    println!("\n{}RGB Gradient Effect:{}", StyleCapsule::new().bold().to_ansi(), StyleCapsule::reset().to_ansi());
    for i in 0..=10 {
        let r = 255;
        let g = (255 * i / 10) as u8;
        let b = 0;
        let style = StyleCapsule::new().fg(Color::Rgb(r, g, b)).bold();
        print!("{}■{}", style.to_ansi(), StyleCapsule::reset().to_ansi());
    }
    println!();

    // Color bars
    println!("\n{}Color Bars:{}", StyleCapsule::new().bold().to_ansi(), StyleCapsule::reset().to_ansi());
    let colors = [
        Color::Red, Color::Green, Color::Blue, Color::Yellow,
        Color::Magenta, Color::Cyan, Color::BrightRed, Color::BrightGreen,
    ];
    for color in &colors {
        let style = StyleCapsule::new().bg(*color);
        print!("{}    {}", style.to_ansi(), StyleCapsule::reset().to_ansi());
    }
    println!();

    // Performance demonstration
    println!("\n{}Performance Metrics:{}", StyleCapsule::new().bold().to_ansi(), StyleCapsule::reset().to_ansi());
    println!("StyleCapsule size: {} bytes (cache-aligned)", core::mem::size_of::<StyleCapsule>());
    println!("ColorCapsule size: {} bytes (cache-aligned)", core::mem::size_of::<atomic_capsule::terminal::output::ColorCapsule>());
    println!("Zero heap allocations for escape sequence generation");
    println!("Lockfree atomic operations (T1 Atomic tier)");

    println!("\n{}Demo complete!{}", StyleCapsule::new().bold().fg(Color::Green).to_ansi(), StyleCapsule::reset().to_ansi());
}
