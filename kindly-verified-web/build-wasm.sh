#!/bin/bash
# Build script for Kindly-Verified WASM with comprehensive optimizations

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}Kindly-Verified WASM Build${NC}"
echo -e "${BLUE}========================================${NC}"

# Step 1: Clean previous builds
echo -e "\n${YELLOW}Step 1: Cleaning previous builds...${NC}"
rm -rf dist target/wasm32-unknown-unknown/release/*.wasm target/wasm32-unknown-unknown/release/*.js

# Step 2: Build with release optimizations
echo -e "\n${YELLOW}Step 2: Building WASM release binary...${NC}"
cargo build --release --target wasm32-unknown-unknown --quiet

if [ $? -ne 0 ]; then
    echo -e "${RED}Build failed!${NC}"
    exit 1
fi

echo -e "${GREEN}✓ WASM build successful${NC}"

# Step 3: Check for trunk availability
if command -v trunk &> /dev/null; then
    echo -e "\n${YELLOW}Step 3: Bundling with trunk...${NC}"
    trunk build --release
else
    echo -e "\n${YELLOW}Step 3: Trunk not found, using cargo build output directly${NC}"
    mkdir -p dist
    cp target/wasm32-unknown-unknown/release/*.wasm dist/ 2>/dev/null || true
    cp target/wasm32-unknown-unknown/release/*.js dist/ 2>/dev/null || true
fi

# Step 4: Display size before optimization
echo -e "\n${YELLOW}Step 4: Bundle size before optimization:${NC}"
WASM_SIZE=$(du -b dist/*.wasm 2>/dev/null | awk '{print $1}' | head -1 || echo "0")
JS_SIZE=$(du -b dist/*.js 2>/dev/null | awk '{print $1}' | paste -sd+ - | bc || echo "0")

echo -e "WASM: $(numfmt --to=iec-i --suffix=B $WASM_SIZE 2>/dev/null || echo "$WASM_SIZE bytes")"
echo -e "JS:   $(numfmt --to=iec-i --suffix=B $JS_SIZE 2>/dev/null || echo "$JS_SIZE bytes")"

# Step 5: Optional wasm-opt optimization
if command -v wasm-opt &> /dev/null; then
    echo -e "\n${YELLOW}Step 5: Applying wasm-opt post-link optimization...${NC}"
    for wasm_file in dist/*.wasm; do
        if [ -f "$wasm_file" ]; then
            wasm-opt -Oz -o "${wasm_file}.opt" "$wasm_file"
            mv "${wasm_file}.opt" "$wasm_file"
            echo -e "${GREEN}✓ Optimized $wasm_file${NC}"
        fi
    done
else
    echo -e "\n${YELLOW}Step 5: wasm-opt not installed (optional, for additional optimization)${NC}"
    echo -e "  Install with: cargo install wasm-opt"
fi

# Step 6: Compression testing
echo -e "\n${YELLOW}Step 6: Testing compression methods...${NC}"

# Gzip compression
if command -v gzip &> /dev/null; then
    cp dist/*.wasm /tmp/kindly.wasm.orig 2>/dev/null || true
    gzip -9 -k -f dist/*.wasm 2>/dev/null || true
    GZIP_SIZE=$(du -b dist/*.wasm.gz 2>/dev/null | awk '{print $1}' | head -1 || echo "0")
    echo -e "Gzip:   $(numfmt --to=iec-i --suffix=B $GZIP_SIZE 2>/dev/null || echo "$GZIP_SIZE bytes")"
fi

# Brotli compression (best for WASM)
if command -v brotli &> /dev/null; then
    brotli -k -f dist/*.wasm 2>/dev/null || true
    BROTLI_SIZE=$(du -b dist/*.wasm.br 2>/dev/null | awk '{print $1}' | head -1 || echo "0")
    echo -e "Brotli: $(numfmt --to=iec-i --suffix=B $BROTLI_SIZE 2>/dev/null || echo "$BROTLI_SIZE bytes")"

    if [ "$BROTLI_SIZE" -lt 524288 ]; then
        echo -e "${GREEN}✓ Target <500KB achieved!${NC}"
    else
        echo -e "${YELLOW}⚠ Brotli size: $BROTLI_SIZE bytes (target: <524288 bytes)${NC}"
    fi
else
    echo -e "${YELLOW}Brotli not installed (recommended for compression)${NC}"
    echo -e "  Install with: apt-get install brotli"
fi

# Step 7: Final size verification
echo -e "\n${YELLOW}Step 7: Final bundle size analysis:${NC}"
ls -lh dist/ | grep -E "\.(wasm|js|br|gz)$" | awk '{printf "%-40s %10s\n", $9, $5}'

# Step 8: Summary
echo -e "\n${BLUE}========================================${NC}"
echo -e "${GREEN}✓ WASM build complete!${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""
echo -e "Bundle location: $(pwd)/dist"
echo -e "Ready for deployment to:"
echo -e "  - Fly.io: fly deploy"
echo -e "  - Static hosting: Copy dist/ to your server"
echo ""
