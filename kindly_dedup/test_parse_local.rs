// Test WET parser locally
use std::fs::File;
use std::io::{BufReader, BufRead, Read};

fn main() -> std::io::Result<()> {
    // Download and save test file
    let output = std::process::Command::new("bash")
        .args(&["-c", "curl -s 'https://data.commoncrawl.org/crawl-data/CC-MAIN-2024-33/segments/1722640353668.0/wet/CC-MAIN-20240802234508-20240803024508-00000.warc.wet.gz' | zcat > /tmp/test_wet_full.txt"])
        .output()?;
    
    if !output.status.success() {
        eprintln!("Failed to download: {}", String::from_utf8_lossy(&output.stderr));
        return Ok(());
    }
    
    // Parse file
    let file = File::open("/tmp/test_wet_full.txt")?;
    let mut reader = BufReader::new(file);
    
    let mut doc_count = 0;
    let mut record_count = 0;
    let mut line = String::new();
    
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        
        if line.trim().starts_with("WARC/") {
            record_count += 1;
        }
        
        if line.trim().starts_with("WARC-Type: conversion") {
            // Found conversion, now find Content-Length
            let mut content_len = 0;
            let mut url = String::new();
            
            loop {
                line.clear();
                if reader.read_line(&mut line)? == 0 { break; }
                let trimmed = line.trim();
                if trimmed.is_empty() { break; }
                
                if trimmed.starts_with("Content-Length:") {
                    content_len = trimmed.split(':').nth(1)
                        .and_then(|s| s.trim().parse().ok())
                        .unwrap_or(0);
                }
                if trimmed.starts_with("WARC-Target-URI:") {
                    url = trimmed.split(':').skip(1).collect::<Vec<_>>().join(":").trim().to_string();
                }
            }
            
            // Read content
            if content_len > 0 {
                let mut content = vec![0u8; content_len];
                if reader.read_exact(&mut content).is_ok() {
                    let text = String::from_utf8_lossy(&content);
                    let trimmed_text = text.trim();
                    
                    if trimmed_text.len() >= 100 {
                        doc_count += 1;
                        println!("Doc {}: {} ({} chars)", doc_count, url, trimmed_text.len());
                        
                        if doc_count >= 10 {
                            break;
                        }
                    }
                }
            }
        }
    }
    
    println!("\nTotal records: {}", record_count);
    println!("Documents extracted (≥100 chars): {}", doc_count);
    
    Ok(())
}
