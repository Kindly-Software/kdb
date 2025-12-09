// Placeholder PNG asset generator for kindly-av1 Microsoft Store
// Creates minimal valid PNG files with correct dimensions and purple background

use std::fs::File;
use std::io::Write;

fn create_png(width: u32, height: u32, filename: &str) -> std::io::Result<()> {
    let mut file = File::create(filename)?;

    // Purple color: #9B59B6 = RGB(155, 89, 182)
    let r: u8 = 155;
    let g: u8 = 89;
    let b: u8 = 182;

    // PNG signature
    file.write_all(&[137, 80, 78, 71, 13, 10, 26, 10])?;

    // IHDR chunk
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(b"IHDR");
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.push(8); // Bit depth
    ihdr.push(2); // Color type: RGB
    ihdr.push(0); // Compression method
    ihdr.push(0); // Filter method
    ihdr.push(0); // Interlace method

    write_chunk(&mut file, &ihdr)?;

    // IDAT chunk (compressed image data)
    let mut pixels = Vec::new();
    for _ in 0..height {
        pixels.push(0); // Filter type: None
        for _ in 0..width {
            pixels.push(r);
            pixels.push(g);
            pixels.push(b);
        }
    }

    // Simple zlib compression (simplified for solid color)
    let mut idat = Vec::new();
    idat.extend_from_slice(b"IDAT");

    // Zlib header (deflate, default compression)
    let compressed = miniz_oxide::deflate::compress_to_vec(&pixels, 6);
    idat.extend_from_slice(&compressed);

    write_chunk(&mut file, &idat)?;

    // IEND chunk
    write_chunk(&mut file, b"IEND")?;

    println!("✓ Created {filename} ({width}×{height})");
    Ok(())
}

fn write_chunk(file: &mut File, data: &[u8]) -> std::io::Result<()> {
    let length = (data.len() - 4) as u32; // Exclude chunk type from length
    file.write_all(&length.to_be_bytes())?;
    file.write_all(data)?;

    // CRC32 of chunk type + data
    let crc = crc32fast::hash(data);
    file.write_all(&crc.to_be_bytes())?;

    Ok(())
}

fn main() -> std::io::Result<()> {
    println!("Generating placeholder PNG assets for kindly-av1...");
    println!("Color: #9B59B6 (Byzantine Royal Purple)\n");

    create_png(50, 50, "StoreLogo.png")?;
    create_png(44, 44, "Square44x44Logo.png")?;
    create_png(150, 150, "Square150x150Logo.png")?;
    create_png(310, 150, "Wide310x150Logo.png")?;
    create_png(310, 310, "LargeTile.png")?;

    println!("\n⚠️  WARNING: These are PLACEHOLDER assets only!");
    println!("Replace with branded designs before Microsoft Store submission.");
    println!("See README.md for design guidelines.");

    Ok(())
}
