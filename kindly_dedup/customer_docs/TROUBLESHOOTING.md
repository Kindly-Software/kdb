# Troubleshooting Guide

Common issues and solutions for Kindly Dedup.

## Performance Issues

### Slow Processing Speed

**Symptom**: Throughput significantly below expected (< 10,000 docs/sec)

**Causes and Solutions**:

1. **Insufficient threads**:
   ```bash
   # Check current thread count
   kindly-dedup deduplicate --input data.jsonl --output results.json --threads $(nproc)
   ```

2. **Small batch size**:
   ```bash
   # Increase batch size
   kindly-dedup deduplicate --input data.jsonl --output results.json --batch-size 5000
   ```

3. **Slow storage (HDD instead of SSD)**:
   - Move data to SSD
   - Use persistent mode to reduce I/O

4. **Resource contention**:
   ```bash
   # Check system load
   top
   # Look for high CPU/memory usage from other processes
   ```

5. **Input format overhead**:
   - Use JSONL instead of CSV (2-3× faster parsing)
   - Pre-validate input files for corruption

**Expected Performance**:
- Single-threaded: 50,000-60,000 docs/sec
- 8 threads: 200,000-300,000 docs/sec
- 16 threads: 300,000-400,000 docs/sec

### High Memory Usage

**Symptom**: Process using more RAM than expected

**Solutions**:

1. **Enable persistent mode**:
   ```bash
   kindly-dedup deduplicate \
     --input data.jsonl \
     --output results.json \
     --persistent \
     --storage-path ./storage
   ```
   This reduces RAM usage by 93% (3.5 GB vs 40 GB for 10M docs).

2. **Reduce batch size**:
   ```bash
   kindly-dedup deduplicate --input data.jsonl --output results.json --batch-size 500
   ```

3. **Process in stages**:
   ```bash
   # Split large file into smaller chunks
   split -l 100000 large_corpus.jsonl chunk_

   # Process each chunk
   for file in chunk_*; do
     kindly-dedup deduplicate --input $file --output ${file}.json --persistent
   done
   ```

**Memory Guidelines**:
- 100K docs: 2 GB RAM
- 1M docs: 4 GB RAM
- 10M docs: 8 GB RAM (standard) or 4 GB (persistent)
- 100M+ docs: Use persistent mode (4-8 GB RAM regardless of size)

### GPU Not Detected

**Symptom**: `--gpu` flag has no effect, using CPU instead

**Solutions**:

1. **Verify GPU is available**:
   ```bash
   # NVIDIA
   nvidia-smi

   # AMD
   rocm-smi

   # Intel
   xpu-smi
   ```

2. **Check GPU drivers**:
   - NVIDIA: Install CUDA 11.0+ or newer
   - AMD: Install ROCm 5.0+ or newer
   - Intel: Install oneAPI drivers

3. **Verify GPU feature is enabled**:
   ```bash
   kindly-dedup --version
   # Should show: "GPU support: enabled"
   ```

4. **Fallback to CPU**:
   If GPU unavailable, CPU performance is still excellent (60,000+ docs/sec).

## Memory Errors

### Out of Memory (OOM)

**Error**: `Error: Out of memory` or process killed by OS

**Solutions**:

1. **Use persistent mode** (RECOMMENDED):
   ```bash
   kindly-dedup deduplicate \
     --input data.jsonl \
     --output results.json \
     --persistent \
     --storage-path ./storage
   ```

2. **Reduce batch size**:
   ```bash
   kindly-dedup deduplicate --input data.jsonl --output results.json --batch-size 100
   ```

3. **Increase system swap**:
   ```bash
   # Linux: Add 16 GB swap
   sudo fallocate -l 16G /swapfile
   sudo chmod 600 /swapfile
   sudo mkswap /swapfile
   sudo swapon /swapfile
   ```

4. **Process incrementally**:
   Split dataset into smaller files and process separately.

