# Testing kindly_dedup Landing Page with Playwright MCP

## Setup Complete ✅

The Playwright MCP server has been successfully installed and configured for Claude Code.

### Installation Details
- **MCP Server**: Playwright MCP v0.0.45
- **Location**: `/home/samuel/playwright-mcp/`
- **Configuration**: Added to `~/.claude.json`
- **Status**: ✓ Connected

```bash
claude mcp list
# Output: playwright: node /home/samuel/playwright-mcp/cli.js - ✓ Connected
```

## Development Server

The kindly-web landing page is currently served at:
- **URL**: http://localhost:8080
- **Server**: Python HTTP server (dist/ directory)
- **Status**: Running (200 OK responses)

## Testing the Landing Page

### Option 1: Using Playwright MCP in a New Claude Code Session

Start a new Claude Code conversation and the Playwright MCP tools will be available. You can then test the landing page with commands like:

```
Navigate to http://localhost:8080 and verify:
1. All 10 sections render correctly (Hero, Performance, Features, Comparison, Demo, Pricing, API, FAQ, CTA, Footer)
2. Conservative performance claims are displayed with hardware disclaimers
3. Download Demo button is prominent and styled with gold accent
4. FAQ section shows all 5 questions
5. Mobile responsiveness works correctly
```

### Option 2: Manual Playwright Script

Alternatively, use this standalone Playwright test script:

```bash
cd /home/samuel/Primitives/kindly-web
npm init -y
npm install -D @playwright/test
```

Create `tests/landing-page.spec.js`:

```javascript
const { test, expect } = require('@playwright/test');

test.describe('kindly_dedup Landing Page', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('http://localhost:8080');
  });

  test('should render all 10 sections', async ({ page }) => {
    // Check Hero section
    await expect(page.locator('h1')).toContainText('Deduplicate');

    // Check Performance section exists
    const perfSection = page.locator('text=38× Faster Single-Threaded');
    await expect(perfSection).toBeVisible();

    // Check Demo section
    const demoButton = page.locator('button:has-text("Download Demo")');
    await expect(demoButton).toBeVisible();

    // Check FAQ section
    const faq = page.locator('text=What makes kindly_dedup faster');
    await expect(faq).toBeVisible();

    // Check pricing tiers
    await expect(page.locator('text=$0')).toBeVisible(); // Free tier
    await expect(page.locator('text=$0.01')).toBeVisible(); // Pay as you go
  });

  test('should have conservative performance claims with disclaimers', async ({ page }) => {
    // Check for hardware disclaimer
    await expect(page.locator('text=/AMD Ryzen 9 6900HX/i')).toBeVisible();
    await expect(page.locator('text=/results may vary/i')).toBeVisible();
  });

  test('should have prominent Download Demo button', async ({ page }) => {
    const demoButton = page.locator('button:has-text("Download Demo")');
    await expect(demoButton).toBeVisible();

    // Check for gold accent (primary button variant)
    const buttonClass = await demoButton.getAttribute('class');
    expect(buttonClass).toContain('primary'); // Assuming gold uses primary variant
  });

  test('should be mobile responsive', async ({ page }) => {
    // Test mobile viewport
    await page.setViewportSize({ width: 375, height: 667 });

    // Hero should still be visible
    await expect(page.locator('h1')).toBeVisible();

    // CTA button should be visible
    await expect(page.locator('button:has-text("Download Demo")')).toBeVisible();
  });

  test('should display all 5 FAQ questions', async ({ page }) => {
    const faqQuestions = [
      'What makes kindly_dedup faster',
      'How do you validate performance',
      'What accuracy can I expect',
      'Can I try before buying',
      'How do I use it'
    ];

    for (const question of faqQuestions) {
      await expect(page.locator(`text=/${question}/i`)).toBeVisible();
    }
  });

  test('should have contact information in footer', async ({ page }) => {
    await expect(page.locator('text=/sales@kindly.software/i')).toBeVisible();
    await expect(page.locator('text=/support@kindly.software/i')).toBeVisible();
  });
});
```

