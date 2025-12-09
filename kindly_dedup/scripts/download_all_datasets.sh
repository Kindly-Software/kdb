#!/usr/bin/env bash
# Download All Realistic Datasets
#
# Downloads industry-standard LLM training datasets for kindly_dedup benchmarking
# with B32 provenance tracking (SHA-256 manifests)
#
# Usage:
#   ./scripts/download_all_datasets.sh          # Download all datasets (default sizes)
#   ./scripts/download_all_datasets.sh small    # Small datasets (100K each, ~5GB total)
#   ./scripts/download_all_datasets.sh large    # Large datasets (10M each, ~500GB total)

set -euo pipefail

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
OUTPUT_DIR="$PROJECT_DIR/test_data/realistic"

# Dataset sizes
DATASET_SIZE="${1:-medium}"

case "$DATASET_SIZE" in
    small)
        LIMIT_100K=100000
        LIMIT_1M=100000
        LIMIT_10M=100000
        echo -e "${YELLOW}Using SMALL dataset sizes (100K each, ~5GB total)${NC}"
        ;;
    medium)
        LIMIT_100K=100000
        LIMIT_1M=1000000
        LIMIT_10M=1000000
        echo -e "${YELLOW}Using MEDIUM dataset sizes (1M each, ~50GB total)${NC}"
        ;;
    large)
        LIMIT_100K=100000
        LIMIT_1M=1000000
        LIMIT_10M=10000000
        echo -e "${YELLOW}Using LARGE dataset sizes (10M each, ~500GB total)${NC}"
        ;;
    *)
        echo -e "${RED}Error: Unknown size '$DATASET_SIZE'${NC}"
        echo "Usage: $0 [small|medium|large]"
        exit 1
        ;;
esac

echo "========================================================================"
echo "Realistic Dataset Downloader for kindly_dedup"
echo "========================================================================"
echo "Output directory: $OUTPUT_DIR"
echo

# Create output directory
mkdir -p "$OUTPUT_DIR"

# Check if cargo is available
if ! command -v cargo &> /dev/null; then
    echo -e "${RED}Error: cargo not found. Please install Rust.${NC}"
    exit 1
fi

# Check if download_corpus binary exists
echo "Building download_corpus binary..."
cd "$PROJECT_DIR"
if ! cargo build --bin download_corpus --features download-tools --release; then
    echo -e "${RED}Error: Failed to build download_corpus binary${NC}"
    exit 1
fi

DOWNLOAD_BIN="$PROJECT_DIR/target/release/download_corpus"

if [ ! -f "$DOWNLOAD_BIN" ]; then
    echo -e "${RED}Error: download_corpus binary not found at $DOWNLOAD_BIN${NC}"
    exit 1
fi

echo -e "${GREEN}✓ Build complete${NC}"
echo

# ============================================================================
# Dataset 1: Common Crawl (100K)
# ============================================================================
echo "========================================================================"
echo "Dataset 1/4: Common Crawl (100K documents)"
echo "========================================================================"

OUTPUT_FILE_100K="$OUTPUT_DIR/commoncrawl_100k.json"

if [ -f "$OUTPUT_FILE_100K" ]; then
    echo -e "${YELLOW}File already exists: $OUTPUT_FILE_100K${NC}"
    echo "Skipping download. Delete file to re-download."
else
    echo "Downloading Common Crawl (100K documents)..."
    "$DOWNLOAD_BIN" \
        --source commoncrawl \
        --limit "$LIMIT_100K" \
        --output "$OUTPUT_FILE_100K" \
        --generate-manifest

    if [ $? -eq 0 ]; then
        echo -e "${GREEN}✓ Common Crawl 100K download complete${NC}"
    else
        echo -e "${RED}✗ Common Crawl 100K download failed${NC}"
        exit 1
    fi
fi
echo

# ============================================================================
# Dataset 2: Common Crawl (1M)
# ============================================================================
echo "========================================================================"
echo "Dataset 2/4: Common Crawl (1M documents)"
echo "========================================================================"

OUTPUT_FILE_1M="$OUTPUT_DIR/commoncrawl_1m.json"

if [ -f "$OUTPUT_FILE_1M" ]; then
    echo -e "${YELLOW}File already exists: $OUTPUT_FILE_1M${NC}"
    echo "Skipping download. Delete file to re-download."
