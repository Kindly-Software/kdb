# macOS Entitlements Explanation - kindly-av1

## What Are Entitlements?

Entitlements are key-value pairs that grant your app permission to use specific system resources or capabilities beyond the default sandboxed environment. They are embedded in your app's code signature during the signing process.

**Think of entitlements as**: Permissions your app requests from macOS, similar to Android/iOS app permissions.

## Why Entitlements Matter

### Mac App Store Requirements

**Mandatory**:
- ✓ App Sandbox (`com.apple.security.app-sandbox`)
- ✓ Hardened Runtime (enabled via `--options runtime` in codesign)

**Consequence**: Apps without App Sandbox are **automatically rejected** by App Store review.

### Security Model

macOS uses **least privilege principle**:
- Apps start with minimal permissions (sandboxed)
- Request additional permissions via entitlements
- User grants/denies permissions at runtime (for sensitive resources)

## kindly-av1 Entitlements Breakdown

### Required Entitlements

#### 1. App Sandbox (CRITICAL)

```xml
<key>com.apple.security.app-sandbox</key>
<true/>
```

**Purpose**: Enables macOS App Sandbox, isolating the app from the rest of the system.

**What it does**:
- ✓ Restricts file system access (only user-selected files)
- ✓ Restricts network access (unless explicitly allowed)
- ✓ Restricts hardware access (unless explicitly allowed)
- ✓ Prevents keylogging, screen recording, etc.

**Impact on kindly-av1**:
- ✓ User must select input/output files via file dialogs or drag & drop
- ✓ Cannot read arbitrary files (e.g., /Users/john/Documents/video.mp4 without permission)
- ✓ Cannot write to system directories (e.g., /usr/local/bin)

**Compliance**: **MANDATORY** for Mac App Store. No exceptions.

---

#### 2. User-Selected File Access (CRITICAL)

```xml
<key>com.apple.security.files.user-selected.read-write</key>
<true/>
```

**Purpose**: Allow reading and writing files explicitly selected by the user.

**What it does**:
- ✓ User drags video.mp4 into app → app can read video.mp4
- ✓ User selects output directory → app can write encoded.ivf to that directory
- ✓ App retains access to user-selected directories (security-scoped bookmarks)

**How it works**:
```rust
// macOS automatically grants access to files selected via:
// 1. File dialogs (NSOpenPanel, NSSavePanel)
// 2. Drag & drop
// 3. Command-line arguments (if launched from Finder)

// Example: User selects file
fn select_file() -> Option<PathBuf> {
    // Use osascript for sandbox-safe file selection
    let output = std::process::Command::new("osascript")
        .args(&["-e", "POSIX path of (choose file)"])
        .output()
        .ok()?;
    Some(PathBuf::from(String::from_utf8_lossy(&output.stdout).trim()))
}

// Once selected, read/write works normally:
let file = std::fs::File::open(path)?; // ✓ Works
```

**Without this entitlement**:
- ❌ App crashes when trying to read user-selected files
- ❌ "Operation not permitted" errors

**Compliance**: **MANDATORY** for file-based apps like encoders.

---

#### 3. GPU Access (CRITICAL)

```xml
<key>com.apple.security.device.gpu</key>
<true/>
```

**Purpose**: Allow direct GPU access for Metal, Vulkan, ROCm compute workloads.

