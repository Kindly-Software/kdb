# Fly.io Setup for kindly_dedup Landing Page Deployment

## ✅ Installation Complete

The fly.io CLI (`flyctl`) has been successfully installed and configured.

### Installation Details
- **Version**: flyctl v0.3.206 linux/amd64
- **Location**: `/home/samuel/.fly/bin/flyctl`
- **PATH**: Added to `~/.bashrc` (permanent)
- **Build Date**: 2025-10-30

### Verify Installation
```bash
flyctl version
# Output: flyctl v0.3.206 linux/amd64 Commit: f40ad4c3e63b5f7d33f51217c48995d78796e3d1 BuildDate: 2025-10-30T08:29:38Z
```

---

## ⚠️ Authentication Required

You need to log in to your Fly.io account before deploying.

### Login to Fly.io
```bash
flyctl auth login
```

This will:
1. Open a browser window for authentication
2. Prompt you to sign in with your Fly.io account
3. Store the access token in `~/.fly/config.yml`

### Verify Authentication
```bash
flyctl auth whoami
# Should display your Fly.io email/username
```

### Check Organizations
```bash
flyctl orgs list
# Lists all organizations you have access to
```

---

## 🚀 Deploying kindly_dedup Landing Page

Once authenticated, you can deploy the landing page to Fly.io.

### Prerequisites
1. ✅ flyctl installed (done)
2. ⏳ Authenticated (need to run `flyctl auth login`)
3. ✅ WASM bundle built (`kindly-web/dist/`)

### Deployment Steps

#### 1. Create Fly.io App Configuration

Create `fly.toml` in `/home/samuel/Primitives/kindly-web/`:

```toml
# fly.toml - kindly_dedup landing page
app = "kindly-dedup"
primary_region = "iad"  # Washington, D.C. (or choose closest region)

[build]
  # Use static file serving with nginx
  builtin = "static"

[http_service]
  internal_port = 8080
  force_https = true
  auto_stop_machines = "stop"
  auto_start_machines = true
  min_machines_running = 0

[[http_service.static]]
  guest_path = "/usr/share/nginx/html"
  url_prefix = "/"

[mounts]
  source = "dist"
  destination = "/usr/share/nginx/html"
```

#### 2. Create Dockerfile (Alternative: Static Build)

If using static build, create `Dockerfile`:

```dockerfile
FROM nginx:alpine

# Copy WASM bundle to nginx html directory
COPY dist/ /usr/share/nginx/html/

# Configure nginx for WASM MIME types
RUN echo 'types { application/wasm wasm; }' > /etc/nginx/conf.d/wasm.conf

# Expose port 8080
EXPOSE 8080

# Start nginx
CMD ["nginx", "-g", "daemon off;"]
```

#### 3. Initialize Fly.io App

```bash
cd /home/samuel/Primitives/kindly-web

# Launch interactive setup
flyctl launch

# Or non-interactive:
flyctl launch \
  --name kindly-dedup \
  --region iad \
  --no-deploy
```

This will:
- Create `fly.toml` if it doesn't exist
- Set up the app on Fly.io
- Configure default settings

#### 4. Deploy the Application

```bash
# Deploy the WASM bundle
flyctl deploy

# Monitor deployment
flyctl status

# Check logs
flyctl logs
```

#### 5. Verify Deployment

```bash
# Get app URL
flyctl info

# Expected output:
# Name     = kindly-dedup
# Owner    = [your-org]
# Hostname = kindly-dedup.fly.dev
# ...
```

Visit: https://kindly-dedup.fly.dev

---

## 🌍 Custom Domain Setup (Optional)

Once deployed, you can configure a custom domain like `kindly.software` or `dedup.kindly.software`.

### Add Custom Domain

```bash
# Add domain to Fly.io app
flyctl certs create kindly.software

# Or subdomain
flyctl certs create dedup.kindly.software
```

### Configure DNS

Add CNAME or AAAA records to your DNS provider:

**Option 1: CNAME (Recommended for subdomains)**
```
dedup.kindly.software. CNAME kindly-dedup.fly.dev.
```

**Option 2: AAAA (For root domain)**
```bash
# Get Fly.io IPv6 addresses
flyctl ips list

# Add AAAA records to DNS:
kindly.software. AAAA [ipv6-address-1]
kindly.software. AAAA [ipv6-address-2]
```

### Verify Certificate

```bash
flyctl certs show kindly.software
# Should show: Status = Ready
```

---

## 📊 Fly.io Configuration Options

### Scaling

```bash
# Scale to specific number of machines
flyctl scale count 2

# Scale by region
flyctl regions set iad ord
```

### Machine Types

```bash
# Use smallest machine for static site (cheap!)
flyctl machine update --vm-size shared-cpu-1x

# List available machine types
flyctl platform vm-sizes
```

### Environment Variables (if needed)

```bash
# Set environment variables
flyctl secrets set API_KEY=your-key-here

# List secrets
flyctl secrets list
```

