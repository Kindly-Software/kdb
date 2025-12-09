# KindlyAPI TUI Flow: Real-Time API Health Dashboard with Intelligence Insights

**Design Philosophy**: Like `htop` for APIs - real-time health monitoring with mouse-clickable navigation + intelligent workflow suggestions

**Tech Stack**: ratatui + crossterm (Rust TUI framework)

**Intelligence Features Displayed**:
- Workflow suggestions panel (auto-detected endpoint relationships)
- Auto-filled parameters indicator (smart inference in action)
- Cross-API relationship graph (multi-API orchestration visualization)
- Normalization status (response schema harmonization active)
- OAuth refresh status (automatic token management)

---

## Screen Layouts

### 1. Main Dashboard (Default View)

```
┌─ KindlyAPI v1.0 - Intelligent MCP Generation ───────────────────────────────────┐
│ [Dashboard] [Integrations] [API Catalog] [Workflows] [Audit Log] [Settings] [Free 2/3] ⚙️ │
├───────────────────────────────────────────────────────────────────────────────────┤
│ Active Integrations (5: 2 Official MCP, 2 Extended, 1 Long Tail) Last: 14:23:45 │
├───┬──────────┬──────────┬─────────────┬──────────────────────────────────────────┤
│ # │ API      │ Type     │ Health      │ Coverage / Endpoints                    │
├───┼──────────┼──────────┼─────────────┼──────────────────────────────────────────┤
│ 1 │ Stripe*  │ MCP      │ L0 ████████ │ 23 tools (3% coverage)                  │
│ 2 │ Stripe+  │ Extended │ L0 ████████ │ 687 tools (remaining 97%)               │
│ 3 │ GitHub*  │ MCP      │ L1 ████░░░░ │ 45 tools (20% coverage)                 │
│ 4 │ GitHub+  │ Extended │ L0 ████████ │ 198 tools (remaining 80%)               │
│ 5 │ Twilio   │ API      │ L0 ████████ │ 87 tools (no official MCP)              │
└───┴──────────┴──────────┴─────────────┴──────────────────────────────────────────┘

Legend: * = Official MCP  |  + = KindlyAPI extension  |  (blank) = Long tail

Health Levels:  L0=Normal  L1=Degraded  L2=Limited  L3=Paused
Circuit Breaker States: [████████] = Healthy   [████░░░░] = Warning

┌─ Recent Activity ─────────────────────────────────────────────────────────────┐
│ 14:23:41  ✓ Stripe     POST /v1/charges              201  142ms  🧠 Inferred  │
│ 14:23:38  ✓ GitHub     GET  /repos/user/repo/issues  200  187ms  📊 Cached   │
│ 14:23:35  ✗ Twilio     POST /Messages                401  234ms  🔄 Refreshed│
│ 14:23:32  ✓ OpenAI     POST /v1/chat/completions     200   91ms  ✨ Workflow │
│ 14:23:29  ✓ Stripe     GET  /v1/customers/cus_123   200  156ms  🔄 Normalized│
└───────────────────────────────────────────────────────────────────────────────┘

Intelligence Legend: 🧠=Smart param inference | 📊=Cache hit | 🔄=OAuth refresh/Normalized | ✨=Workflow detected

Actions:  [Enter] View Details  [Space] Pause/Resume  [D] Delete  [R] Refresh
          [Q] Quit  [?] Help

Status: Capsule Runtime Active | ACB-64 Breakers: 1 L1, 1 L3 | ALE-128 Chain: Valid
```

**Mouse Actions:**
- Click integration row → View Details screen
- Click column header → Sort by that column
- Click health bar → Breaker details popup
- Click tab buttons → Switch screens

---

### 2. Integration Details View

