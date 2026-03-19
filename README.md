# steady-steady

Chrome browser bridge for AI agents.

`steady-steady` connects an agent to your local Chrome session through the Chrome DevTools Protocol (CDP). It gives you a small local server plus a CLI for opening tabs, reading pages, navigating, running JavaScript, typing into focused elements, and taking screenshots. It also injects a lightweight on-page badge so you can see which agent is interacting with the browser.

The repo currently contains three main pieces:

- A Rust implementation of the bridge and CLI in [`src/main.rs`](/Users/eyalgoren/Code/p_87_steady_steady/src/main.rs)
- A Node-based MCP server in [`mcp-server.js`](/Users/eyalgoren/Code/p_87_steady_steady/mcp-server.js)
- An older Node CLI/server implementation in [`chrome.cjs`](/Users/eyalgoren/Code/p_87_steady_steady/chrome.cjs)

## What it can do

- List open Chrome tabs
- Open and close tabs
- Navigate an existing tab
- Read visible page text
- Run arbitrary JavaScript in a page
- Type, clear, and insert text in focused elements
- Capture screenshots
- Show an agent badge inside the page

## Prerequisites

- macOS with Google Chrome
- Rust toolchain (`cargo`)
- Node.js, if you want to use the MCP server or legacy JS scripts

This project reads Chrome's DevTools endpoint from:

`~/Library/Application Support/Google/Chrome/DevToolsActivePort`

That file exists only when Chrome remote debugging is enabled. If it is missing, enable remote debugging in `chrome://inspect/#remote-debugging`.

## Quick Start

Start the Rust bridge server:

```bash
cargo run -- serve
```

In another terminal, use the CLI:

```bash
cargo run -- tabs
cargo run -- open https://example.com
cargo run -- read <target-id>
```

You can also build the binary once and run it directly:

```bash
cargo build
./target/debug/steady tabs
```

## Rust CLI

The Rust binary is named `steady`.

```bash
steady --help
```

Available commands:

- `steady serve`
- `steady tabs`
- `steady open <url>`
- `steady close <target-id>`
- `steady read <target-id>`
- `steady navigate <target-id> <url>`
- `steady screenshot <target-id> [out-file]`
- `steady js <target-id> <expression>`
- `steady type <target-id> <text...>`
- `steady clear <target-id>`
- `steady insert <target-id> <text...>`
- `steady stop`

You can pass a custom badge name globally:

```bash
steady --agent "Codex" tabs
steady --agent "Codex" type abcd1234 "hello from steady-steady"
```

Short tab IDs from `steady tabs` are accepted anywhere a `target-id` is required.

## MCP Server

The MCP server exposes the browser bridge as MCP tools over stdio.

Start it with:

```bash
node mcp-server.js
```

Tools currently exposed:

- `browser_tabs`
- `browser_open`
- `browser_close`
- `browser_read`
- `browser_navigate`
- `browser_screenshot`
- `browser_js`
- `browser_type`
- `browser_clear`
- `browser_insert`

Example MCP config:

```json
{
  "mcpServers": {
    "steady-steady": {
      "command": "node",
      "args": ["/absolute/path/to/p_87_steady_steady/mcp-server.js"]
    }
  }
}
```

There is a local example in [`.mcp.json`](/Users/eyalgoren/Code/p_87_steady_steady/.mcp.json).

## Node Dependencies

There is currently no root `package.json` in this repo, even though the MCP server and legacy JS implementation depend on Node packages already present in `node_modules/`.

If you want to recreate that setup cleanly, install the needed packages in the repo root:

```bash
npm install ws @modelcontextprotocol/sdk zod
```

If you want this to be reproducible for others, adding a root `package.json` would be a good next cleanup step.

## Legacy Node CLI

The older JS implementation lives in [`chrome.cjs`](/Users/eyalgoren/Code/p_87_steady_steady/chrome.cjs), with a tiny launcher in [`cli.js`](/Users/eyalgoren/Code/p_87_steady_steady/cli.js).

Examples:

```bash
node chrome.cjs serve
node chrome.cjs tabs
node chrome.cjs open https://example.com
node chrome.cjs stop
```

The Rust implementation appears to be the primary path now, but the JS version is still useful as a reference and fallback.

## Repo Layout

- [`src/main.rs`](/Users/eyalgoren/Code/p_87_steady_steady/src/main.rs): Rust bridge server and CLI
- [`src/badge.js`](/Users/eyalgoren/Code/p_87_steady_steady/src/badge.js): injected browser badge/chat UI
- [`mcp-server.js`](/Users/eyalgoren/Code/p_87_steady_steady/mcp-server.js): MCP adapter
- [`chrome.cjs`](/Users/eyalgoren/Code/p_87_steady_steady/chrome.cjs): legacy Node bridge
- [`demo-video/`](/Users/eyalgoren/Code/p_87_steady_steady/demo-video): Remotion demo project and rendered outputs

## Demo Video

The `demo-video` folder contains a separate Remotion project used to render a short product/demo video.

From inside [`demo-video/package.json`](/Users/eyalgoren/Code/p_87_steady_steady/demo-video/package.json):

```bash
cd demo-video
npm install
npm run studio
npm run render
```

Rendered outputs currently live in `demo-video/out/`.

## Notes

- The bridge server listens on `127.0.0.1:9333`
- The server PID file is stored at `/tmp/chrome-cdp-server.pid`
- Screenshots default to `/tmp/screenshot.png`
- If Chrome disconnects, the bridge exits and should be restarted

## License

`Cargo.toml` declares this project as `MIT`.
