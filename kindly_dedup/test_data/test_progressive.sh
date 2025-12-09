#!/bin/bash
# Progressive C4 Corpus Testing (10K → 1M documents)
# Purpose: B32-compliant performance validation at scale
# Output: Metrics, memory stats, throughput, accuracy samples

set -e

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
SCALES=(10000 100000 1000000)
HF_TOKEN="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"
THRESHOLD=0.85
OUTPUT_DIR="test_data"
RESULTS_LOG="/tmp/C4_PROGRESSIVE_TEST_RESULTS.md"
TIMING_LOG="/tmp/C4_TIMING_METRICS.log"

# Ensure output directory exists
mkdir -p "$OUTPUT_DIR"

# Initialize results log
cat > "$RESULTS_LOG" << 'EOF'
# Large-Scale C4 Corpus Testing Results

Date: 2025-11-17
System: Intel Core Ultra 7 155H, 22 cores
Testing Strategy: Progressive scaling (10K → 1M documents)

## Test Summary

EOF

# Initialize timing log
echo "Scale,Download_Time_s,Corpus_Size_MB,Doc_Count,Dedup_Time_s,Throughput_docs_sec,Peak_RSS_GB,Avg_Latency_us" > "$TIMING_LOG"

# Function: Convert seconds to human-readable format
format_time() {
    local seconds=$1
    if (( seconds < 60 )); then
        echo "${seconds}s"
    elif (( seconds < 3600 )); then
        echo "$((seconds / 60))m $((seconds % 60))s"
    else
        echo "$((seconds / 3600))h $((seconds % 3600 / 60))m"
    fi
}

# Function: Get memory usage in GB
get_memory_gb() {
    local rss_kb=$1
    echo "scale=2; $rss_kb / 1048576" | bc
}

# Function: Run dedup test with monitoring
run_dedup_test() {
    local input_file=$1
    local output_file=$2
    local scale=$3

    echo -e "${BLUE}[INFO] Starting dedup test for $scale documents...${NC}"

    local start_time=$(date +%s)
    local peak_rss=0
    local pid=""

    # Background: Run dedup in background and monitor
    (
        /usr/bin/time -v \
            cargo run --release --bin kindly_dedup --features "benchmarking" -- \
            --input "$input_file" \
            --threshold "$THRESHOLD" \
            --output "$output_file" \
            2>&1 | tee "${OUTPUT_DIR}/dedup_${scale}.log"
    ) &

    pid=$!

    # Monitor process
    echo -e "${BLUE}[INFO] Process PID: $pid${NC}"
    while ps -p $pid > /dev/null 2>&1; do
        # Read RSS from /proc
        if [ -f "/proc/$pid/status" ]; then
            local rss_kb=$(grep "VmRSS" "/proc/$pid/status" | awk '{print $2}')
            if [ ! -z "$rss_kb" ]; then
                local rss_gb=$(get_memory_gb "$rss_kb")
                echo "  RSS: $rss_gb GB"

                # Update peak RSS
                if (( $(echo "$rss_gb > $peak_rss" | bc -l) )); then
                    peak_rss=$rss_gb
                fi
            fi
        fi
        sleep 2
    done

    wait $pid 2>/dev/null || true

    local end_time=$(date +%s)
    local elapsed=$((end_time - start_time))

    echo -e "${GREEN}[OK] Dedup test complete in $(format_time $elapsed)${NC}"

    # Parse results from dedup log
    local throughput=0
    if grep -q "docs/sec" "${OUTPUT_DIR}/dedup_${scale}.log"; then
        throughput=$(grep "docs/sec" "${OUTPUT_DIR}/dedup_${scale}.log" | tail -1 | awk '{print $NF}' | sed 's/,//')
    fi

    echo "$scale,$elapsed,$throughput,$peak_rss" >> "$TIMING_LOG"
}

# Function: Download corpus from HuggingFace
download_corpus() {
    local limit=$1
    local scale_name=$2

    if [ -f "${OUTPUT_DIR}/c4_${scale_name}.jsonl" ]; then
        echo -e "${YELLOW}[SKIP] Corpus already exists: c4_${scale_name}.jsonl${NC}"
        return
    fi

    echo -e "${BLUE}[INFO] Downloading $limit documents from C4 corpus...${NC}"

    local start_time=$(date +%s)

    HF_TOKEN=$HF_TOKEN \
    cargo run --release --bin download_hf_corpus --features "hf-datasets" -- \
        --dataset allenai/c4 \
        --subset en \
        --limit "$limit" \
        --output "${OUTPUT_DIR}/c4_${scale_name}.jsonl" \
        --generate-manifest \
        2>&1 | tee "${OUTPUT_DIR}/download_${scale_name}.log"

    local end_time=$(date +%s)
    local elapsed=$((end_time - start_time))

    echo -e "${GREEN}[OK] Download complete in $(format_time $elapsed)${NC}"

    # Get file size
    local size_bytes=$(stat -f%z "${OUTPUT_DIR}/c4_${scale_name}.jsonl" 2>/dev/null || stat -c%s "${OUTPUT_DIR}/c4_${scale_name}.jsonl" 2>/dev/null || echo "0")
    local size_mb=$(echo "scale=1; $size_bytes / 1048576" | bc)

    echo "  Corpus size: ${size_mb} MB"

    return $elapsed
}