```
┌─ Integration Details: Stripe+ (Extended) ────────────────────────────────────────┐
│ [← Back to Dashboard]                                       [Test Auth] [Update] │
├──────────────────────────────────────────────────────────────────────────────────┤
│ Integration ID: ext_stripe_advanced                                             │
│ Type:           MCP Extension (extends official Stripe MCP server)              │
│ Base URL:       https://api.stripe.com/v1                                       │
│ Auth Type:      Bearer Token                                                     │
│ Created:        2025-09-28 10:34:12                                              │
│ Last Used:      2 minutes ago                                                    │
├──────────────────────────────────────────────────────────────────────────────────┤
│ Extension Status                                                                 │
│ ├─ Official MCP Coverage:  23/710 endpoints (3.2%)                              │
│ ├─ With KindlyAPI:         710/710 endpoints (100%)                             │
│ ├─ Gap Filled:             687 endpoints (coupons, disputes, refunds, etc.)     │
│ └─ Use Together:           stripe.create_charge (official) + stripe_advanced... │
├──────────────────────────────────────────────────────────────────────────────────┤
│ Health & Performance                                                             │
│ ├─ Current Health:     L0 (Normal) ████████████████████████                     │
│ ├─ Breaker Level:      0 (no degradation)                                       │
│ ├─ Success Rate (24h): 99.2% (1,234 / 1,244)                                    │
│ ├─ Avg Latency (p50):  145ms                                                     │
│ ├─ Avg Latency (p99):  342ms                                                     │
│ └─ Drift Detected:     No                                                        │
├──────────────────────────────────────────────────────────────────────────────────┤
│ Rate Limits                                                                      │
│ ├─ Per Minute:  45 / 60   [████████████████░░░░] 75%  Resets: 14:24:00         │
│ ├─ Per Hour:    892 / 1000 [██████████████████░░] 89%  Resets: 15:00:00        │
│ └─ Per Day:     12,345 / 50,000 [██████░░░░░░░░░░░░] 25%                       │
├──────────────────────────────────────────────────────────────────────────────────┤
│ Security Policy                                                                  │
│ ├─ Allowed Endpoints: 12 patterns configured                                    │
│ ├─ Blocked Methods:   DELETE (safety policy)                                    │
│ └─ Retry Policy:      Exponential backoff (5 attempts)                          │
├──────────────────────────────────────────────────────────────────────────────────┤
│ Available Endpoints (12 allowed, 45 total)                                      │
│   [✓] POST   /v1/charges              - Create a charge                         │
│   [✓] GET    /v1/charges/{id}         - Retrieve charge                         │
│   [✓] POST   /v1/customers            - Create customer                         │
│   [✓] GET    /v1/customers/{id}       - Retrieve customer                       │
│   [✗] DELETE /v1/customers/{id}       - Delete customer (BLOCKED)               │
│   ...                                                                            │
│   [E] Show All Endpoints                                                         │
├──────────────────────────────────────────────────────────────────────────────────┤
│ Recent Errors (Last 10)                                                          │
│   None in past 24 hours                                                          │
└──────────────────────────────────────────────────────────────────────────────────┘

Actions: [B] Back  [T] Test Auth  [U] Update Config  [A] View Audit Log  [Q] Quit
```

**Mouse Actions:**
- Click "Test Auth" → Run test_auth and show result
- Click "Update" → Open config editor
- Click endpoint row → Show parameter details
- Click rate bar → Detailed rate limit breakdown

---

### 3. Audit Log View (ALE-128 Chain)

```
┌─ Audit Log: All Integrations ────────────────────────────────────────────────────┐
│ [← Back] [Filter ▼] [Export]                            Chain Valid: ✓ Verified │
├──────────────────────────────────────────────────────────────────────────────────┤
│ Showing last 100 entries (24h window) - Free tier                               │
├──────────┬──────────┬────────────────┬────────────────┬────────┬───────────────┤
│ Time     │ Event    │ Integration    │ Endpoint       │ Status │ Hash          │
├──────────┼──────────┼────────────────┼────────────────┼────────┼───────────────┤
│ 14:23:41 │ CALL_OK  │ Stripe         │ POST /charges  │ 201    │ a3f9...1d2e   │
│ 14:23:38 │ CALL_OK  │ GitHub         │ GET /repos/... │ 200    │ 7bc4...8a1f   │
│ 14:23:35 │ CALL_ERR │ Twilio         │ POST /Message..│ 401    │ 2d5e...9c3b   │
│ 14:23:32 │ CALL_OK  │ OpenAI         │ POST /v1/chat/.│ 200    │ 8e7f...4d2a   │
│ 14:23:29 │ CALL_OK  │ Stripe         │ GET /customer..│ 200    │ 1a2b...5e6d   │
│ 14:22:15 │ BREAKER  │ Twilio         │ L0→L3 (AUTH)   │ N/A    │ 9f8e...3c4d   │
│ 14:21:08 │ DRIFT    │ GitHub         │ Snapshot #2 OK │ N/A    │ 6d7e...2a1b   │
│ 14:20:45 │ INTEGRATE│ GitHub         │ Setup complete │ N/A    │ 4c5d...8e9f   │
│ ...                                                                              │
└──────────┴──────────┴────────────────┴────────────────┴────────┴───────────────┘

Hash Chain Verification: [████████████████████] 100/100 entries valid
  Genesis Hash: 0000000000000000000000000000000000000000000000000000000000000000
  Latest Hash:  a3f9d8e71c2b5f4a9d8e71c2b5f4a9d8e71c2b5f4a9d8e71c2b5f4a1d2e3c4b

Events:  CALL_OK=Success  CALL_ERR=Error  BREAKER=Level change  DRIFT=Spec change
         INTEGRATE=New integration  SPEC_FETCH=Doc retrieval

Actions: [F] Filter  [E] Export  [V] Verify Chain  [↑↓] Scroll  [Q] Back
```

