//! Logo Background Remover
//!
//! Removes light gray/white background from KDB logo and makes it transparent.

use image::{DynamicImage, GenericImageView, Rgba, RgbaImage};

fn main() {
    // Process simplified logo (heart with bug)
    let simple_input = "/home/samuel/Downloads/1000012508.jpg";
    let simple_output = "/home/samuel/Primitives/kindly-services/dist/kdb-logo-simple.png";

    println!("[1/4] Processing simplified logo...");
    let img = image::open(simple_input).expect("Failed to load simplified logo");
    let transparent = remove_background(img);
    transparent.save(simple_output).expect("Failed to save simplified logo");
    println!("✓ Simplified logo: {} ({}x{})", simple_output, transparent.width(), transparent.height());

    // Copy to kdb-api-landing for navbar
    let api_output = "/home/samuel/Primitives/kdb-api-landing/dist/kdb-logo-simple.png";
    std::fs::copy(simple_output, api_output).expect("Failed to copy to API dist");
    println!("✓ Copied to API: {}", api_output);

    // Copy to kdb assets
    let kdb_output = "/home/samuel/Primitives/kdb/assets/web/kdb-logo-simple.png";
    std::fs::copy(simple_output, kdb_output).expect("Failed to copy to kdb assets");
    println!("✓ Copied to KDB assets: {}", kdb_output);

    println!("\n✓ All done! Simplified logo ready for deployment");
}

fn remove_background(img: DynamicImage) -> RgbaImage {
    let (width, height) = img.dimensions();
    let mut output = RgbaImage::new(width, height);

    for y in 0..height {
        for x in 0..width {
            let pixel = img.get_pixel(x, y);
            let (r, g, b, a) = (pixel[0], pixel[1], pixel[2], pixel[3]);

            // Detect light gray/white background
            // Gray pixels have R≈G≈B and high values (>200)
            let is_gray = (r as i16 - g as i16).abs() < 20
                       && (g as i16 - b as i16).abs() < 20
                       && (r as i16 - b as i16).abs() < 20;

            let is_light = r > 200 && g > 200 && b > 200;

            if is_gray && is_light {
                // Make transparent
                output.put_pixel(x, y, Rgba([r, g, b, 0]));
            } else {
                // Keep original pixel
                output.put_pixel(x, y, Rgba([r, g, b, a]));
            }
        }
    }

    output
}