**What it does**:
- ✓ Access to Metal framework (Apple's GPU API)
- ✓ Access to Vulkan via MoltenVK (translates Vulkan → Metal)
- ✓ Access to ROCm HIP runtime (AMD GPU compute)
- ✓ GPU memory allocation, shader compilation, dispatch

**Impact on kindly-av1**:
- ✓ GPU-accelerated encoding (10-100× speedup)
- ✓ Metal/Vulkan backends work properly
- ✓ ROCm HIP kernels execute on AMD GPUs

**Without this entitlement**:
- ❌ Metal/Vulkan/ROCm initialization fails
- ❌ Fallback to CPU encoding (100× slower)
- ❌ "Access denied" errors from GPU drivers

**Compliance**: Required for GPU-accelerated apps. Apple allows this for video/graphics apps.

---

#### 4. JIT Compilation / Unsigned Executable Memory (CRITICAL for GPU)

```xml
<key>com.apple.security.cs.allow-unsigned-executable-memory</key>
<true/>
```

**Purpose**: Allow runtime code generation (JIT) for Metal/Vulkan shader compilation.

**What it does**:
- ✓ Metal shader compiler generates GPU code at runtime
- ✓ Vulkan SPIR-V → MSL (Metal Shading Language) translation
- ✓ ROCm HIP kernel compilation
- ✓ JIT-compiled GPU kernels execute on GPU

**Technical Details**:
- GPU shaders are compiled **at runtime** (not ahead-of-time)
- Metal/Vulkan drivers generate machine code on-the-fly
- This requires marking memory pages as executable (`mmap PROT_EXEC`)
- macOS Hardened Runtime normally blocks this (prevents code injection attacks)
- This entitlement explicitly allows it for GPU shaders

**Security Implications**:
- ⚠️ Increases attack surface (JIT spraying, ROP attacks)
- ✓ Acceptable for GPU apps (Apple approves this for Metal/Vulkan apps)
- ✓ Required for **all** GPU compute apps (including Apple's own apps)

**Without this entitlement**:
- ❌ Metal shader compilation crashes
- ❌ Vulkan pipeline creation fails
- ❌ "Code signing blocked mmap(PROT_EXEC)" errors

**Compliance**: Required for GPU apps. Apple accepts this for legitimate GPU use cases.

---

### Optional Entitlements (Currently Disabled)

#### 5. Network Client (Optional)

```xml
<!-- Uncomment if needed -->
<!--
<key>com.apple.security.network.client</key>
<true/>
-->
```

**Purpose**: Allow outbound network connections (HTTP/HTTPS requests).

**Use Cases for kindly-av1**:
- License validation (check Gumroad API for valid license key)
- Crash reporting (send crash logs to analytics service)
- Update checks (fetch latest version from kindly.software)
- Streaming output (send encoded video to CDN)

**Current Status**: **DISABLED** (kindly-av1 works offline)

**Enable if**:
- You add Gumroad license validation
- You add crash reporting (e.g., Sentry)
- You add update checks

**Privacy Requirement**:
- Must disclose in privacy policy
- Must explain **why** network access is needed
- Users cannot disable network access (it's all-or-nothing)

---

#### 6. Network Server (Optional)

```xml
<!--
<key>com.apple.security.network.server</key>
<true/>
-->
```

**Purpose**: Allow the app to accept incoming network connections (act as server).

**Use Cases**:
- HTTP server for OBS overlay (kindly-av1 already has this via `http-server` module)
- API endpoint for external tools
- WebSocket server for real-time progress updates

**Current Status**: **DISABLED** (OBS overlay runs in separate binary)

**Enable if**:
- You integrate OBS overlay into main app
- You add REST API for external control

---

#### 7. Camera Access (Optional)

```xml
<!--
<key>com.apple.security.device.camera</key>
<true/>
-->
```

**Purpose**: Access the Mac's camera for live video encoding.

**Use Case**: Encode camera feed directly (e.g., for streaming/recording)

**Current Status**: **DISABLED** (kindly-av1 encodes files, not camera)

**Enable if**: You add camera capture feature

**Privacy Requirement**:
- Must declare in `NSCameraUsageDescription` in Info.plist
- User sees permission dialog on first access
- User can revoke in System Preferences → Privacy

---

#### 8. Microphone Access (Optional)

```xml
<!--
<key>com.apple.security.device.audio-input</key>
<true/>
-->
```

**Purpose**: Access the Mac's microphone for audio encoding.

**Use Case**: Capture audio for video encoding (e.g., screen recording with voiceover)

**Current Status**: **DISABLED** (kindly-av1 encodes video files, not live audio)

**Enable if**: You add audio capture feature

---

#### 9. Disable Library Validation (Use with Caution)

```xml
<!--
<key>com.apple.security.cs.disable-library-validation</key>
<true/>
-->
```

**Purpose**: Allow loading unsigned/third-party libraries (e.g., ROCm HIP runtime).

**What it does**:
- ✓ Load libraries not signed by Apple or your team
- ✓ Load libraries from /usr/local/lib (e.g., ROCm)
- ✓ Load libraries from third-party installers

**Security Risk**:
- ⚠️ **HIGH RISK**: Allows code injection attacks (malicious .dylib loaded)
- ⚠️ Apple may **reject** during App Store review

**Alternative**:
- **Bundle ROCm libraries in .app** (sign with your certificate)
- Use `@rpath` to load bundled libraries
- Avoids need for this entitlement

**Recommendation**: **AVOID** unless absolutely necessary. Bundle all dependencies.

---

## Hardened Runtime

### What is Hardened Runtime?

Hardened Runtime is a **security feature** that protects your app from code injection and memory corruption attacks.

**Enabled via**:
```bash
codesign --sign "Developer ID" \
    --options runtime \  # ← Enables Hardened Runtime
    --entitlements app.entitlements \
    kindly-av1.app
```

**Protections**:
- ✓ Prevents code injection (no `mmap PROT_EXEC` without entitlement)
- ✓ Prevents DYLD environment variable attacks
- ✓ Prevents debugging without entitlement
- ✓ Prevents runtime tampering

**Entitlements Override**:
- `com.apple.security.cs.allow-unsigned-executable-memory` → Allow JIT (GPU shaders)
- `com.apple.security.cs.disable-library-validation` → Allow third-party libraries
- `com.apple.security.get-task-allow` → Allow debugging (set to `false` for production)

**Compliance**: **MANDATORY** for Mac App Store and notarization.

---

## Entitlements vs Info.plist Permissions

### Key Differences

| Aspect | Entitlements | Info.plist |
|--------|--------------|------------|
| **Purpose** | Grant system-level permissions | Declare app metadata and usage descriptions |
| **Format** | XML plist (embedded in code signature) | XML plist (in app bundle) |
| **Enforcement** | Enforced by macOS kernel at runtime | Enforced by AppKit/Foundation frameworks |
| **Example** | `com.apple.security.device.gpu` | `NSCameraUsageDescription` |

### Usage Descriptions in Info.plist

For **sensitive permissions** (camera, microphone, location), you must **also** add usage descriptions in `Info.plist`:

```xml
<!-- Entitlement grants permission -->
<key>com.apple.security.device.camera</key>
<true/>

<!-- Info.plist explains WHY you need it (shown to user) -->
<key>NSCameraUsageDescription</key>
<string>kindly-av1 needs camera access to encode live video.</string>
```

**User sees dialog**:
```
"kindly-av1" would like to access the camera.
kindly-av1 needs camera access to encode live video.

[Don't Allow]  [OK]
```

**kindly-av1 usage descriptions** (in Info.plist):
- `NSCameraUsageDescription` - "kindly-av1 may access camera for live video encoding."
- `NSMicrophoneUsageDescription` - "kindly-av1 may access microphone for audio encoding."
- `NSAppleEventsUsageDescription` - "kindly-av1 needs access to AppleScript events for automation integration."

**Note**: These are **placeholder** descriptions. Update based on actual features.

---

## Debugging Sandbox Violations

### Enable Sandbox Logging

```bash
# Stream sandbox violations in real-time
log stream --predicate 'process == "kindly-av1" AND eventMessage CONTAINS "sandbox"' --level debug

# Example output:
# Sandbox: kindly-av1(12345) deny(1) file-read-data /Users/john/.zshrc
#                                    ↑ Violation type  ↑ Blocked path
```

### Common Violations

**1. Reading dotfiles** (`.zshrc`, `.bashrc`, `.profile`):
```
deny(1) file-read-data /Users/john/.zshrc
```

**Fix**: Don't read user's shell config files. kindly-av1 doesn't need these.

**2. Writing to /tmp**:
```
deny(1) file-write-create /tmp/kindly-av1-temp
```

**Fix**: Use `NSTemporaryDirectory()` or Rust `std::env::temp_dir()`:
```rust
let tmp = std::env::temp_dir(); // ✓ Sandbox-safe
// macOS returns: ~/Library/Caches/kindly-av1/Temp/
```

**3. Accessing /usr/local**:
```
deny(1) file-read-data /usr/local/lib/libhip_runtime.dylib
```

**Fix**: Bundle library in `.app/Contents/Frameworks/` and sign it.

**4. Network access without entitlement**:
```
deny(1) network-outbound connect to 192.168.1.1:443
```

**Fix**: Add `com.apple.security.network.client` entitlement.

---

## Testing Entitlements

### Check Embedded Entitlements

```bash
# Extract entitlements from signed app
codesign -d --entitlements - kindly-av1.app

# Output:
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist ...>
<plist version="1.0">
<dict>
    <key>com.apple.security.app-sandbox</key>
    <true/>
    <key>com.apple.security.device.gpu</key>
    <true/>
    ...
</dict>
</plist>
```

### Validate Entitlements

```bash
# Check if app is properly sandboxed
asctl sandbox check --pid $(pgrep kindly-av1)

# Expected output:
kindly-av1 (pid 12345): Sandboxed
```

### Test GPU Access

```rust
// Test if GPU entitlement works
fn test_gpu_access() -> Result<(), String> {
    // Try to create Metal device
    let device = metal::Device::system_default()
        .ok_or("Failed to create Metal device")?;

    println!("✓ GPU access granted");
    println!("  Device: {}", device.name());

    Ok(())
}
```

**Expected output**:
```
✓ GPU access granted
  Device: Apple M2 Pro
```

**Without entitlement**:
```
Error: Failed to create Metal device
```

---

## App Store Review Considerations

### Entitlements Apple Scrutinizes

**1. Unsigned Executable Memory** (`allow-unsigned-executable-memory`):
- ⚠️ Apple reviews carefully (potential security risk)
- ✓ Explain in review notes: "Required for Metal/Vulkan GPU shader compilation"
- ✓ Provide technical justification

**2. Disable Library Validation** (`disable-library-validation`):
- ⚠️ Often **rejected** (high security risk)
- ✓ Apple prefers bundled libraries
- ✓ Only use if absolutely necessary (e.g., ROCm)

**3. Network Entitlements**:
- ⚠️ Triggers privacy review
- ✓ Must explain in privacy policy
- ✓ Must justify in review notes

### Review Notes Template

**For App Store reviewers**:
```
ENTITLEMENTS JUSTIFICATION

1. com.apple.security.device.gpu
   Required for GPU-accelerated AV1 encoding via Metal/Vulkan.
   Without GPU access, encoding is 100× slower (CPU-only).

2. com.apple.security.cs.allow-unsigned-executable-memory
   Required for Metal shader compiler (runtime JIT compilation).
   All GPU-accelerated apps require this for shader execution.

3. com.apple.security.files.user-selected.read-write
   Required to read input video files and write encoded output.
   User explicitly selects files via drag & drop or file dialog.

TESTING:
- Launch app and drag video.mp4 into window
- Encoding completes in <10 seconds (GPU-accelerated)
- Output saved to user-selected directory
```

---

## Summary

### kindly-av1 Entitlements

| Entitlement | Status | Purpose |
|-------------|--------|---------|
| `app-sandbox` | ✓ Required | Isolate app from system |
| `files.user-selected.read-write` | ✓ Required | Access user-selected files |
| `device.gpu` | ✓ Required | GPU-accelerated encoding |
| `allow-unsigned-executable-memory` | ✓ Required | Metal/Vulkan JIT shaders |
| `network.client` | Optional | License validation, updates |
| `network.server` | Optional | OBS overlay HTTP server |
| `device.camera` | Optional | Live camera encoding |
| `device.audio-input` | Optional | Live audio encoding |
| `disable-library-validation` | ⚠️ Avoid | Third-party libraries (ROCm) |

### Next Steps

1. **Review entitlements file**: `kindly-av1.app/Contents/Resources/kindly-av1.entitlements`
2. **Test sandboxed app**: Run on clean macOS install, check for violations
3. **Enable optional entitlements**: If adding network/camera features
4. **Document in review notes**: Explain GPU/JIT entitlements to Apple reviewers

For questions: support@kindly.software