### Memory Leak Detection

**Symptom**: Memory usage grows over time

**Solutions**:

1. **Monitor memory**:
   ```bash
   # Run with monitoring
   watch -n 1 'ps aux | grep kindly-dedup'
   ```

2. **Restart service periodically**:
   For API server deployments, schedule daily restarts:
   ```bash
   # Systemd timer (daily restart at 3 AM)
   sudo systemctl restart kindly-dedup.service
   ```

3. **Update to latest version**:
   Memory leaks are rare but fixed promptly in new releases.

## Input/Output Errors

### Invalid Input Format

**Error**: `Error: Invalid JSON on line 123` or `Error: CSV parse error`

**Solutions**:

1. **Validate input file**:
   ```bash
   # For JSONL
   jq -c . input.jsonl > validated.jsonl

   # For CSV
   csvlint input.csv
   ```

2. **Check encoding**:
   ```bash
   # Convert to UTF-8
   iconv -f ISO-8859-1 -t UTF-8 input.txt > input_utf8.txt
   ```

3. **Remove corrupted lines**:
   ```bash
   # Skip invalid JSON lines
   grep '^{.*}$' input.jsonl > clean_input.jsonl
   ```

### File Not Found

**Error**: `Error: No such file or directory: data.jsonl`

**Solutions**:

1. **Use absolute paths**:
   ```bash
   kindly-dedup deduplicate --input /full/path/to/data.jsonl --output /full/path/to/results.json
   ```

2. **Verify file permissions**:
   ```bash
   ls -la data.jsonl
   chmod 644 data.jsonl
   ```

### Output File Cannot Be Written

**Error**: `Error: Permission denied writing to results.json`

**Solutions**:

1. **Check directory permissions**:
   ```bash
   chmod 755 $(dirname results.json)
   ```

2. **Verify disk space**:
   ```bash
   df -h
   ```

3. **Use different output path**:
   ```bash
   kindly-dedup deduplicate --input data.jsonl --output /tmp/results.json
   ```

## API Server Issues

### Server Won't Start

**Error**: `Error: Address already in use`

**Solutions**:

1. **Check if port is in use**:
   ```bash
   lsof -i :8080
   ```

2. **Use different port**:
   ```bash
   kindly-dedup serve --port 8081
   ```

3. **Kill existing process**:
   ```bash
   pkill -f kindly-dedup
   kindly-dedup serve
   ```

### API Requests Timeout

**Symptom**: HTTP requests hang or timeout

**Solutions**:

1. **Check server health**:
   ```bash
   curl http://localhost:8080/health
   ```

2. **Increase timeout**:
   ```bash
   # Client-side
   curl --max-time 300 -X POST http://localhost:8080/api/v1/deduplicate
   ```

3. **Use batch API**:
   For large requests, use `/api/v1/deduplicate/batch` instead of synchronous endpoint.

4. **Check server logs**:
   ```bash
   journalctl -u kindly-dedup -n 100 -f
   ```

### Rate Limit Errors

**Error**: `HTTP 429: Too Many Requests`

**Solutions**:

1. **Check rate limit headers**:
   ```bash
   curl -I http://localhost:8080/api/v1/deduplicate
   # Look for X-RateLimit-Remaining
   ```

2. **Upgrade license**:
   Contact sales@kindly.ai for higher rate limits.

3. **Implement client-side backoff**:
   ```python
   import time

   def api_call_with_retry(client, data, max_retries=5):
       for i in range(max_retries):
           try:
               return client.deduplicate(data)
           except RateLimitError:
               time.sleep(2 ** i)  # Exponential backoff
   ```

## License Issues

### License Invalid

**Error**: `Error: Invalid license key`

**Solutions**:

1. **Verify license key**:
   ```bash
   echo $KINDLY_DEDUP_LICENSE
   ```

2. **Re-activate license**:
   ```bash
   export KINDLY_DEDUP_LICENSE="your-correct-license-key"
   kindly-dedup --version
   ```

