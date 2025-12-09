#!/bin/bash
# ROCm Installation Verification Script
# Usage: ssh samuel@kindly-hub "~/Primitives/kindly-av1/scripts/verify_rocm.sh"

set -e

echo "======================================"
echo "ROCm Installation Verification"
echo "======================================"
echo ""

# Check ROCm version
echo "1. ROCm Version:"
/opt/rocm-6.0.2/bin/hipcc --version 2>&1 | head -3 || echo "❌ hipcc not found"
echo ""

# Check GPU detection
echo "2. GPU Detection (rocminfo):"
/opt/rocm-6.0.2/bin/rocminfo 2>&1 | grep -A 5 "gfx1035" || echo "❌ gfx1035 not detected"
echo ""

# Enumerate GPUs
echo "3. GPU Enumeration:"
/opt/rocm-6.0.2/bin/rocm_agent_enumerator 2>&1 | grep -v "SyntaxWarning" || echo "❌ rocm_agent_enumerator failed"
echo ""

# Check kernel driver
echo "4. AMDGPU Kernel Driver:"
lsmod | grep amdgpu | head -1 || echo "❌ amdgpu driver not loaded"
echo ""

# Check KFD device
echo "5. KFD Device:"
ls -la /dev/kfd 2>/dev/null || echo "❌ /dev/kfd not found"
echo ""

# Check DRI devices
echo "6. DRI Devices:"
ls -la /dev/dri/ | grep -E "card|render" || echo "❌ No DRI devices found"
echo ""

# Check user groups
echo "7. User Groups:"
groups | grep -E "render|video" && echo "✅ User in render/video groups" || echo "❌ User NOT in render/video groups"
echo ""

# Test HIP compilation
echo "8. HIP Compilation Test:"
cat > /tmp/hip_verify.cpp << 'EOF'
#include <hip/hip_runtime.h>
#include <stdio.h>
int main() {
    int deviceCount = 0;
    hipError_t err = hipGetDeviceCount(&deviceCount);
    if (err != hipSuccess) {
        printf("❌ hipGetDeviceCount failed: %s\n", hipGetErrorString(err));
        return 1;
    }
    printf("✅ Found %d HIP device(s)\n", deviceCount);
    if (deviceCount > 0) {
        hipDeviceProp_t prop;
        hipGetDeviceProperties(&prop, 0);
        printf("   Device 0: %s\n", prop.name);
        printf("   Compute Capability: %d.%d\n", prop.major, prop.minor);
        printf("   Memory: %.2f GB\n", prop.totalGlobalMem / 1e9);
    }
    return 0;
}
EOF

/opt/rocm-6.0.2/bin/hipcc /tmp/hip_verify.cpp -o /tmp/hip_verify 2>&1 | grep -i error && echo "❌ Compilation failed" || echo "✅ Compilation succeeded"

if [ -f /tmp/hip_verify ]; then
    echo ""
    echo "9. HIP Runtime Test:"
    /tmp/hip_verify 2>&1
    rm -f /tmp/hip_verify
fi

rm -f /tmp/hip_verify.cpp

echo ""
echo "======================================"
echo "Verification Complete"
echo "======================================"
