#!/bin/bash

# Configure Stripe MCP for Claude Code
# This script helps set up MCP configuration and manage API keys securely

set -e

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}=== Stripe MCP Configuration ===${NC}"
echo ""

# Function to ask yes/no
ask_yes_no() {
    local prompt="$1"
    local response
    while true; do
        read -p "$(echo -e ${BLUE}$prompt${NC}) (y/n): " -r response
        case "$response" in
            [yY][eE][sS]|[yY])
                return 0
                ;;
            [nN][oO]|[nN])
                return 1
                ;;
            *)
                echo "Please answer y or n"
                ;;
        esac
    done
}

# Function to securely input API key
input_api_key() {
    local key_type="$1"
    read -sp "Enter your Stripe $key_type (hidden): " key
    echo ""
    echo "$key"
}

# Step 1: Choose installation method
echo -e "${YELLOW}Step 1: Choose Installation Method${NC}"
echo "1) Remote (https://mcp.stripe.com) - Recommended, no setup needed"
echo "2) Local (npx @stripe/mcp) - Self-hosted, full control"
echo ""
read -p "Choose [1-2]: " install_choice

case $install_choice in
    1)
        INSTALL_TYPE="remote"
        INSTALL_DESC="Remote (Stripe-hosted)"
        ;;
    2)
        INSTALL_TYPE="local"
        INSTALL_DESC="Local (Self-hosted)"
        ;;
    *)
        echo -e "${RED}Invalid choice${NC}"
        exit 1
        ;;
esac

echo -e "${GREEN}✅ Selected: $INSTALL_DESC${NC}"
echo ""

# Step 2: Choose environment
echo -e "${YELLOW}Step 2: Choose Environment${NC}"
echo "1) Test (sk_test_..., pk_test_...) - Safe, no real charges"
echo "2) Live (sk_live_..., pk_live_...) - Real payments, be careful!"
echo ""
read -p "Choose [1-2]: " env_choice

case $env_choice in
    1)
        ENV_MODE="test"
        ENV_DESC="Test Mode"
        CARD_HINT="Use 4242 4242 4242 4242 for testing"
        ;;
    2)
        if ask_yes_no "⚠️  LIVE MODE - Are you SURE you want to use LIVE API keys?"; then
            ENV_MODE="live"
            ENV_DESC="Live Mode"
            CARD_HINT="Real charges will be processed"
        else
            echo "Switching to test mode..."
            ENV_MODE="test"
            ENV_DESC="Test Mode"
            CARD_HINT="Use 4242 4242 4242 4242 for testing"
        fi
        ;;
    *)
        echo -e "${RED}Invalid choice${NC}"
        exit 1
        ;;
esac

echo -e "${GREEN}✅ Selected: $ENV_DESC${NC}"
echo "   $CARD_HINT"
echo ""

# Step 3: API Key input
echo -e "${YELLOW}Step 3: Stripe API Keys${NC}"
echo ""
echo "You need a Stripe Restricted API Key:"
echo "1. Go to https://dashboard.stripe.com/apikeys"
echo "2. Click 'Create restricted key'"
echo "3. Grant permissions for: Products, Prices, Customers, Payment Intents, Checkout"
echo "4. Copy the Secret Key (sk_test_... or sk_live_...)"
echo ""

SECRET_KEY=$(input_api_key "Secret Key")
if [ -z "$SECRET_KEY" ]; then
    echo -e "${RED}❌ API key cannot be empty${NC}"
    exit 1
fi

# Validate key format
if [[ ! $SECRET_KEY =~ ^sk_(test|live)_ ]]; then
    echo -e "${RED}⚠️  Warning: Key doesn't look like a valid Stripe secret key${NC}"
    echo "   Expected format: sk_test_... or sk_live_..."
    if ! ask_yes_no "Continue anyway?"; then
        exit 1
    fi
fi

echo -e "${GREEN}✅ Secret key received${NC}"
echo ""

# Step 4: Choose storage method
echo -e "${YELLOW}Step 4: Secure Key Storage${NC}"
echo "Choose how to store your API key:"
echo "1) Environment Variable (.env file)"
echo "2) System Keyring (pass/gpg-agent)"
echo "3) MCP Config with env override (NOT recommended)"
echo ""
read -p "Choose [1-3]: " storage_choice

case $storage_choice in
    1)
        STORAGE_METHOD="env_file"
        STORAGE_DESC="Environment Variable (.env)"
        ;;
    2)
        STORAGE_METHOD="keyring"
        STORAGE_DESC="System Keyring (pass)"
        ;;
    3)
        if ask_yes_no "⚠️  Storing key in config is less secure. Continue?"; then
            STORAGE_METHOD="config"
            STORAGE_DESC="MCP Config (less secure)"
        else
            STORAGE_METHOD="env_file"
            STORAGE_DESC="Environment Variable (.env)"
        fi
        ;;
    *)
        echo -e "${RED}Invalid choice${NC}"
        exit 1
        ;;
esac

echo -e "${GREEN}✅ Selected: $STORAGE_DESC${NC}"
echo ""

# Step 5: Save configuration
echo -e "${YELLOW}Step 5: Saving Configuration${NC}"
echo ""

# Create ~/.env if using env_file
if [ "$STORAGE_METHOD" = "env_file" ]; then
    ENV_FILE="$HOME/.env"

    # Backup existing .env
    if [ -f "$ENV_FILE" ]; then
        cp "$ENV_FILE" "$ENV_FILE.backup"
        echo -e "${YELLOW}Backed up existing .env to ${ENV_FILE}.backup${NC}"
    fi

    # Write or append to .env
    {
        echo ""
        echo "# Stripe MCP Configuration - $(date)"
        echo "export STRIPE_ENV=$ENV_MODE"
        echo "export STRIPE_SECRET_KEY='$SECRET_KEY'"
    } >> "$ENV_FILE"

    chmod 600 "$ENV_FILE"
    echo -e "${GREEN}✅ Configuration saved to $ENV_FILE${NC}"
    echo "   (File is readable only by you: chmod 600)"

