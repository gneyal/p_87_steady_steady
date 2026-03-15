# Steady Steady

A Chrome browser bridge for AI coding agents. One persistent WebSocket connection to Chrome's DevTools Protocol — list tabs, open/close pages, read content, take screenshots, and run JavaScript, all from the CLI.

Built to give [Claude Code](https://claude.ai/claude-code) (or any terminal-based AI agent) full browser control without extensions.

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

## Prerequisites

- **Chrome 146+** with remote debugging enabled
- Go to `chrome://inspect/#remote-debugging` and toggle **"Allow remote debugging for this browser instance"**
- Node.js 18+

## Setup

```bash
git clone https://github.com/gneyal/p_87_steady_steady.git
cd p_87_steady_steady
npm install
```

## Usage

### Start the server

```bash
node chrome.js serve
```

Chrome will prompt "Allow remote debugging?" — click **Allow** once.

### Commands

```bash
# List all open tabs
node chrome.js tabs

# Open a new tab
node chrome.js open https://example.com

# Close a tab (use full or partial ID from `tabs`)
node chrome.js close A107EE39

# Read page text content
node chrome.js read A107EE39

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

Add this to your Claude Code settings (`~/.claude/settings.json`) to auto-approve browser commands:

```json
{
  "permissions": {
    "allow": [
      "Bash(node /path/to/chrome.js:*)"
    ]
  }
}
```

Then Claude Code can read tweets, navigate pages, extract data from authenticated sites, click buttons, and more — all through your actual logged-in browser session.

## License

MIT
