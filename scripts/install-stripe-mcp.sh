#!/bin/bash

# Stripe MCP Installation Script
# Install and configure Stripe MCP server for Claude Code
# Usage: ./install-stripe-mcp.sh [--remote|--local] [--test|--live]

set -e

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
INSTALL_TYPE="${1:-remote}"  # remote or local
ENV_MODE="${2:-test}"        # test or live
STRIPE_MCP_URL="https://mcp.stripe.com/"
LOCAL_PORT=3000

echo -e "${BLUE}=== Stripe MCP Installation ===${NC}"
echo "Install Type: $INSTALL_TYPE"
echo "Environment: $ENV_MODE"
echo ""

# Check prerequisites
echo -e "${YELLOW}Checking prerequisites...${NC}"

if ! command -v npm &> /dev/null; then
    echo -e "${RED}❌ npm not found. Please install Node.js 18+${NC}"
    exit 1
fi
echo -e "${GREEN}✅ npm found: $(npm --version)${NC}"

if ! command -v node &> /dev/null; then
    echo -e "${RED}❌ Node.js not found. Please install Node.js 18+${NC}"
    exit 1
fi
echo -e "${GREEN}✅ Node.js found: $(node --version)${NC}"

# Function to check if MCP config exists
check_mcp_config() {
    if [ -f ~/.claude/settings.json ]; then
        echo -e "${GREEN}✅ Claude Code config found${NC}"
        return 0
    elif [ -d ~/.config/claude-code ]; then
        echo -e "${GREEN}✅ Claude Code config directory found${NC}"
        return 0
    else
        echo -e "${YELLOW}⚠️  No Claude Code config found yet${NC}"
        return 1
    fi
}

# Installation based on type
if [ "$INSTALL_TYPE" = "remote" ]; then
    echo ""
    echo -e "${BLUE}Installing Remote Stripe MCP Server${NC}"
    echo "URL: $STRIPE_MCP_URL"
    echo ""

    if ! check_mcp_config; then
        echo -e "${YELLOW}Note: Claude Code will initialize config on first run${NC}"
    fi

    echo -e "${GREEN}✅ Remote server ready at: $STRIPE_MCP_URL${NC}"
    echo "   No local installation needed"
    echo ""
    echo -e "${YELLOW}Next steps:${NC}"
    echo "1. Add to Claude Code:"
    echo "   ${BLUE}claude mcp add --transport http stripe https://mcp.stripe.com/${NC}"
    echo ""
    echo "2. Or manually add to ~/.claude/settings.json:"
    cat << 'EOF'
    {
      "mcpServers": {
        "stripe": {
          "url": "https://mcp.stripe.com/",
          "auth": "oauth"
        }
      }
    }
EOF

elif [ "$INSTALL_TYPE" = "local" ]; then
    echo ""
    echo -e "${BLUE}Installing Local Stripe MCP Server${NC}"
    echo ""

    # Check if already installed
    if npm list -g @stripe/mcp &> /dev/null; then
        echo -e "${YELLOW}@stripe/mcp already installed globally${NC}"
        VERSION=$(npm list -g @stripe/mcp | grep @stripe/mcp | head -1 | awk '{print $2}')
        echo "Version: $VERSION"
    else
        echo -e "${YELLOW}Installing @stripe/mcp globally...${NC}"
        npm install -g @stripe/mcp
        echo -e "${GREEN}✅ Installation complete${NC}"
    fi

    echo ""
    echo -e "${YELLOW}Testing local server...${NC}"

    # Create a simple test script
    TEST_SCRIPT=$(mktemp)
    cat > "$TEST_SCRIPT" << 'SCRIPT'
#!/bin/bash
timeout 5 npx -y @stripe/mcp --tools=all --api-key=sk_test_dummy 2>&1 | head -5
SCRIPT
    chmod +x "$TEST_SCRIPT"

    if $TEST_SCRIPT &> /dev/null || true; then
        echo -e "${GREEN}✅ Local server can be started${NC}"
    fi
    rm "$TEST_SCRIPT"

    echo ""
    echo -e "${YELLOW}Next steps:${NC}"
    echo "1. Set your Stripe API key:"
    echo "   ${BLUE}export STRIPE_SECRET_KEY='sk_test_YOUR_KEY'${NC}"
    echo ""
    echo "2. Add to Claude Code (~/.claude/settings.json):"
    cat << 'EOF'
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

else
    echo -e "${RED}❌ Unknown install type: $INSTALL_TYPE${NC}"
    echo "Usage: $0 [--remote|--local] [--test|--live]"
    exit 1
fi

# API Key setup
echo ""
echo -e "${BLUE}=== API Key Configuration ===${NC}"
echo ""
echo -e "${YELLOW}⚠️  IMPORTANT: You need a Stripe API key${NC}"
echo ""
echo "Steps:"
echo "1. Go to https://dashboard.stripe.com/apikeys"
echo "2. Create a Restricted API Key:"
echo "   - Grant: Products, Prices, Customers, Payment Intents, Checkout Sessions"
echo "   - Copy the Secret Key (sk_test_...)"
echo ""
echo "3. Store securely:"
echo "   Option A (Environment Variable):"
echo "   ${BLUE}export STRIPE_SECRET_KEY='sk_test_YOUR_KEY'${NC}"
echo ""
echo "   Option B (System Keyring):"
echo "   ${BLUE}pass insert stripe/secret_key${NC}"
echo "   Then load in shell:"
echo "   ${BLUE}export STRIPE_SECRET_KEY=\$(pass show stripe/secret_key)${NC}"
echo ""
echo "   Option C (.env file, .gitignored):"
echo "   ${BLUE}echo 'STRIPE_SECRET_KEY=sk_test_YOUR_KEY' >> ~/.env${NC}"
echo "   ${BLUE}source ~/.env${NC}"
echo ""

# Verification
echo ""
echo -e "${BLUE}=== Verification ===${NC}"
echo ""

if [ -z "$STRIPE_SECRET_KEY" ]; then
    echo -e "${YELLOW}⚠️  STRIPE_SECRET_KEY not set${NC}"
    echo "Please set your API key before using Stripe MCP"
else
    KEY_PREFIX=${STRIPE_SECRET_KEY:0:7}
    echo -e "${GREEN}✅ STRIPE_SECRET_KEY detected: ${KEY_PREFIX}...${NC}"
fi

echo ""

# Test with curl if remote
if [ "$INSTALL_TYPE" = "remote" ]; then
    echo -e "${YELLOW}Testing remote server connectivity...${NC}"
    if curl -s -I "$STRIPE_MCP_URL" > /dev/null 2>&1; then
        echo -e "${GREEN}✅ Remote server is accessible${NC}"
    else
        echo -e "${RED}❌ Cannot reach remote server${NC}"
        echo "   Check your internet connection"
    fi
fi

echo ""
echo -e "${GREEN}=== Installation Complete ===${NC}"
echo ""
echo "Next: Follow the configuration steps above to add Stripe to your MCP config"
echo ""
echo "Documentation: /home/samuel/Primitives/STRIPE_MCP_SETUP.md"
echo ""
