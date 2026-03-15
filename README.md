# Steady Steady

A Chrome browser bridge for AI coding agents. One persistent WebSocket connection to Chrome's DevTools Protocol — list tabs, open/close pages, read content, take screenshots, and run JavaScript, all from the CLI.

Built to give [Claude Code](https://claude.ai/claude-code) (or any terminal-based AI agent) full browser control without extensions.

## "Isn't this just Chrome's CDP?"

Chrome 146 added a remote debugging toggle — so why do you need this?

**Raw CDP** gives you a WebSocket endpoint. That's it. To actually use it, you need to:
- Write WebSocket connection code for every interaction
- Handle Chrome's "Allow remote debugging?" prompt on every new connection
- Manually attach to targets, manage session IDs, parse protocol responses
- Deal with 32-character hex target IDs

**Steady Steady** wraps all of that into a persistent server + simple CLI:

| Raw CDP | Steady Steady |
|---------|---------------|
| New "Allow?" prompt per connection | Click Allow once |
| 20 lines of WebSocket code per action | `node chrome.js read <tab>` |
| Manage session IDs yourself | Sessions cached automatically |
| 32-char hex target IDs | First 8 chars work |
| No AI agent integration | Comes with a Claude Code skill |
| No visual feedback | Badge shows which agent is in control |

CDP is the engine. This is the steering wheel.

## How It Works

```
Chrome Browser (CDP WebSocket :9222)
        │
        │  single persistent connection
        │
  chrome.js serve  (background server :9333)
        │
        │  HTTP POST
        │
  chrome.js <command>  (CLI client)
```

The server holds one WebSocket connection open to Chrome, so you only click "Allow remote debugging" once. All CLI commands go through the server via HTTP.

## Quick Start

### 1. Enable Chrome remote debugging

Go to `chrome://inspect/#remote-debugging` and toggle **"Allow remote debugging for this browser instance"**.

Requires Chrome 146+.

### 2. Install and run

```bash
git clone https://github.com/gneyal/p_87_steady_steady.git
cd p_87_steady_steady
npm install
node chrome.js serve
```

Chrome will prompt "Allow remote debugging?" — click **Allow** once.

### 3. Use it

```bash
# List all open tabs
node chrome.js tabs

# Open a new tab
node chrome.js open https://example.com

# Read page text content (works on authenticated pages!)
node chrome.js read A107EE39

# Close a tab
node chrome.js close A107EE39

# Take a screenshot
node chrome.js screenshot A107EE39 /tmp/shot.png

# Navigate an existing tab to a new URL
node chrome.js navigate A107EE39 https://google.com

# Run JavaScript on a page
node chrome.js js A107EE39 "document.title"

# Stop the server
node chrome.js stop
```

Tab IDs are shown by the `tabs` command. You can use the full 32-char ID or just the first 8 characters.

## Use with Claude Code

### Auto-approve browser commands

Add this to your Claude Code settings (`~/.claude/settings.json`):

```json
{
  "permissions": {
    "allow": [
      "Bash(node /path/to/chrome.js:*)"
    ]
  }
}
```

### Install the Claude Code skill (optional)

This teaches Claude Code how to use the browser bridge automatically — it'll know to start the server, look up tabs, read pages, etc. without you explaining anything.

```bash
mkdir -p ~/.claude/skills/steady-steady
cp SKILL.md ~/.claude/skills/steady-steady/SKILL.md
```

Then just ask Claude Code things like:
- "read this tweet for me" (paste URL)
- "what tabs do I have open?"
- "open gmail and check my latest emails"
- "take a screenshot of the current page"

It uses your actual logged-in browser session — no re-authentication needed.

## What Can You Do With It?

- **Read JS-heavy sites** — tweets, SPAs, anything that needs JavaScript to render
- **Access authenticated content** — Gmail, dashboards, internal tools — through your logged-in session
- **Automate browser actions** — click buttons, fill forms, archive emails
- **Extract data** — scrape content from any page you can see
- **Take screenshots** — capture any tab as PNG
- **Run arbitrary JS** — full access to the DOM of any open tab

## Requirements

- Chrome 146+ (for the remote debugging toggle)
- Node.js 18+
- macOS (reads Chrome's `DevToolsActivePort` from `~/Library/Application Support/Google/Chrome/`)

## License

MIT
