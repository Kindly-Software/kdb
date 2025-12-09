# Flathub Publishing Guide for kindly-av1

Complete guide for publishing kindly-av1 to Flathub (the official Flatpak app store).

## Quick Reference

**Status**: Proprietary software allowed on Flathub with additional review
**License**: LicenseRef-proprietary
**Review Time**: 1-2 weeks for new apps, 2-3 days for updates
**Repository**: https://github.com/flathub/flathub

---

## 1. Prerequisites

### Required Accounts
- **GitHub account**: For Flathub PR submission
- **GPG key**: For signing commits (optional but recommended)
- **Flathub account**: Automatically created on first PR

### Local Setup
```bash
# Install flatpak-builder
sudo apt install flatpak-builder

# Add Flathub remote
flatpak remote-add --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo

# Install Freedesktop runtime
flatpak install flathub org.freedesktop.Platform//23.08
flatpak install flathub org.freedesktop.Sdk//23.08
```

---

## 2. Initial Flathub Submission (New App)

### Step 1: Fork Flathub Repository
```bash
# Visit https://github.com/flathub/flathub and click "Fork"

# Clone your fork
git clone https://github.com/<YOUR_USERNAME>/flathub.git
cd flathub
```

### Step 2: Create App Branch
```bash
# Create new branch for your app
git checkout -b new-app/software.kindly.av1
```

### Step 3: Copy Manifest
```bash
# Copy your manifest to the Flathub repo
cp /path/to/kindly-av1/packaging/flatpak/software.kindly.av1.yml .

# Copy AppStream metadata
mkdir -p shared-modules
cp /path/to/kindly-av1/packaging/flatpak/software.kindly.av1.metainfo.xml shared-modules/
```

### Step 4: Update Manifest for Flathub

**CRITICAL CHANGES** for Flathub submission:

```yaml
# Change binary source from local file to URL
sources:
  # OLD (local build):
  - type: file
    path: ../../target/x86_64-unknown-linux-gnu/release/kindly-av1

  # NEW (Flathub requires URLs):
  - type: archive
    url: https://github.com/kindly-ai/kindly-av1/releases/download/v1.0.0/kindly-av1-1.0.0-linux-x86_64.tar.gz
    sha256: <ACTUAL_SHA256_OF_TARBALL>
```

**Why**: Flathub builds are reproducible - they must download sources from public URLs.

### Step 5: Prepare Release Tarball

```bash
cd /home/samuel/Primitives/kindly-av1

# Build release binary
cargo build --release --target x86_64-unknown-linux-gnu

# Create tarball for Flathub
mkdir -p kindly-av1-1.0.0-linux-x86_64
cp target/x86_64-unknown-linux-gnu/release/kindly-av1 kindly-av1-1.0.0-linux-x86_64/
tar -czf kindly-av1-1.0.0-linux-x86_64.tar.gz kindly-av1-1.0.0-linux-x86_64/

# Get SHA256 hash
sha256sum kindly-av1-1.0.0-linux-x86_64.tar.gz
```

Upload tarball to GitHub Releases:
```bash
# Create GitHub release
gh release create v1.0.0 \
  --title "kindly-av1 v1.0.0" \
  --notes "Initial Flathub release" \
  kindly-av1-1.0.0-linux-x86_64.tar.gz
```

### Step 6: Validate Manifest

```bash
# Install flatpak-builder-lint
pip3 install --user flatpak-builder-lint

# Validate manifest
flatpak-builder-lint manifest software.kindly.av1.yml

# Validate AppStream metadata
flatpak-builder-lint appstream shared-modules/software.kindly.av1.metainfo.xml
```

Fix any errors before submission.

### Step 7: Test Build Locally

```bash
# Build locally using Flathub manifest
flatpak-builder --force-clean build software.kindly.av1.yml

# Install locally
flatpak-builder --user --install --force-clean build software.kindly.av1.yml

# Run and test
flatpak run software.kindly.av1 --help
```

### Step 8: Commit and Push