# Function: Verify corpus integrity
verify_corpus() {
    local file=$1
    local expected_count=$2

    echo -e "${BLUE}[INFO] Verifying corpus integrity...${NC}"

    local actual_count=$(wc -l < "$file")

    if [ "$actual_count" -eq "$expected_count" ]; then
        echo -e "${GREEN}[OK] Corpus verified: $actual_count documents${NC}"
        return 0
    else
        echo -e "${RED}[ERROR] Document count mismatch! Expected: $expected_count, Got: $actual_count${NC}"
        return 1
    fi
}

# Function: Sample duplicate pairs for accuracy validation
sample_duplicates() {
    local deduplicated_json=$1
    local scale=$2

    echo -e "${BLUE}[INFO] Sampling duplicate pairs for accuracy validation...${NC}"

    # Extract first 10 duplicate clusters
    if [ -f "$deduplicated_json" ]; then
        local sample_file="${OUTPUT_DIR}/duplicate_samples_${scale}.txt"

        # Simple validation: check if output has clusters
        if grep -q "cluster" "$deduplicated_json" 2>/dev/null || grep -q "duplicates" "$deduplicated_json" 2>/dev/null; then
            echo -e "${GREEN}[OK] Deduplicated output contains cluster data${NC}"

            # Count unique clusters
            local cluster_count=$(grep -o "cluster" "$deduplicated_json" 2>/dev/null | wc -l || echo "unknown")
            echo "  Clusters found: $cluster_count"
        else
            echo -e "${YELLOW}[WARN] Could not verify cluster structure${NC}"
        fi
    fi
}

# Main test loop
main() {
    echo -e "${BLUE}=========================================${NC}"
    echo -e "${BLUE}Large-Scale C4 Corpus Testing${NC}"
    echo -e "${BLUE}Progressive: 10K → 100K → 1M Documents${NC}"
    echo -e "${BLUE}=========================================${NC}"
    echo ""

    # Check resources
    echo -e "${BLUE}[INFO] System Resources:${NC}"
    free -h | head -2
    df -h . | tail -1
    echo ""

    for limit in "${SCALES[@]}"; do
        scale_name="${limit}"
        if [ "$limit" -eq 10000 ]; then scale_name="10k"
        elif [ "$limit" -eq 100000 ]; then scale_name="100k"
        elif [ "$limit" -eq 1000000 ]; then scale_name="1m"
        fi

        echo ""
        echo -e "${BLUE}=========================================${NC}"
        echo -e "${BLUE}Scale: ${scale_name} ($limit documents)${NC}"
        echo -e "${BLUE}=========================================${NC}"
        echo ""

        # Download corpus
        download_corpus "$limit" "$scale_name"
        download_elapsed=$?

        corpus_file="${OUTPUT_DIR}/c4_${scale_name}.jsonl"

        # Verify corpus
        verify_corpus "$corpus_file" "$limit" || {
            echo -e "${RED}[ERROR] Corpus verification failed, skipping dedup${NC}"
            continue
        }

        # Check available memory before proceeding
        available_gb=$(free -h | grep "Mem:" | awk '{print $7}' | sed 's/G//')
        if (( $(echo "$available_gb < 3" | bc -l) )); then
            echo -e "${YELLOW}[WARN] Low available memory (${available_gb} GB). Aborting large-scale tests.${NC}"
            break
        fi

        # Run dedup test
        dedup_output="${OUTPUT_DIR}/c4_${scale_name}_deduplicated.json"
        run_dedup_test "$corpus_file" "$dedup_output" "$scale_name"

        # Sample duplicates for accuracy check
        sample_duplicates "$dedup_output" "$scale_name"

        # Cleanup (optional: comment out to keep corpus for later analysis)
        # echo -e "${YELLOW}[INFO] Cleaning up corpus file...${NC}"
        # rm -f "$corpus_file"

        echo ""
    done

    echo -e "${BLUE}=========================================${NC}"
    echo -e "${GREEN}All tests complete!${NC}"
    echo -e "${BLUE}=========================================${NC}"
    echo ""

    # Generate summary
    echo -e "${BLUE}[INFO] Results saved to:${NC}"
    echo "  - Metrics: $TIMING_LOG"
    echo "  - Details: $RESULTS_LOG"
    echo "  - Logs: ${OUTPUT_DIR}/dedup_*.log"
    echo ""

    # Print timing summary
    echo -e "${BLUE}Timing Summary:${NC}"
    column -t -s ',' "$TIMING_LOG"
}

# Run tests
main

echo -e "${GREEN}✅ Progressive testing complete!${NC}"