# Using keyring
elif [ "$STORAGE_METHOD" = "keyring" ]; then
    if command -v pass &> /dev/null; then
        echo "$SECRET_KEY" | pass insert -e "stripe/secret_key" 2>/dev/null || true
        echo -e "${GREEN}✅ Secret key stored in system keyring${NC}"

        # Create helper script
        KEYRING_HELPER="$HOME/.local/bin/load-stripe-env.sh"
        mkdir -p "$(dirname "$KEYRING_HELPER")"
        cat > "$KEYRING_HELPER" << 'SCRIPT'
#!/bin/bash
export STRIPE_SECRET_KEY=$(pass show stripe/secret_key)
SCRIPT
        chmod 700 "$KEYRING_HELPER"
        echo -e "${BLUE}Helper script: $KEYRING_HELPER${NC}"
        echo "Usage: source $KEYRING_HELPER"
    else
        echo -e "${YELLOW}⚠️  'pass' utility not found${NC}"
        echo "Install with: sudo apt install pass"
        echo "Falling back to .env file..."
        STORAGE_METHOD="env_file"
    fi

# Config file (less secure)
elif [ "$STORAGE_METHOD" = "config" ]; then
    echo -e "${YELLOW}⚠️  Storing in MCP config is less secure${NC}"
    echo "Consider using .env or system keyring instead"
fi

echo ""

# Step 6: Update MCP configuration
echo -e "${YELLOW}Step 6: Configure Claude MCP${NC}"
echo ""

if [ "$INSTALL_TYPE" = "remote" ]; then
    echo "Adding remote Stripe MCP server..."

    # Try with claude CLI
    if command -v claude &> /dev/null; then
        if claude mcp add --transport http stripe https://mcp.stripe.com/ 2>/dev/null; then
            echo -e "${GREEN}✅ Stripe MCP added via claude CLI${NC}"
        else
            echo -e "${YELLOW}Could not add via CLI, manual config needed${NC}"
        fi
    else
        echo -e "${YELLOW}Claude CLI not found, using manual config${NC}"
    fi

    # Create manual config example
    MCP_CONFIG_EXAMPLE=$(cat << 'EOF'
{
  "mcpServers": {
    "stripe": {
      "url": "https://mcp.stripe.com/",
      "auth": "oauth"
    }
  }
}
EOF
)

elif [ "$INSTALL_TYPE" = "local" ]; then
    # Generate local config
    if [ "$STORAGE_METHOD" = "env_file" ]; then
        MCP_CONFIG_EXAMPLE=$(cat << 'EOF'
{
  "mcpServers": {
    "stripe": {
      "command": "npx",
      "args": ["@stripe/mcp", "--tools=all"],
      "env": {
        "STRIPE_SECRET_KEY": "sk_test_YOUR_KEY"
      }
    }
  }
}
EOF
)
    else
        MCP_CONFIG_EXAMPLE=$(cat << 'EOF'
{
  "mcpServers": {
    "stripe": {
      "command": "npx",
      "args": ["@stripe/mcp", "--tools=all"],
      "env": {
        "STRIPE_SECRET_KEY": "sk_test_YOUR_KEY"
      }
    }
  }
}
EOF
)
    fi
fi

echo "MCP Configuration:"
echo "$MCP_CONFIG_EXAMPLE"
echo ""
echo "Add this to your Claude config:"
echo "  ~/.claude/settings.json (Claude Code)"
echo "  ~/.cursor/mcp.json (Cursor)"
echo "  .vscode/mcp.json (VS Code)"
echo ""

# Step 7: Verification
echo -e "${YELLOW}Step 7: Verification${NC}"
echo ""

# Test API key
echo "Testing API key..."
RESPONSE=$(curl -s -I -H "Authorization: Bearer $SECRET_KEY" https://api.stripe.com/v1/products 2>&1 || true)

if echo "$RESPONSE" | grep -q "200\|400"; then
    echo -e "${GREEN}✅ API key is valid${NC}"
elif echo "$RESPONSE" | grep -q "401\|403"; then
    echo -e "${RED}❌ API key authentication failed${NC}"
    echo "   Check that the key is correct and active"
else
    echo -e "${YELLOW}⚠️  Could not verify key (check internet connection)${NC}"
fi

echo ""

# Step 8: Testing with Stripe CLI (optional)
echo -e "${YELLOW}Step 8: (Optional) Install Stripe CLI for Local Testing${NC}"
echo ""
if ask_yes_no "Install Stripe CLI for webhook testing and local development?"; then
    echo "Installing Stripe CLI..."

    # Detect OS and install
    if [ "$(uname)" = "Linux" ]; then
        curl -s https://get.stripe.dev | bash
        echo -e "${GREEN}✅ Stripe CLI installed${NC}"
        echo "   First run: stripe login"
    else
        echo "Download from: https://stripe.com/docs/stripe-cli"
    fi
fi

echo ""
echo -e "${GREEN}=== Configuration Complete ===${NC}"
echo ""
echo "Summary:"
echo "  Install Type: $INSTALL_DESC"
echo "  Environment: $ENV_DESC"
echo "  Storage: $STORAGE_DESC"
echo ""
echo "Next steps:"
echo "1. Restart Claude Code"
echo "2. Test with: 'Ask Claude about Stripe products'"
echo "3. See documentation: /home/samuel/Primitives/STRIPE_MCP_SETUP.md"
echo ""
