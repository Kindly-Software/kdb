<p align="center">
  <img src="kdb_transparent.png" alt="Kindly Debugger" width="120" height="120" style="border-radius: 50%"/>
</p>

<h1 align="center">✨ KDB - The Kindly Debugger ✨</h1>

<p align="center">
  <strong>Give your AI the superpower of <span style="color: #FFD700;">traveling back in time</span> to find what went wrong and fix the timeline.</strong>
</p>

<p align="center">
  <a href="https://www.kindly.software">🌐 Website</a> •
  <a href="https://www.kindly.software/#signup">🎉 Start Free</a> •
  <a href="https://www.kindly.software/#docs">📖 Docs</a>
</p>

---

## 🚀 Quick Start (30 seconds)

### Option 1: One-Line Setup
```bash
npx -p @kindly-software-inc/kdb kdb-configure --auto
```
Follow the prompts to sign in with Google and auto-configure your MCP client.

### Option 2: With License Key
```bash
npx -p @kindly-software-inc/kdb kdb-configure --auto --license "YOUR-LICENSE-KEY"
```

### Option 3: Manual Config
Add to your MCP client config (Claude Code, Cursor, etc.):
```json
{
  "mcpServers": {
    "kdb": {
      "command": "npx",
      "args": ["@kindly-software-inc/kdb"]
    }
  }
}
```

**That's it!** 🎊 Ask your AI: *"Debug my crashing program"*

---

## ⏱️ What It Does

**Time-travel debugging for AI workflows.** Step forward. Step backward. Debug as if the bug never existed.

| Feature | Description |
|---------|-------------|
| ⏪ **Step Backward** | Rewind execution to find root causes |
| ⏩ **Step Forward** | Resume from any checkpoint |
| 🎯 **Breakpoints** | Hardware-level with hit counting |
| 📚 **Stack Traces** | SIMD-accelerated unwinding |
| 🔐 **Audit Trail** | Hash-chain integrity for compliance |

---

## 💜 How It Works

```
┌─────────────────┐     MCP      ┌─────────────────┐
│  Your Machine   │◄────────────►│  Kindly Server  │
│  (any OS)       │              │  (Linux x86_64) │
│                 │              │                 │
│  Claude Code    │              │  🔮 Time-Travel │
│  Cursor         │              │  🎯 Breakpoints │
│  Any AI         │              │  📊 Analysis    │
└─────────────────┘              └─────────────────┘
```

**No local installation.** Works on macOS, Windows, Linux via MCP.

---

## 🏷️ Pricing

| Tier | Price | Sessions |
|------|-------|----------|
| 🆓 **Hobby** | Free | 5/month (unlimited during 7-day trial!) |
| 💎 **Pro** | $19/mo | 100/month + unlimited time-travel |
| 🔧 **Engineer** | $49/mo | 500/month + memory replay + LSH bug search |
| 👥 **Teams** | $129/mo | 2,000/month + team audit logs |
| 🏢 **Enterprise** | Custom | Unlimited + SOX/SOC2/GDPR/HIPAA compliance |

**[Start Free →](https://www.kindly.software/#signup)**

---

## 🛠️ MCP Tools

Your AI gets these debugging superpowers:

- `debugger_attach` - Connect to a process
- `debugger_set_breakpoint` - Set breakpoints
- `debugger_continue` - Resume execution
- `debugger_step_forward` - Step forward
- `debugger_step_backward` - ⏪ Step backward in time
- `debugger_get_stack_trace` - Get stack trace
- `debugger_get_variables` - Read memory
- `debugger_export_trace` - Export execution trace

---

## 💬 Example

**You:** "My Rust program crashes at runtime. PID is 12345"

**AI:**
```
I'll attach and debug that for you.

📎 Attached to process 12345
📸 Captured snapshot
🔍 Stack trace shows crash at unwrap() on None in process_data()

The issue is at line 47 in src/main.rs. Here's the fix:
[code suggestion]
```

---

## 📞 Support

- 🌐 [kindly.software](https://www.kindly.software)
- 📖 [Documentation](https://www.kindly.software/#docs)
- 💬 [Discord](https://discord.gg/kindly) *(coming soon)*

---

<p align="center">
  <strong>Built with 💜 by <a href="https://www.kindly.software">Kindly Software</a></strong>
</p>
