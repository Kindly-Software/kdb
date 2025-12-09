# kindly_dedup Distribution Packages v0.2.1

## Available Packages

### 1. Customer Demo Package
**File**: `kindly_dedup_demo_v0.2.1.zip`

**For**: End customers evaluating the software

**Contents**:
- `client_demo` - Pre-built demo binary (751 KB)
- `README.md` - Quick start guide + custom data instructions
- `SALES_SHEET.md` - Product overview
- `EVALUATION_LICENSE.txt` - 30-day evaluation terms

**Use Case**: Send this to customers who want to try kindly_dedup
- 30-day evaluation period
- 5 million document limit
- Full production performance

### 2. Sales Partner Package
**File**: `kindly_dedup_sales_partner_v0.2.1.zip`

**For**: Your sales partner

**Contents**:
- `README_PARTNER.md` - Partner onboarding guide
- `SALES_PARTNER_INSTRUCTIONS.md` - How to sell (EN)
- `SALES_PARTNER_INSTRUCTIONS_FR.md` - How to sell (FR)
- `SALES_SHEET.md` - Customer-facing materials
- `TECHNICAL_DETAILS.md` - Deep technical reference

**Use Case**: Give this to your partner so they can sell independently
- Target customer profiles
- Sales pitch and objections
- Commission structure
- Demo walkthrough

## File Verification

SHA-256 checksums are in `checksums.txt`

Verify integrity:
```bash
sha256sum -c checksums.txt
```

## Distribution

### For Customers:
```bash
# Send via email or file sharing
# Customer extracts and runs:
unzip kindly_dedup_demo_v0.2.1.zip
cd customer_demo_v0.2.1
chmod +x client_demo
./client_demo
```

### For Sales Partners:
```bash
# Send to your partner
# They use it as sales toolkit
unzip kindly_dedup_sales_partner_v0.2.1.zip
cd sales_partner_v0.2.1
# Read README_PARTNER.md for onboarding
```

## Version Info

- **Version**: 0.2.1
- **Release Date**: October 31, 2025
- **Demo Limit**: 5 million documents
- **Evaluation Period**: 30 days
- **Binary Size**: 751 KB
- **Platform**: Linux x86_64 (tested on Ubuntu 24.04)

## Support

- **Sales**: sales@kindly.software
- **Support**: support@kindly.software
- **Partner Program**: sales@kindly.software

---

**Dedup from Kindly 💜**
