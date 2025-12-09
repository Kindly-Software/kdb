// Test actual download flow to isolate issue
use std::io::{BufReader, BufRead, Read};
use flate2::read::GzDecoder;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    
    let url = "https://data.commoncrawl.org/crawl-data/CC-MAIN-2024-33/segments/1722640353668.0/wet/CC-MAIN-20240802234508-20240803024508-00000.warc.wet.gz";
    
    println!("Downloading {}...", url);
    let response = client.get(url).send().await?;
    let bytes = response.bytes().await?;
    
    println!("Downloaded {} bytes (gzipped)", bytes.len());
    
    // Decompress
    let decoder = GzDecoder::new(&bytes[..]);
    let mut reader = BufReader::with_capacity(64 * 1024, decoder);
    
    let mut line = String::new();
    let mut line_count = 0;
    let mut conversion_count = 0;
    
    while line_count < 100 {
        line.clear();
        let bytes_read = reader.read_line(&mut line)?;
        if bytes_read == 0 { break; }
        
        if line.trim().starts_with("WARC-Type: conversion") {
            conversion_count += 1;
            println!("Found conversion record #{}", conversion_count);
        }
        
        line_count += 1;
    }
    
    println!("\nTotal lines read: {}", line_count);
    println!("Conversion records found: {}", conversion_count);
    
    Ok(())
}