```bash
# Add files
git add software.kindly.av1.yml shared-modules/software.kindly.av1.metainfo.xml

# Commit
git commit -m "Add kindly-av1 v1.0.0

kindly-av1 is a GPU-accelerated AV1 encoder delivering 10-100× speedups.
Proprietary binary-only distribution.
"

# Push to your fork
git push origin new-app/software.kindly.av1
```

### Step 9: Submit Pull Request

1. Visit https://github.com/flathub/flathub
2. Click "Pull requests" → "New pull request"
3. Click "compare across forks"
4. Select your fork and branch
5. Title: `Add kindly-av1 v1.0.0`
6. Description:
   ```markdown
   ## New app submission: kindly-av1

   **Category**: AudioVideo, Video
   **License**: Proprietary (binary-only distribution)
   **Homepage**: https://kindly.software

   ### Description
   kindly-av1 is a breakthrough GPU-accelerated AV1 encoder delivering 10-100×
   speedups over CPU encoders. Built with the Computational Capsule Architecture
   for deterministic, production-grade encoding.

   ### Proprietary Software Justification
   - Trade secret algorithms (competitive advantage)
   - Binary-only distribution protects IP
   - Free to use (no licensing fees)
   - GPU acceleration requires proprietary optimizations

   ### Testing
   - [x] Builds successfully with `flatpak-builder`
   - [x] Runs on AMD/NVIDIA/Intel GPUs
   - [x] AppStream metadata validates
   - [x] Manifest passes linter

   ### Screenshots
   ![kindly-av1 encoding](https://kindly.software/screenshots/kindly-av1-encoding.png)
   ```

7. Click "Create pull request"

---

## 3. Flathub Review Process

### What Flathub Reviewers Check

**General** (All Apps):
- [ ] Manifest syntax valid
- [ ] AppStream metadata complete
- [ ] Sources downloadable from public URLs
- [ ] Build succeeds on Flathub infrastructure
- [ ] Permissions justified (--device=dri, etc.)
- [ ] Icon provided (SVG + PNG)

**Proprietary Software** (Extra Scrutiny):
- [ ] Clear license disclosure (`LicenseRef-proprietary`)
- [ ] Justification for proprietary distribution
- [ ] No malware/spyware (manual binary inspection)
- [ ] Reasonable permissions (no excessive --filesystem, --device)
- [ ] Homepage/documentation exists
- [ ] Developer reputation (GitHub activity, website)

### Timeline
- **New app review**: 1-2 weeks (proprietary apps take longer)
- **Update review**: 2-3 days
- **Emergency updates**: Request expedited review in PR

### Common Rejection Reasons
1. **Missing sources**: All files must be downloaded from URLs (no local `path:`)
2. **Excessive permissions**: `--filesystem=host` rejected (use specific paths)
3. **No justification**: Proprietary apps need clear explanation
4. **Broken build**: Must build on Flathub infrastructure (not just locally)
5. **Malware detected**: Binary inspection fails
6. **Invalid AppStream**: Missing required fields (license, categories, description)

---

## 4. Updating Existing Flathub App

### Step 1: Update Manifest
```bash
cd flathub
git checkout -b update/software.kindly.av1-1.1.0

# Edit software.kindly.av1.yml
# - Update version
# - Update URL to new release tarball
# - Update SHA256 hash
```

### Step 2: Update AppStream Metadata
```yaml
# In shared-modules/software.kindly.av1.metainfo.xml
<releases>
  <release version="1.1.0" date="2025-02-01">
    <description>
      <p>New features:</p>
      <ul>
        <li>ROCm backend for AMD GPUs</li>
        <li>2-pass VBR encoding</li>
      </ul>
    </description>
  </release>
  <release version="1.0.0" date="2025-01-15">
    ...
  </release>
</releases>
```

### Step 3: Submit Update PR
```bash
git add software.kindly.av1.yml shared-modules/software.kindly.av1.metainfo.xml
git commit -m "Update kindly-av1 to v1.1.0"
git push origin update/software.kindly.av1-1.1.0

# Create PR on GitHub
```

