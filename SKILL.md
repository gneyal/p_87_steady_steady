---
name: steady-steady
version: 1.0.0
description: Control Chrome browser via CDP. Use when user asks to open a URL, read a webpage, take a screenshot, list tabs, close tabs, run JS on a page, or mentions /steady-steady or /browser.
---

# Steady Steady — Chrome Browser Bridge

Control the user's Chrome browser via the Chrome DevTools Protocol. Open tabs, read pages, take screenshots, run JavaScript, and more — all through a persistent CDP connection.

## Prerequisites

- Chrome 146+ with remote debugging enabled (`chrome://inspect/#remote-debugging`)
- Server script at `./chrome.js`
- `ws` package installed in that directory

## Step 0: Check for Updates

Before running, check if chrome.js has been updated:

```bash
# Check if local chrome.js matches the repo
LOCAL_HASH=$(md5 -q ./chrome.js 2>/dev/null)
REMOTE_HASH=$(curl -sf "https://raw.githubusercontent.com/gneyal/p_87_steady_steady/main/chrome.js" 2>/dev/null | md5)
```

If hashes differ, tell the user:
> An update is available for steady-steady. Run this to update:
> `cd <steady-steady-install-dir> && git pull`

If the fetch fails or hashes match, continue silently.

## Step 1: Ensure Server is Running

Check if the CDP server is already running:

```bash
curl -s -o /dev/null -w "%{http_code}" http://127.0.0.1:9333 2>/dev/null
```

If not running (connection refused), start it in the background:

```bash
node ./chrome.js serve
```

Wait 2 seconds after starting, then verify with `tabs` command. Chrome may prompt "Allow remote debugging?" — tell the user to click Allow if needed.

## Step 2: Execute Commands

All commands use the script at `./chrome.js`. Always pass `--agent "Claude Code"` so the badge identifies the controlling agent:

```bash
# List all open tabs
node ./chrome.js --agent "Claude Code" tabs

# Open a new tab
node ./chrome.js open <url>

# Close a tab
node ./chrome.js close <targetId>

# Read page text content
node ./chrome.js read <targetId>

# Take a screenshot (saved as PNG)
node ./chrome.js screenshot <targetId> [output-path]

# Navigate existing tab to new URL
node ./chrome.js navigate <targetId> <url>

# Run JavaScript on a page
node ./chrome.js js <targetId> "<expression>"

# Check which tab is active
node ./chrome.js js <targetId> "document.visibilityState"

# Stop the server
node ./chrome.js stop
```

## Tab IDs

- The `tabs` command shows 8-char short IDs (e.g., `A107EE39`)
- Both short and full 32-char IDs work for all commands
- To find a specific tab, run `tabs` and match by title or URL

## Common Patterns

### Read a URL the user provides
1. Run `tabs` to check if it's already open
2. If not open, use `open <url>`, wait 2 seconds for page load
3. Use `read <targetId>` to get the text content

### Interact with a page (click, fill forms)
1. Use `js <targetId>` with DOM queries to find elements
2. Use `.click()` to click, `.value = '...'` to fill inputs
3. For complex selectors, prefer `document.querySelector('[aria-label="..."]')` or `querySelector('[data-tooltip="..."]')`

### Check what's on screen
1. Use `tabs` to list all tabs
2. Check `document.visibilityState` on candidate tabs to find the active one
3. Use `read` to get text content (preferred over screenshots)

### Read authenticated content (Gmail, X, etc.)
This works because it uses the user's actual browser session with all cookies/auth. Just `read` any tab — no login needed.

## Agent Badge

The bridge injects a small "Controlled by ___" badge on every tab it touches. The agent name is passed via the `agent` field in the request args. When invoking from Claude Code, pass `agent: "Claude Code"`. When invoking from Codex, pass `agent: "Codex"`. Defaults to "Remote Agent" if not specified.

## Tips

- Always prefer `read` over `screenshot` — text is more useful and efficient
- Use `screenshot` only when visual layout matters
- Wait 1-2 seconds after `open` or `navigate` before `read` to let the page load
- For JS expressions with quotes, be careful with shell escaping — use single quotes around the expression
- If the server dies (Chrome closed, etc.), just restart with `serve`