---

## 💰 Cost Estimation

Fly.io pricing for static WASM landing page:

**Shared CPU (Recommended for Landing Page)**:
- **shared-cpu-1x**: 256 MB RAM, 1 shared CPU
- **Cost**: ~$1.94/month (if running 24/7)
- **Free tier**: 3 shared-cpu-1x machines included in free plan

**With Auto-Stop** (recommended):
- Auto-stop machines when no traffic
- Auto-start on request
- **Cost**: Near $0 for low-traffic sites (free tier covers it)

**Bandwidth**:
- 100 GB/month included (free)
- Bundle size: ~160 KB = ~625,000 page loads in free tier

---

## 🔧 Useful Commands

### App Management
```bash
# List all apps
flyctl apps list

# Open app dashboard
flyctl dashboard

# Destroy app (careful!)
flyctl apps destroy kindly-dedup
```

### Deployment
```bash
# Deploy specific directory
flyctl deploy --local-only

# Deploy with build args
flyctl deploy --build-arg VERSION=1.0.0

# Rollback to previous deployment
flyctl releases rollback
```

### Monitoring
```bash
# Stream logs
flyctl logs

# Check app status
flyctl status

# Check machine metrics
flyctl machine list
```

### Debugging
```bash
# SSH into machine
flyctl ssh console

# Run command in machine
flyctl ssh console -C "ls -la /usr/share/nginx/html"

# Check machine health
flyctl checks list
```

---

## 📝 Recommended fly.toml for kindly_dedup

Here's a production-ready configuration:

```toml
# fly.toml - kindly_dedup landing page (production)
app = "kindly-dedup"
primary_region = "iad"  # Washington, D.C.

# Build configuration
[build]
  dockerfile = "Dockerfile"

# HTTP service configuration
[http_service]
  internal_port = 8080
  force_https = true
  auto_stop_machines = "suspend"  # Auto-stop when idle
  auto_start_machines = true      # Auto-start on request
  min_machines_running = 0        # Scale to zero when idle

  # Health checks
  [http_service.concurrency]
    type = "requests"
    soft_limit = 200
    hard_limit = 250

# Machine configuration
[[vm]]
  size = "shared-cpu-1x"  # Smallest machine (256 MB RAM)
  memory = "256mb"
  cpus = 1

# Metrics
[metrics]
  port = 9091
  path = "/metrics"
```

---

## 🚀 Quick Deploy Script

Save this as `deploy.sh` in `kindly-web/`:

```bash
#!/bin/bash
set -e

echo "🚀 Deploying kindly_dedup landing page to Fly.io..."

# Ensure we're in the right directory
cd "$(dirname "$0")"

# Build WASM bundle (if not already built)
if [ ! -d "dist" ]; then
  echo "📦 Building WASM bundle..."
  env RUSTFLAGS="" trunk build --release
fi

# Verify bundle size
echo "📊 Verifying bundle size..."
./scripts/verify_bundle_size.sh || true

# Deploy to Fly.io
echo "🚀 Deploying to Fly.io..."
flyctl deploy

# Check deployment status
echo "✅ Deployment complete! Checking status..."
flyctl status

# Get app URL
APP_URL=$(flyctl info | grep Hostname | awk '{print $3}')
echo ""
echo "✅ Deployed to: https://$APP_URL"
echo ""
echo "🧪 Run Lighthouse audit:"
echo "   lighthouse https://$APP_URL --view"
```

Make it executable:
```bash
chmod +x /home/samuel/Primitives/kindly-web/deploy.sh
```

---

## ✅ Current Status

- ✅ **flyctl installed**: v0.3.206
- ✅ **PATH configured**: Added to ~/.bashrc
- ⏳ **Authentication**: Required (run `flyctl auth login`)
- ⏳ **App created**: Not yet (run `flyctl launch` after auth)
- ⏳ **Deployed**: Not yet

---

## 🎯 Next Steps

1. **Authenticate**:
   ```bash
   flyctl auth login
   ```

2. **Create fly.toml** (use recommended config above)

3. **Create Dockerfile** (if using Docker build)

4. **Initialize app**:
   ```bash
   flyctl launch --name kindly-dedup --region iad
   ```

5. **Deploy**:
   ```bash
   flyctl deploy
   ```

6. **Verify**:
   ```bash
   flyctl status
   curl https://kindly-dedup.fly.dev
   ```

---

## 📚 Resources

- Fly.io Docs: https://fly.io/docs/
- Static Assets Guide: https://fly.io/docs/languages-and-frameworks/static/
- WASM on Fly.io: https://fly.io/docs/languages-and-frameworks/wasm/
- Custom Domains: https://fly.io/docs/networking/custom-domain/
- Pricing: https://fly.io/docs/about/pricing/

---

**Status**: ✅ Ready for authentication and deployment
**Next Step**: Run `flyctl auth login` to authenticate