Updates are approved **much faster** (2-3 days) than new apps.

---

## 5. Direct Hosting (Alternative to Flathub)

If Flathub approval is slow or rejected, host your own Flatpak repository.

### Setup Flatpak Repository
```bash
cd /home/samuel/Primitives/kindly-av1/packaging/flatpak

# Build and export to repo
./build-flatpak.sh --repo-path /path/to/public-repo

# Host on web server (nginx example)
sudo cp -r /path/to/public-repo /var/www/html/flatpak-repo

# Add GPG signing (optional but recommended)
gpg --gen-key
flatpak build-sign --gpg-sign=YOUR_KEY_ID /var/www/html/flatpak-repo
```

### User Installation
```bash
# Add your repository
flatpak remote-add --user kindly-av1 https://kindly.software/flatpak-repo

# Install
flatpak install --user kindly-av1 software.kindly.av1
```

**Advantages**:
- No Flathub approval delays
- Full control over updates
- Proprietary software accepted

**Disadvantages**:
- Less discoverability (not in Flathub GUI)
- Users must manually add repository
- No Flathub CDN (slower downloads)

---

## 6. GPU Access and Permissions

### Critical Permission: `--device=dri`

```yaml
finish-args:
  - --device=dri  # Direct Rendering Infrastructure
```

**What it enables**:
- Vulkan API access (Mesa Vulkan drivers)
- OpenGL/OpenCL (legacy GPU APIs)
- ROCm (AMD GPU compute) via `/dev/dri/renderD*`
- CUDA (NVIDIA, if proprietary drivers installed)

**Security considerations**:
- Gives access to **all GPUs** on system
- Can read GPU memory (shared with other apps)
- Flathub reviewers scrutinize this permission

**Justification** (for Flathub PR):
> kindly-av1 requires GPU access for hardware-accelerated AV1 encoding.
> The `--device=dri` permission grants Vulkan/ROCm/CUDA access to encode
> video 10-100× faster than CPU. Without GPU access, the app cannot function.

### Other Permissions Explained

```yaml
--filesystem=home         # Read/write user videos (input/output files)
--filesystem=/media       # Removable drives (USB, external HDDs)
--share=network           # License validation, telemetry
--socket=x11              # X11 display (for GUI/progress feedback)
--socket=wayland          # Wayland display (modern Linux)
```

**Minimize permissions**: Only request what you need. Flathub rejects overly broad permissions.

---

## 7. Proprietary Software on Flathub

### Allowed Proprietary Apps (Precedents)

Flathub **does allow** proprietary software if:
1. Free to use (no payment required)
2. Clear license disclosure
3. Legitimate use case
4. No malware/spyware

**Examples on Flathub**:
- `com.spotify.Client` (proprietary music streaming)
- `com.discordapp.Discord` (proprietary chat app)
- `com.slack.Slack` (proprietary collaboration)
- `com.nvidia.GeForceNOW` (proprietary game streaming)

### Required Disclosures

**In AppStream metadata**:
```xml
<project_license>LicenseRef-proprietary</project_license>
<custom>
  <value key="flathub::proprietary">true</value>
</custom>
```

**In PR description**:
```markdown
### Proprietary Software Justification
kindly-av1 is distributed as a proprietary binary because:
- Trade secret algorithms provide competitive 10-100× speedups
- GPU optimizations are proprietary intellectual property
- Free to use (no licensing fees or subscriptions)
- Source code is not publicly available
```

### Binary Inspection

Flathub reviewers **manually inspect** proprietary binaries for:
- Malware signatures
- Suspicious network activity
- Excessive permissions
- Obfuscated code

**Be prepared to answer**:
- "What does this binary do?"
- "Why is source code not available?"
- "What data does it collect?"

**Transparency builds trust**: Provide clear documentation, screenshots, and justifications.

---

## 8. Troubleshooting

### Build Fails on Flathub but Works Locally

**Cause**: Flathub uses different build environment (older compilers, strict sandboxing).

