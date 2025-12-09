#!/bin/bash
# Download first WET file and parse locally
curl -s "https://data.commoncrawl.org/crawl-data/CC-MAIN-2024-33/segments/1722640353668.0/wet/CC-MAIN-20240802234508-20240803024508-00000.warc.wet.gz" | zcat | head -500 > /tmp/test_wet.txt

echo "=== Sample WET content (first 100 lines) ==="
head -100 /tmp/test_wet.txt

echo ""
echo "=== Counting records ==="
grep -c "^WARC/1.0" /tmp/test_wet.txt

echo ""
echo "=== Conversion records ==="
grep -c "WARC-Type: conversion" /tmp/test_wet.txt

echo ""
echo "=== Sample conversion record with content length ==="
grep -A10 "WARC-Type: conversion" /tmp/test_wet.txt | head -15
