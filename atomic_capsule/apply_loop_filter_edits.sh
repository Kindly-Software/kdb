#!/bin/bash
# Script to wire LoopFilterCapsule into Av1EncoderMetacapsule

FILE="/home/samuel/Primitives/atomic_capsule/src/encoder/encoder_metacapsule.rs"

# 1. Update header comment: 14 -> 15 sub-capsules
sed -i 's/Orchestrates 14 sub-capsules/Orchestrates 15 sub-capsules/' "$FILE"
sed -i 's/Sub-Capsules: 12 encoder stages/Sub-Capsules: 15 encoder stages/' "$FILE"

# 2. Add import for LoopFilterCapsule
sed -i '/use crate::encoder::SuperresolutionCapsule;/a #[cfg(feature = "portable_simd")]\nuse crate::encoder::loop_filter::LoopFilterCapsule;' "$FILE"

# 3. Update doc comment: Sub-Capsules (14 total) -> (15 total)
sed -i 's/# Sub-Capsules (14 total)/# Sub-Capsules (15 total)/' "$FILE"

# 4. Add LoopFilterCapsule to the numbered list
sed -i '/14\. SuperresolutionCapsule/a /// 15. LoopFilterCapsule (T2): Deblocking filter' "$FILE"

# 5. Add parameter to new() function
sed -i '/        _intra_prediction: \&IntraPredictionCapsule,$/a \        #[cfg(feature = "portable_simd")]\n        _loop_filter: \&LoopFilterCapsule,' "$FILE"

# 6. Add import in tests module
sed -i '/    use crate::encoder::intra_prediction::IntraPredictionCapsule;/a \    #[cfg(feature = "portable_simd")]\n    use crate::encoder::loop_filter::LoopFilterCapsule;' "$FILE"

# 7. Add LoopFilterCapsule creation and pass to new() in create_test_metacapsule()
sed -i '/        let intra_prediction = IntraPredictionCapsule::new();$/a \        #[cfg(feature = "portable_simd")]\n        let loop_filter = LoopFilterCapsule::new(32, 3);' "$FILE"

sed -i '/            \&intra_prediction,$/a \            #[cfg(feature = "portable_simd")]\n            \&loop_filter,' "$FILE"

echo "✅ All edits applied successfully!"
echo ""
echo "Summary of changes:"
echo "  1. Updated header: 14 → 15 sub-capsules"
echo "  2. Added import: use crate::encoder::loop_filter::LoopFilterCapsule"
echo "  3. Updated doc comment: Sub-Capsules (14 total) → (15 total)"
echo "  4. Added to numbered list: 15. LoopFilterCapsule (T2): Deblocking filter"
echo "  5. Added parameter to new(): _loop_filter: &LoopFilterCapsule"
echo "  6. Added test import: use crate::encoder::loop_filter::LoopFilterCapsule"
echo "  7. Updated test helper: create and pass LoopFilterCapsule::new(32, 3)"
