#!/bin/bash
# Setup self-signed TLS certificate for kindly.software (testing only)
# This is a fallback for when Let's Encrypt DNS configuration isn't ready yet
#
# IMPORTANT: Self-signed certificates will show browser warnings.
# Use this only for development/testing. Switch to Let's Encrypt for production.

set -e

DOMAIN="kindly.software"
CERT_DIR="/etc/ssl/certs"
KEY_DIR="/etc/ssl/private"
VALIDITY_DAYS=365

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_ok() {
    echo -e "${GREEN}✅ ${1}${NC}"
}

log_warn() {
    echo -e "${YELLOW}⚠️  ${1}${NC}"
}

log_error() {
    echo -e "${RED}❌ ${1}${NC}"
}

log_section() {
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${BLUE}${1}${NC}"
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
}

log_section "Self-Signed Certificate Setup for ${DOMAIN}"

echo ""
log_warn "This creates a SELF-SIGNED certificate"
echo "   - Not trusted by browsers (will show warnings)"
echo "   - Valid for ${VALIDITY_DAYS} days"
echo "   - Useful for development and testing ONLY"
echo ""
echo "For production, use Let's Encrypt:"
echo "   ./scripts/setup_letsencrypt.sh"
echo ""
read -p "Continue? (y/n) " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Cancelled."
    exit 0
fi

# Create certificate directories
echo ""
log_section "Step 1: Create Directories"

echo "📁 Creating certificate directories..."
sudo mkdir -p "$KEY_DIR"
sudo mkdir -p "$CERT_DIR"
log_ok "Directories created"

# Generate private key
echo ""
log_section "Step 2: Generate Private Key"

KEY_FILE="${KEY_DIR}/${DOMAIN}.key"

echo "🔑 Generating RSA 4096-bit private key..."
echo "   (This may take a moment...)"

if sudo openssl genrsa -out "$KEY_FILE" 4096 > /dev/null 2>&1; then
    sudo chmod 600 "$KEY_FILE"
    log_ok "Private key generated: ${KEY_FILE}"
    echo ""
    echo "📊 Key size:"
    sudo openssl rsa -in "$KEY_FILE" -text -noout 2>/dev/null | grep -i "private-key:" | sed 's/^/   /'
else
    log_error "Failed to generate private key"
    exit 1
fi

# Generate certificate signing request
echo ""
log_section "Step 3: Create Certificate Signing Request"

CSR_FILE="/tmp/${DOMAIN}.csr"

echo "📝 Creating certificate signing request..."
sudo openssl req -new \
    -key "$KEY_FILE" \
    -out "$CSR_FILE" \
    -subj "/C=US/ST=California/L=San Francisco/O=Kindly Software/CN=${DOMAIN}/emailAddress=admin@${DOMAIN}" \
    2>/dev/null || {
    log_error "Failed to create CSR"
    exit 1
}

log_ok "Certificate signing request created"

# Generate self-signed certificate
echo ""
log_section "Step 4: Generate Self-Signed Certificate"

CERT_FILE="${CERT_DIR}/${DOMAIN}.crt"

echo "📜 Generating self-signed certificate (${VALIDITY_DAYS} days validity)..."
sudo openssl x509 -req \
    -days ${VALIDITY_DAYS} \
    -in "$CSR_FILE" \
    -signkey "$KEY_FILE" \
    -out "$CERT_FILE" \
    -extfile <(printf "subjectAltName=DNS:${DOMAIN},DNS:www.${DOMAIN}") \
    2>/dev/null || {
    log_error "Failed to generate certificate"
    exit 1
}

log_ok "Self-signed certificate generated: ${CERT_FILE}"

# Verify certificate
echo ""
log_section "Step 5: Verify Certificate"

echo "🔍 Certificate details:"
sudo openssl x509 -in "$CERT_FILE" -noout -text | grep -E "Subject:|Issuer:|Not Before:|Not After:|Public-Key:|DNS:" | sed 's/^/   /'

# Display fingerprint
echo ""
echo "📌 Certificate fingerprint (SHA-256):"
sudo openssl x509 -in "$CERT_FILE" -noout -fingerprint -sha256 | sed 's/^/   /'

