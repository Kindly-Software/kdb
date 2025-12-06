#!/bin/bash
#
# KDB Deployment Script
# Deploys kdb to Fly.io with health checks and monitoring
#
# Usage:
#   ./deploy.sh              # Deploy to production
#   ./deploy.sh --staging    # Deploy to staging
#   ./deploy.sh --logs       # View deployment logs
#
# Requirements:
#   - flyctl CLI installed (https://fly.io/docs/hands-on/install-flyctl/)
#   - Authenticated with: flyctl auth login
#

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
APP_NAME="${FLY_APP:-kdb-mcp-server}"
REGION="${FLY_REGION:-sjc}"
TIMEOUT="${DEPLOY_TIMEOUT:-300}"

# Functions
log_info() {
    echo -e "${BLUE}ℹ${NC} $*"
}

log_success() {
    echo -e "${GREEN}✅${NC} $*"
}

log_warn() {
    echo -e "${YELLOW}⚠${NC} $*"
}

log_error() {
    echo -e "${RED}❌${NC} $*"
}

check_prerequisites() {
    log_info "Checking prerequisites..."

    if ! command -v flyctl &> /dev/null; then
        log_error "flyctl CLI not found. Install from https://fly.io/docs/hands-on/install-flyctl/"
        exit 1
    fi

    log_success "flyctl version: $(flyctl version)"

    if ! flyctl auth whoami &> /dev/null; then
        log_error "Not authenticated with Fly.io. Run: flyctl auth login"
        exit 1
    fi

    log_success "Authenticated as: $(flyctl auth whoami)"
}

validate_docker() {
    log_info "Validating Docker image..."

    if ! command -v docker &> /dev/null; then
        log_warn "Docker not found, using Fly.io remote builder"
        return 0
    fi

    log_success "Docker available for local builds"
}

deploy_production() {
    log_info "Deploying KDB to production ($REGION)..."

    # Build and deploy
    if flyctl deploy \
        --remote-only \
        --region="$REGION" \
        --app="$APP_NAME" \
        --wait-timeout="$TIMEOUT" \
        2>&1 | tee /tmp/flyctl-deploy.log; then
        log_success "Deployment successful!"
    else
        log_error "Deployment failed. See logs above."
        exit 1
    fi
}

health_check() {
    log_info "Running health check..."

    # Wait for machine to be ready
    sleep 10

    # Get app URL
    local url="https://${APP_NAME}.fly.dev/health"
    log_info "Checking $url..."

    # Retry logic (max 30 attempts, 10s interval)
    local attempts=0
    local max_attempts=30

    while [ $attempts -lt $max_attempts ]; do
        attempts=$((attempts + 1))

        if curl -sf "$url" > /tmp/health.json 2>&1; then
            log_success "Health check passed!"
            cat /tmp/health.json | jq . || cat /tmp/health.json
            return 0
        fi

        if [ $attempts -lt $max_attempts ]; then
            log_warn "Health check attempt $attempts/$max_attempts failed, retrying..."
            sleep 10
        fi
    done

    log_error "Health check failed after $max_attempts attempts"
    return 1
}

metrics_endpoint() {
    log_info "Testing metrics endpoint..."

    local url="https://${APP_NAME}.fly.dev/metrics"

    if curl -sf "$url" > /tmp/metrics.txt 2>&1; then
        log_success "Metrics endpoint is healthy"
        head -20 /tmp/metrics.txt
    else
        log_warn "Metrics endpoint not responding (may not be implemented yet)"
    fi
}

show_deployment_info() {
    log_success "Deployment Complete!"
    echo ""
    echo "📊 App Details:"
    echo "  Name: $APP_NAME"
    echo "  URL: https://${APP_NAME}.fly.dev"
    echo "  Region: $REGION"
    echo ""
    echo "🔗 Useful Commands:"
    echo "  View logs:       flyctl logs --app=$APP_NAME"
    echo "  SSH into VM:     flyctl ssh console --app=$APP_NAME"
    echo "  Scale machines:  flyctl scale count 2 --app=$APP_NAME"
    echo "  Update config:   nano fly.toml && flyctl deploy"
    echo "  Restart app:     flyctl restart --app=$APP_NAME"
    echo ""
    echo "🎯 Test the debugger:"
    echo "  Health:  curl https://${APP_NAME}.fly.dev/health | jq"
    echo "  Metrics: curl https://${APP_NAME}.fly.dev/metrics"
}

show_logs() {
    log_info "Streaming logs from $APP_NAME..."
    flyctl logs --app="$APP_NAME"
}

cleanup_on_error() {
    local exit_code=$?
    if [ $exit_code -ne 0 ]; then
        log_error "Deployment failed with exit code $exit_code"
        log_warn "View full logs with: flyctl logs --app=$APP_NAME"
    fi
    exit $exit_code
}

# Main script
main() {
    trap cleanup_on_error EXIT

    # Handle command-line arguments
    case "${1:-}" in
        --staging)
            APP_NAME="kdb-mcp-server-staging"
            log_info "Using staging app: $APP_NAME"
            ;;
        --logs)
            show_logs
            exit 0
            ;;
        --help)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --staging    Deploy to staging environment"
            echo "  --logs       View deployment logs"
            echo "  --help       Show this help message"
            exit 0
            ;;
    esac

    echo ""
    log_info "KDB Deployment Script"
    echo ""

    check_prerequisites
    validate_docker
    deploy_production

    # Run health checks
    echo ""
    if health_check; then
        metrics_endpoint
        show_deployment_info
    else
        log_error "Deployment validation failed"
        exit 1
    fi
}

# Run main function
main "$@"