**Solutions**:
1. Test with `--sandbox` flag locally:
   ```bash
   flatpak-builder --sandbox --force-clean build software.kindly.av1.yml
   ```
2. Check Flathub build logs (available in PR comments)
3. Pin exact runtime version (`runtime-version: '23.08'`)

### "Sources must be URLs, not local paths"

**Fix**: Replace `type: file, path: ...` with `type: archive, url: ...` pointing to GitHub release.

### AppStream Validation Errors

```bash
# Validate locally
appstreamcli validate software.kindly.av1.metainfo.xml

# Common issues:
# - Missing <url type="homepage">
# - Missing <categories>
# - Invalid <release> date format (use YYYY-MM-DD)
```

### Permission Denied on GPU

**Symptom**: `flatpak run software.kindly.av1` fails with "Cannot open /dev/dri/renderD128".

**Fix**: Ensure `--device=dri` in manifest:
```yaml
finish-args:
  - --device=dri
```

**Workaround** (if still fails):
```bash
# Grant GPU access manually
flatpak override --user --device=dri software.kindly.av1
```

---

## 9. Maintenance and Updates

### Release Workflow
1. Build new version: `cargo build --release`
2. Create tarball: `tar -czf kindly-av1-x.y.z.tar.gz ...`
3. Upload to GitHub Releases
4. Update Flathub manifest (URL + SHA256)
5. Submit PR with release notes
6. Wait for approval (2-3 days)

### Automated Updates (Future)

Flathub supports **external data checker** for automatic updates:

```yaml
# In manifest
x-checker-data:
  type: json
  url: https://api.github.com/repos/kindly-ai/kindly-av1/releases/latest
  version-query: .tag_name
  url-query: .assets[] | select(.name=="kindly-av1-" + $version + "-linux-x86_64.tar.gz") | .browser_download_url
```

Flathub bot will auto-submit PRs when new releases are detected.

---

## 10. Support and Resources

### Official Documentation
- **Flathub Submission**: https://github.com/flathub/flathub/wiki/App-Submission
- **Flatpak Builder**: https://docs.flatpak.org/en/latest/flatpak-builder.html
- **AppStream Spec**: https://www.freedesktop.org/software/appstream/docs/

### Community Support
- **Flathub Matrix**: `#flathub:matrix.org`
- **Flatpak IRC**: `#flatpak` on Libera.Chat
- **GitHub Issues**: https://github.com/flathub/flathub/issues

### kindly-av1 Specific
- **Homepage**: https://kindly.software
- **GitHub**: https://github.com/kindly-ai/kindly-av1
- **Support**: support@kindly.software

---

## 11. Checklist for Submission

Before submitting to Flathub:

- [ ] Manifest validates: `flatpak-builder-lint manifest software.kindly.av1.yml`
- [ ] AppStream validates: `flatpak-builder-lint appstream software.kindly.av1.metainfo.xml`
- [ ] Builds locally: `flatpak-builder build software.kindly.av1.yml`
- [ ] Runs successfully: `flatpak run software.kindly.av1 --help`
- [ ] Icon provided (SVG + PNG fallback)
- [ ] Sources are URLs (not local paths)
- [ ] SHA256 hash matches tarball
- [ ] License disclosed (`LicenseRef-proprietary`)
- [ ] Permissions justified
- [ ] Release tarball uploaded to GitHub
- [ ] Screenshots available
- [ ] Homepage exists
- [ ] PR description complete

---

## Next Steps

1. **Test locally**:
   ```bash
   cd /home/samuel/Primitives/kindly-av1/packaging/flatpak
   ./build-flatpak.sh
   flatpak install --user kindly-av1.flatpak
   flatpak run software.kindly.av1 --version
   ```

2. **Create GitHub Release**:
   ```bash
   gh release create v1.0.0 --title "kindly-av1 v1.0.0" --notes "Initial release"
   ```

3. **Fork Flathub and submit PR** (see Section 2)

4. **Monitor PR for reviewer feedback**

5. **Celebrate when merged!** 🎉