else
    echo "Downloading Common Crawl (1M documents)..."
    echo -e "${YELLOW}Warning: This may take 30-60 minutes${NC}"
    "$DOWNLOAD_BIN" \
        --source commoncrawl \
        --limit "$LIMIT_1M" \
        --output "$OUTPUT_FILE_1M" \
        --generate-manifest

    if [ $? -eq 0 ]; then
        echo -e "${GREEN}✓ Common Crawl 1M download complete${NC}"
    else
        echo -e "${RED}✗ Common Crawl 1M download failed${NC}"
        exit 1
    fi
fi
echo

# ============================================================================
# Dataset 3: Common Crawl (10M) - Large stress test
# ============================================================================
if [ "$DATASET_SIZE" == "large" ]; then
    echo "========================================================================"
    echo "Dataset 3/4: Common Crawl (10M documents)"
    echo "========================================================================"

    OUTPUT_FILE_10M="$OUTPUT_DIR/commoncrawl_10m.json"

    if [ -f "$OUTPUT_FILE_10M" ]; then
        echo -e "${YELLOW}File already exists: $OUTPUT_FILE_10M${NC}"
        echo "Skipping download. Delete file to re-download."
    else
        echo "Downloading Common Crawl (10M documents)..."
        echo -e "${YELLOW}Warning: This may take 6-12 hours${NC}"
        "$DOWNLOAD_BIN" \
            --source commoncrawl \
            --limit "$LIMIT_10M" \
            --output "$OUTPUT_FILE_10M" \
            --generate-manifest

        if [ $? -eq 0 ]; then
            echo -e "${GREEN}✓ Common Crawl 10M download complete${NC}"
        else
            echo -e "${RED}✗ Common Crawl 10M download failed${NC}"
            exit 1
        fi
    fi
    echo
else
    echo "========================================================================"
    echo "Dataset 3/4: Common Crawl (10M documents) - SKIPPED"
    echo "========================================================================"
    echo -e "${YELLOW}Use './scripts/download_all_datasets.sh large' to download 10M dataset${NC}"
    echo
fi

# ============================================================================
# Dataset 4: Future datasets (Pile, C4, RedPajama)
# ============================================================================
echo "========================================================================"
echo "Dataset 4/4: Additional Datasets (Pile, C4, RedPajama)"
echo "========================================================================"
echo -e "${YELLOW}Not yet implemented. Currently only Common Crawl is supported.${NC}"
echo
echo "Planned datasets:"
echo "  - The Pile (EleutherAI): https://pile.eleuther.ai/"
echo "  - C4: https://huggingface.co/datasets/allenai/c4"
echo "  - RedPajama: https://huggingface.co/datasets/togethercomputer/RedPajama-Data-1T"
echo

# ============================================================================
# Summary
# ============================================================================
echo "========================================================================"
echo "Download Summary"
echo "========================================================================"
echo

# Count downloaded files
count=0
total_size=0

for file in "$OUTPUT_DIR"/*.json; do
    if [ -f "$file" ]; then
        count=$((count + 1))
        size=$(stat -c%s "$file" 2>/dev/null || stat -f%z "$file" 2>/dev/null || echo 0)
        total_size=$((total_size + size))
        echo -e "${GREEN}✓${NC} $(basename "$file") ($(numfmt --to=iec-i --suffix=B $size 2>/dev/null || echo "$size bytes"))"
    fi
done

echo
echo "Total datasets: $count"
echo "Total storage: $(numfmt --to=iec-i --suffix=B $total_size 2>/dev/null || echo "$total_size bytes")"
echo

# Verify manifests
echo "Verifying manifests..."
manifest_count=0
for file in "$OUTPUT_DIR"/*.manifest.json; do
    if [ -f "$file" ]; then
        manifest_count=$((manifest_count + 1))
        echo -e "${GREEN}✓${NC} $(basename "$file")"
    fi
done
echo "Total manifests: $manifest_count"
echo

# Final message
if [ $count -gt 0 ]; then
    echo -e "${GREEN}✓ Dataset download complete!${NC}"
    echo
    echo "Next steps:"
    echo "  1. Verify integrity: cargo test --test dataset_manager_tests --features download-tools"
    echo "  2. Run benchmarks: cargo bench --features download-tools"
    echo "  3. Run deduplication: cargo run --example realistic_benchmark --features download-tools"
    exit 0
else
    echo -e "${RED}✗ No datasets downloaded${NC}"
    exit 1
fi