**Mouse Actions:**
- Click event row → Show full details (params, response, error)
- Click "Filter" → Open filter dialog
- Click "Export" → Save audit log to file
- Click "Verify Chain" → Run full tamper-evident verification

---

### 4. Filter Dialog (Popup)

```
┌─ Filter Audit Log ───────────────────────────────────┐
│                                                       │
│  Integration:  [All ▼]                               │
│                                                       │
│  Event Type:   [☑] Calls  [☑] Breaker  [☐] Drift    │
│                                                       │
│  Status:       [☑] Success  [☑] Error                │
│                                                       │
│  Time Range:   [Last 24h ▼]                          │
│                                                       │
│  Endpoint:     [________] (regex supported)          │
│                                                       │
│                                                       │
│          [Apply Filter]  [Reset]  [Cancel]           │
│                                                       │
└───────────────────────────────────────────────────────┘
```

---

### 5. API Catalog Screen (NEW)

```
┌─ API Catalog - Browse 100+ Pre-Integrated APIs ─────────────────────────────────┐
│ [← Back] [Filter: Without MCP ▼] [Search: payment________] [Category: All ▼]   │
├───────────────────────────────────────────────────────────────────────────────────┤
│ Showing 23 APIs without official MCP servers (Filter: payment)                  │
├───┬──────────────┬─────────────────────────────────┬──────┬────────┬───────────┤
│ # │ API          │ Description                     │ Type │ Auth   │ Popularity│
├───┼──────────────┼─────────────────────────────────┼──────┼────────┼───────────┤
│ 1 │ PayPal       │ Payment processing              │ API  │ OAuth  │ ★★★★★     │
│ 2 │ Square       │ Point of sale & payments        │ API  │ Bearer │ ★★★★☆     │
│ 3 │ Authorize.Net│ Payment gateway                 │ API  │ APIKey │ ★★★☆☆     │
│ 4 │ Braintree    │ PayPal payment processor        │ API  │ OAuth  │ ★★★★☆     │
│ 5 │ Adyen        │ Global payment platform         │ API  │ APIKey │ ★★★★☆     │
│ ...                                                                              │
└───┴──────────────┴─────────────────────────────────┴──────┴────────┴───────────┘

Categories: Payment (23) | Communication (45) | Database (12) | AI (18) | Other (34)

Actions: [Enter] Add Integration  [C] Compare APIs  [?] API Details  [B] Back
```

**Mouse Actions:**
- Click API row → Show API details + "Add Integration" button
- Click "Compare" → Side-by-side comparison (e.g., PayPal vs Square)
- Click "Filter" → Toggle between "Without MCP", "All APIs", "Community only"

---

### 6. Compare APIs Screen (NEW)

```
┌─ Compare: PayPal vs Square ──────────────────────────────────────────────────────┐
│ [← Back to Catalog]                                                              │
├───────────────────────┬────────────────────────┬───────────────────────────────┤
│ Feature               │ PayPal                 │ Square                        │
├───────────────────────┼────────────────────────┼───────────────────────────────┤
│ Has Official MCP?     │ No                     │ No                            │
│ Auth Type             │ OAuth2                 │ Bearer Token                  │
│ Setup Complexity      │ Moderate               │ Easy                          │
│ Rate Limits           │ 10,000/day             │ 40/minute                     │
│ API Coverage          │ Full (OpenAPI 3.1)     │ Full (OpenAPI 3.0)            │
│ Community Rating      │ ★★★★★ (234 users)      │ ★★★★☆ (156 users)             │
│ Pricing               │ 2.9% + $0.30           │ 2.6% + $0.10                  │
│ Availability          │ Global                 │ US, CA, UK, AU, JP            │
│ Special Features      │ Subscriptions, PayLater│ POS integration, Invoicing    │
├───────────────────────┴────────────────────────┴───────────────────────────────┤
│ Recommendation: Both work well. PayPal for global reach, Square for lower fees. │
│ Alternative: Stripe* (has official MCP server - prefer if available)            │
└───────────────────────────────────────────────────────────────────────────────────┘

Actions: [1] Add PayPal  [2] Add Square  [B] Back to Catalog
```