Run tests:
```bash
npx playwright test
```

### Option 3: Using MCP Tools Directly (Requires New Session)

In a fresh Claude Code session with Playwright MCP active, you can use these commands:

1. **Navigate to page**:
   ```
   playwright navigate http://localhost:8080
   ```

2. **Take accessibility snapshot**:
   ```
   playwright snapshot
   ```

3. **Click elements**:
   ```
   playwright click "Download Demo"
   ```

4. **Verify text**:
   ```
   playwright expect "38× Faster Single-Threaded"
   ```

## Validation Checklist

Use this checklist to validate the landing page transformation:

### ✅ Content Accuracy
- [ ] Hero headline: "Deduplicate Datasets in Seconds, Not Hours"
- [ ] Subheadline: "38-580× faster than Python"
- [ ] Hardware disclaimer present: "AMD Ryzen 9 6900HX (16 cores)"
- [ ] "Results may vary" warning displayed

### ✅ Section Rendering
- [ ] Performance section shows 3 StatCards
- [ ] Demo section shows 3 tiers (100K, 1M, 10M docs)
- [ ] FAQ section shows all 5 questions
- [ ] API section shows CLI + Rust examples
- [ ] Pricing shows 3 tiers (Free, Pay As You Go, Enterprise)

### ✅ Design Consistency
- [ ] Byzantine purple (#702963) primary color used
- [ ] Gold (#FFD700) accent on CTA buttons
- [ ] Download Demo button is prominent (gold, primary variant)
- [ ] All sections use existing design system components

### ✅ Performance Claims
- [ ] Single-threaded: 38× faster (60K vs 1,572 docs/sec)
- [ ] Multi-threaded: 400K-900K docs/sec @ 16 cores (range provided)
- [ ] Accuracy: 95-98% F1 score
- [ ] All claims include "results may vary" context

### ✅ Mobile Responsiveness
- [ ] Hero section readable on mobile (375px width)
- [ ] Buttons tap-friendly (minimum 44px touch target)
- [ ] Text scales appropriately
- [ ] Sections stack vertically on mobile

### ✅ Accessibility
- [ ] All interactive elements keyboard accessible
- [ ] Sufficient color contrast (WCAG 2.1 AA)
- [ ] Semantic HTML structure
- [ ] ARIA labels where needed

## Lighthouse Audit

Run a Lighthouse audit to verify production readiness:

```bash
# Install Lighthouse CLI
npm install -g lighthouse

# Run audit
lighthouse http://localhost:8080 --view --output html --output-path ./lighthouse-report.html

# Target scores:
# Performance: 95+
# Accessibility: 95+
# Best Practices: 95+
# SEO: 95+
```

## Bundle Size Verification

Verify the bundle stays within budget:

```bash
cd /home/samuel/Primitives/kindly-web
./scripts/verify_bundle_size.sh

# Expected output:
# ✅ WASM bundle: <380KB gzipped (currently ~160KB)
# ✅ Total bundle: <200KB (WASM + JS)
```

## Next Steps

1. **Start fresh Claude Code session** to access Playwright MCP tools
2. **Run automated tests** using the script above
3. **Perform Lighthouse audit** to validate performance
4. **Test on real devices** (mobile, tablet, desktop)
5. **Deploy to production** once all validations pass

## Stopping the Development Server

To stop the Python HTTP server when done:

```bash
pkill -f "python3 -m http.server 8080"
```

## MCP Server Management

View MCP server status:
```bash
claude mcp list
```

Remove Playwright MCP server (if needed):
```bash
claude mcp remove playwright
```

Re-add Playwright MCP server:
```bash
claude mcp add playwright node /home/samuel/playwright-mcp/cli.js
```

## Documentation

- Playwright MCP: https://github.com/microsoft/playwright-mcp
- Playwright Testing: https://playwright.dev
- MCP Protocol: https://modelcontextprotocol.io

---

**Status**: ✅ Ready for automated testing with Playwright MCP
**Server**: ✓ Running on http://localhost:8080
**MCP**: ✓ Configured and connected