3. **Check license expiration**:
   ```bash
   kindly-dedup --license-info
   ```

4. **Contact support**:
   Email support@kindly.ai with your license key.

### License Expired

**Error**: `Error: License expired on 2025-11-01`

**Solutions**:

1. **Renew license**:
   Contact sales@kindly.ai for renewal.

2. **Temporary workaround** (limited performance):
   ```bash
   # CLI continues to work with reduced throughput
   kindly-dedup deduplicate --input data.jsonl --output results.json
   ```

3. **Grace period**:
   30-day grace period after expiration (warnings only).

## Data Quality Issues

### Too Many False Positives

**Symptom**: Many non-duplicate documents flagged as duplicates

**Solutions**:

1. **Increase threshold**:
   ```bash
   # Try 0.90 or 0.95 instead of 0.85
   kindly-dedup deduplicate --input data.jsonl --output results.json --threshold 0.90
   ```

2. **Check document length**:
   Very short documents (< 50 characters) have higher false positive rates.

3. **Review results manually**:
   Sample 100 clusters and check accuracy.

### Too Many False Negatives

**Symptom**: Known duplicates not detected

**Solutions**:

1. **Lower threshold**:
   ```bash
   # Try 0.80 instead of 0.85
   kindly-dedup deduplicate --input data.jsonl --output results.json --threshold 0.80
   ```

2. **Check text normalization**:
   Ensure documents are properly cleaned (lowercase, punctuation, etc.).

3. **Verify input format**:
   Ensure all text is in the `text` field, not metadata.

## System Issues

### Disk Space Full

**Error**: `Error: No space left on device`

**Solutions**:

1. **Check disk usage**:
   ```bash
   df -h
   du -sh /var/lib/kindly-dedup/*
   ```

2. **Clean old data**:
   ```bash
   rm -rf /var/lib/kindly-dedup/old_storage
   ```

3. **Use compression** (persistent mode):
   Storage automatically compressed, but verify:
   ```bash
   ls -lh /var/lib/kindly-dedup/
   ```

### Permission Denied

**Error**: `Error: Permission denied`

**Solutions**:

1. **Fix ownership**:
   ```bash
   sudo chown -R $USER:$USER /var/lib/kindly-dedup
   ```

2. **Fix permissions**:
   ```bash
   chmod 755 /var/lib/kindly-dedup
   chmod 644 /var/lib/kindly-dedup/*
   ```

3. **Run with correct user**:
   ```bash
   # If using systemd
   sudo systemctl edit kindly-dedup.service
   # Set User=your-username
   ```

## Crash Recovery

### Process Crashed Mid-Job

**Symptom**: Process terminated unexpectedly, partial results

**Solutions**:

1. **Use persistent mode** (automatic recovery):
   ```bash
   kindly-dedup deduplicate \
     --input data.jsonl \
     --output results.json \
     --persistent \
     --storage-path ./storage

   # Re-run same command - automatically resumes
   ```

2. **Check crash logs**:
   ```bash
   dmesg | grep kindly-dedup
   journalctl -u kindly-dedup -n 1000
   ```

3. **Resume from checkpoint**:
   Persistent mode automatically checkpoints every 10,000 documents.

## Getting Help

If issues persist:

1. **Collect diagnostics**:
   ```bash
   kindly-dedup --version
   uname -a
   free -h
   df -h
   ```

2. **Generate support bundle**:
   ```bash
   kindly-dedup support-bundle --output support.tar.gz
   ```

3. **Contact support**:
   - Email: support@kindly.ai
   - Include: Error message, logs, support bundle
   - Enterprise customers: enterprise@kindly.ai (priority support)

4. **Community resources**:
   - GitHub Issues: https://github.com/kindly-ai/kindly-dedup/issues
   - Forum: https://community.kindly.ai
   - Documentation: https://docs.kindly.ai