---

### 7. Settings Screen

```
┌─ Settings ───────────────────────────────────────────────────────────────────────┐
│ [Dashboard] [Integrations] [API Catalog] [Audit Log] [Settings]                 │
├──────────────────────────────────────────────────────────────────────────────────┤
│ Account                                                                          │
│ ├─ Tier:           Free (2/3 KindlyAPI integrations + unlimited MCP monitoring) │
│ ├─ MCP Server:     Active (stdio transport)                                     │
│ └─ Config Path:    ~/.config/kindly-api/config.toml                             │
│                                                                                  │
│ Security                                                                         │
│ ├─ Vault:          OS Keychain (encrypted at rest)                              │
│ ├─ Audit Log:      Enabled (24h retention)                                      │
│ └─ API Key:        [Generate] [Rotate] [Revoke]                                 │
│                                                                                  │
│ Runtime                                                                          │
│ ├─ Capsule Engine: v1.0.0 (lockfree mode)                                       │
│ ├─ ACB-64 Breaker: Auto-tune enabled                                            │
│ ├─ ALE-128 Ledger: Chain valid (last verified: 2m ago)                          │
│ └─ ET-1kB Tiles:   Checkpointing every 60s                                      │
│                                                                                  │
│ Performance                                                                      │
│ ├─ Policy Checks:  <100ns (p99)                                                 │
│ ├─ Avg Call Time:  187ms (includes network)                                     │
│ └─ Memory Usage:   12.4 MB (4 integrations loaded)                              │
│                                                                                  │
│ TUI Options                                                                      │
│ ├─ Refresh Rate:   [1s ▼]                                                       │
│ ├─ Mouse Support:  [Enabled ▼]                                                  │
│ └─ Color Scheme:   [Default ▼]                                                  │
│                                                                                  │
│ [Upgrade to Pro]                                                                 │
│ ├─ Unlimited integrations                                                       │
│ ├─ 7-day audit retention                                                        │
│ ├─ AI-powered features (discover, optimize)                                     │
│ └─ Dashboard + alerts                                   [$20/mo] [Learn More]   │
│                                                                                  │
└──────────────────────────────────────────────────────────────────────────────────┘

Actions: [S] Save  [R] Reset to Defaults  [Q] Back
```

---

### 6. Error Details Popup

```
┌─ Error Details ──────────────────────────────────────────┐
│ Time:            14:23:35                                 │
│ Integration:     Twilio (int_x7y8z9a0b1c2d3e4)           │
│ Endpoint:        POST /2010-04-01/Accounts/.../Messages  │
│ Status:          401 Unauthorized                         │
│ Error Code:      AUTH_EXPIRED                             │
│ Latency:         234ms                                    │
│ Audit Hash:      2d5e9c3b...7a8f1e2d                      │
│                                                            │
│ Explanation:                                               │
│ Authentication token has expired or been revoked.         │
│ Twilio requires token refresh every 24 hours.             │
│                                                            │
│ Suggested Fixes:                                           │
│ 1. Update credentials:                                     │
│    → Run: update_integration(integration_id, auth: {...}) │
│                                                            │
│ 2. Test new credentials:                                   │
│    → Run: test_auth(integration_id)                       │
│                                                            │
│ 3. Check Twilio dashboard for revoked tokens:             │
│    → https://console.twilio.com/settings/api-keys         │
│                                                            │
│ Circuit Breaker Action:                                    │
│ Integration moved to L3 (PAUSED) to prevent further       │
│ failed attempts. Will auto-retry after credentials update.│
│                                                            │
│                         [Copy Command]  [Close]            │
└────────────────────────────────────────────────────────────┘
```

---

## State Machine

```
┌─────────────┐
│   Loading   │  (Initial state: Read ET-1kB tiles, restore capsules)
└──────┬──────┘
       │
       ▼
┌─────────────┐
│    Ready    │  (No integrations configured)
└──────┬──────┘
       │ integrate_api()
       ▼
┌─────────────┐
│   Working   │  (Active integrations, processing calls)
└─────┬───┬───┘
      │   │
      │   └─ call_endpoint() → Success/Error
      │
      ├─ ACB-64 breaker flip → Degraded State (L1-L3)
      │
      ├─ Drift detected → Snapshot Rotation
      │
      └─ Error accumulation → Paused State (L3)
```

**State Transitions:**
- `Loading → Ready`: All ET-1kB tiles loaded, ALE-128 chain verified
- `Ready → Working`: First integration added
- `Working → Degraded`: Breaker level L1/L2 (reduce quality, show warning)
- `Working → Paused`: Breaker level L3 (stop calls, show error + fix)
- `Degraded/Paused → Working`: Manual intervention (update_integration, test_auth)

