# [TRADE SECRET] Stripe webhook handler - prebuilt binary image
# Using ubuntu which has better compatibility with pre-built binaries

FROM ubuntu:22.04

# Install minimal runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libgcc-s1 \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy prebuilt binary (built locally)
COPY target/release/stripe_webhook .

# Make binary executable
RUN chmod +x /app/stripe_webhook

# Expose port
EXPOSE 3000

# Set environment
ENV APP_PORT=3000
ENV RUST_LOG=info

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=10s --retries=5 \
    CMD curl -f http://localhost:3000/health || exit 1

# Run webhook handler
CMD ["./stripe_webhook"]