log_ok "Certificate verification passed"

# Cleanup CSR
rm -f "$CSR_FILE"

# Setup permissions
echo ""
log_section "Step 6: Configure Permissions"

echo "🔐 Setting secure permissions..."
sudo chmod 644 "$CERT_FILE"
sudo chmod 600 "$KEY_FILE"

echo "📂 File permissions:"
sudo ls -lh "$CERT_FILE" | awk '{print "   " $0}'
sudo ls -lh "$KEY_FILE" | awk '{print "   " $0}'

# Create combined file (sometimes needed)
echo ""
log_section "Step 7: Create Combined Certificate + Key"

COMBINED_FILE="${CERT_DIR}/${DOMAIN}.pem"
echo "📦 Creating combined certificate file..."
sudo sh -c "cat '$CERT_FILE' '$KEY_FILE' > '$COMBINED_FILE'"
sudo chmod 600 "$COMBINED_FILE"
log_ok "Combined PEM file created: ${COMBINED_FILE}"

# Configuration guide
echo ""
log_section "Step 8: Configuration Guide"

cat > /tmp/selfsigned_config.txt <<EOF
Self-Signed Certificate Configuration
======================================

Certificate Files:
  - Certificate: ${CERT_FILE}
  - Private Key: ${KEY_FILE}
  - Combined:    ${COMBINED_FILE}

Update your HTTP server configuration:

  [tls]
  enabled = true
  cert_path = "${CERT_FILE}"
  key_path = "${KEY_FILE}"
  min_tls_version = "1.3"

Or use combined file:
  cert_path = "${COMBINED_FILE}"
  key_path = "${COMBINED_FILE}"

Testing Commands:

  1. Check certificate locally:
     openssl x509 -in ${CERT_FILE} -noout -dates

  2. Test HTTPS (ignoring certificate warning):
     curl -k -I https://localhost/

  3. Test with verbose TLS info:
     curl -k -v https://localhost/ 2>&1 | grep -E "SSL|TLS|Certificate"

  4. Using openssl s_client:
     openssl s_client -connect localhost:443 -cert ${CERT_FILE} -key ${KEY_FILE}

Browser Testing:
  1. Navigate to https://kindly.software (or your IP)
  2. Click "Advanced" → "Proceed anyway" (self-signed warning)
  3. View certificate details (click lock icon → Certificate)

Certificate Renewal:
  This certificate expires in ${VALIDITY_DAYS} days.
  To renew, run this script again or use:
    openssl req -x509 -days ${VALIDITY_DAYS} -newkey rsa:4096 ...

Upgrade to Let's Encrypt:
  When DNS is ready, use the Let's Encrypt setup script:
    ./scripts/setup_letsencrypt.sh

  This will:
    ✓ Obtain browser-trusted certificates
    ✓ Enable automatic renewal
    ✓ Remove self-signed warnings

EOF

cat /tmp/selfsigned_config.txt
echo ""
log_ok "Configuration details saved"

# Final summary
echo ""
log_section "Setup Complete"

echo "✨ Self-signed certificate is ready!"
echo ""
echo "   Certificate: ${CERT_FILE}"
echo "   Private Key: ${KEY_FILE}"
echo ""
log_warn "Remember: This is for testing only!"
echo ""
echo "📋 Next steps:"
echo ""
echo "1. Update your HTTP server configuration with the certificate paths above"
echo ""
echo "2. Restart your HTTP server:"
echo "   sudo systemctl restart atomic-http-server"
echo ""
echo "3. Test HTTPS with curl (ignore certificate warning):"
echo "   curl -k -I https://localhost/"
echo ""
echo "4. Test in browser:"
echo "   - You'll see a certificate warning (normal for self-signed)"
echo "   - Click 'Advanced' and 'Proceed anyway'"
echo "   - The connection is still encrypted"
echo ""
echo "5. When DNS is configured, upgrade to Let's Encrypt:"
echo "   ./scripts/setup_letsencrypt.sh"
echo ""