---

## Real-Time Updates (Capsule Integration)

**Update Sources:**
- **AIS-128 (API Integration State)**: Polled every 1s for health/rate changes
- **ACB-64 (Circuit Breaker)**: Event-driven updates on level changes
- **ALE-128 (Audit Ledger)**: Streamed as new entries are committed
- **ET-1kB (Epoch Tiles)**: Read on startup, written every 60s

**UI Refresh Strategy:**
- Main dashboard: 1s refresh (configurable)
- Details view: 2s refresh (less frequent)
- Audit log: Live stream (event-driven)
- Health bars: Real-time animation (smooth transitions)

---

## Keyboard Shortcuts

**Global:**
- `Q` / `Ctrl+C`: Quit
- `?` / `h`: Help screen
- `R`: Force refresh
- `↑↓`: Navigate lists
- `Tab`: Switch tabs
- `Enter`: Select / View details
- `Esc`: Back / Cancel

**Dashboard:**
- `1-9`: Jump to integration N
- `Space`: Pause/resume selected integration
- `D`: Delete integration (with confirmation)
- `N`: New integration wizard

**Details View:**
- `T`: Test auth
- `U`: Update config
- `A`: View audit log
- `E`: View endpoints

**Audit Log:**
- `F`: Filter
- `V`: Verify chain
- `E`: Export
- `/`: Search

---

## Mouse Support (via crossterm)

**Clickable Elements:**
- All buttons (primary, secondary)
- Tab navigation
- List items (integrations, endpoints, audit entries)
- Progress bars (show tooltip on hover)
- Dropdowns (show options on click)
- Links (open in browser if applicable)

**Hover Effects:**
- Highlight row on hover
- Show tooltip with additional info
- Cursor changes (pointer for clickable, default otherwise)

---

## Color Scheme

**Health Levels:**
- L0 (Normal): Green (`████████`)
- L1 (Degraded): Yellow (`████░░░░`)
- L2 (Limited): Orange (`██░░░░░░`)
- L3 (Paused): Red (`░░░░░░░░`)

**Status Indicators:**
- Success: Green `✓`
- Error: Red `✗`
- Warning: Yellow `⚠`
- Info: Blue `ℹ`

**Text:**
- Normal: White
- Highlighted: Cyan
- Error: Red
- Success: Green
- Muted: Gray

---

## Performance Requirements

**Rendering:**
- 60 FPS target for smooth animations
- <16ms frame time (measured via criterion)
- Efficient diff-based rendering (ratatui)

**Memory:**
- <20MB for TUI (with 10 integrations)
- Capsules kept in shared memory (no duplication)
- Audit log: Stream from disk, don't load all into RAM

**CPU:**
- <1% CPU when idle (polling only)
- <5% CPU during active refresh (1s interval)
- No blocking operations on render thread

---

## Implementation Notes

**Framework:** ratatui (previously tui-rs)
**Backend:** crossterm (cross-platform terminal control)
**Layout:** Flexbox-inspired constraint system
**Widgets:** List, Table, Gauge, Paragraph, Block, Tabs
**Mouse:** crossterm::event::MouseEvent

**Code Structure:**
```rust
src/tui/
├── app.rs          // App state machine
├── ui.rs           // Screen rendering
├── events.rs       // Keyboard/mouse event handling
├── screens/
│   ├── dashboard.rs
│   ├── details.rs
│   ├── audit.rs
│   └── settings.rs
└── widgets/
    ├── health_bar.rs   // Custom gauge with ACB-64 integration
    ├── rate_meter.rs   // Rate limit visualization
    └── audit_table.rs  // ALE-128 chain display
```

**Capsule Access:**
- Read AIS-128 via `read_ok()` (lockfree)
- Subscribe to ACB-64 breaker events
- Stream ALE-128 via `LedgerStream`
- Load ET-1kB on startup for continuity

---

## Testing Strategy

**Manual Testing:**
- Test all screens with mock data
- Verify mouse clicks on all buttons
- Test keyboard shortcuts
- Verify color rendering in different terminals

**Automated Testing:**
- Unit tests for state machine transitions
- Integration tests for capsule reading
- Snapshot tests for screen layouts (insta crate)
- Performance tests for rendering (criterion)

**Accessibility:**
- Screen reader compatible (ANSI escape codes)
- Keyboard-only navigation supported
- High-contrast mode option
- No reliance on color alone for critical info
