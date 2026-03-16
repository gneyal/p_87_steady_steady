use base64::Engine;
use clap::{Parser, Subcommand};
use futures_util::{SinkExt, StreamExt};
use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{Mutex, oneshot};
use tokio_tungstenite::tungstenite::Message;

const SERVER_PORT: u16 = 9333;
const PID_FILE: &str = "/tmp/chrome-cdp-server.pid";
const TIMEOUT_MS: u64 = 15000;

// --- CDP Connection ---

struct CdpConnection {
    sender: futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        Message,
    >,
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Value>>>>,
    msg_id: AtomicU64,
    sessions: HashMap<String, String>, // targetId -> sessionId
}

impl CdpConnection {
    async fn new() -> Result<Arc<Mutex<Self>>, Box<dyn std::error::Error + Send + Sync>> {
        let (port, path) = get_endpoint()?;
        let url = format!("ws://127.0.0.1:{}{}", port, path);

        let (ws, _) = tokio_tungstenite::connect_async(&url).await?;
        let (sender, mut receiver) = ws.split();

        let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Value>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let pending_clone = pending.clone();

        // Spawn reader task
        tokio::spawn(async move {
            while let Some(Ok(msg)) = receiver.next().await {
                if let Message::Text(text) = msg {
                    if let Ok(parsed) = serde_json::from_str::<Value>(&text) {
                        if let Some(id) = parsed.get("id").and_then(|v| v.as_u64()) {
                            let mut map = pending_clone.lock().await;
                            if let Some(tx) = map.remove(&id) {
                                let _ = tx.send(parsed);
                            }
                        }
                    }
                }
            }
            eprintln!("CDP disconnected");
            std::process::exit(1);
        });

        eprintln!("Connected to Chrome CDP");

        Ok(Arc::new(Mutex::new(Self {
            sender,
            pending,
            msg_id: AtomicU64::new(0),
            sessions: HashMap::new(),
        })))
    }

    async fn send(
        &mut self,
        method: &str,
        params: Value,
        session_id: Option<&str>,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let id = self.msg_id.fetch_add(1, Ordering::SeqCst) + 1;

        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id, tx);

        let mut msg = json!({ "id": id, "method": method, "params": params });
        if let Some(sid) = session_id {
            msg["sessionId"] = json!(sid);
        }

        self.sender.send(Message::Text(msg.to_string())).await?;

        let result = tokio::time::timeout(std::time::Duration::from_millis(TIMEOUT_MS), rx)
            .await
            .map_err(|_| "CDP timeout")?
            .map_err(|_| "Channel closed")?;

        Ok(result)
    }

    async fn get_session(
        &mut self,
        target_id: &str,
        agent: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(sid) = self.sessions.get(target_id) {
            let sid = sid.clone();
            let _ = self
                .send(
                    "Runtime.evaluate",
                    json!({ "expression": badge_js(agent) }),
                    Some(&sid),
                )
                .await;
            return Ok(sid);
        }

        let msg = self
            .send(
                "Target.attachToTarget",
                json!({ "targetId": target_id, "flatten": true }),
                None,
            )
            .await?;
        let sid = msg["result"]["sessionId"]
            .as_str()
            .ok_or("No sessionId")?
            .to_string();
        self.sessions.insert(target_id.to_string(), sid.clone());

        let _ = self
            .send(
                "Runtime.evaluate",
                json!({ "expression": badge_js(agent) }),
                Some(&sid),
            )
            .await;

        Ok(sid)
    }
}

fn get_endpoint() -> Result<(String, String), Box<dyn std::error::Error + Send + Sync>> {
    let home = std::env::var("HOME")?;
    let port_file = format!(
        "{}/Library/Application Support/Google/Chrome/DevToolsActivePort",
        home
    );
    let content = fs::read_to_string(&port_file).map_err(|_| {
        "Cannot read DevToolsActivePort. Enable remote debugging in chrome://inspect/#remote-debugging"
    })?;
    let lines: Vec<&str> = content.trim().split('\n').collect();
    if lines.len() < 2 {
        return Err("Invalid DevToolsActivePort format".into());
    }
    Ok((lines[0].to_string(), lines[1].to_string()))
}

fn badge_js(agent: &str) -> String {
    let template = r##"if(!document.getElementById('steady-steady-cursor')){
var c=document.createElement('div');c.id='steady-steady-cursor';
c.innerHTML='<svg width="16" height="22" viewBox="0 0 16 22" fill="none" style="filter:drop-shadow(0 1px 2px rgba(0,0,0,0.3))"><path d="M0 0L16 12H7.5L4 22L0 0Z" fill="#e94560"/><path d="M1.5 2.5L13.5 11.5H7.2L3.8 20L1.5 2.5Z" fill="#ff6b81"/></svg><span style="margin-left:6px;background:#e94560;color:#fff;padding:3px 8px;border-radius:4px;font-family:-apple-system,system-ui,sans-serif;font-size:11px;font-weight:600;white-space:nowrap;box-shadow:0 2px 8px rgba(233,69,96,0.4)">AGENT_NAME</span>';
c.style.cssText='position:fixed;bottom:80px;right:30px;z-index:999999;pointer-events:none;display:flex;align-items:flex-start;transition:all 0.3s cubic-bezier(0.4,0,0.2,1);animation:ss-float 3s ease-in-out infinite';
document.body.appendChild(c);
if(!document.getElementById('ss-cursor-style')){var s=document.createElement('style');s.id='ss-cursor-style';s.textContent='@keyframes ss-float{0%,100%{transform:translate(0,0)}50%{transform:translate(-3px,-6px)}}';document.head.appendChild(s)}
window.__ssCursor=c;window.__ssMoveTo=function(x,y){c.style.transition="all 0.5s cubic-bezier(0.4,0,0.2,1)";c.style.animation="none";c.style.left=x+"px";c.style.top=y+"px";c.style.right="auto";c.style.bottom="auto";clearTimeout(window.__ssIdle);window.__ssIdle=setTimeout(function(){c.style.animation="ss-float 3s ease-in-out infinite"},2000)}}"##;
    template.replace("AGENT_NAME", agent)
}

const MOVE_CURSOR_JS: &str = r#"(function(){var el=document.activeElement;if(!el||!window.__ssMoveTo)return;var r=el.getBoundingClientRect();if(r.width>0&&r.height>0)window.__ssMoveTo(r.left+r.width/2,r.top-20)})()"#;

async fn move_cursor_to_active(cdp: &mut CdpConnection, session_id: &str) {
    let _ = cdp.send("Runtime.evaluate", json!({ "expression": MOVE_CURSOR_JS }), Some(session_id)).await;
}

async fn resolve_target_id(
    cdp: &mut CdpConnection,
    partial: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let msg = cdp.send("Target.getTargets", json!({}), None).await?;
    let targets = msg["result"]["targetInfos"]
        .as_array()
        .ok_or("No targets")?;
    for t in targets {
        if let Some(id) = t["targetId"].as_str() {
            if id.starts_with(partial) {
                return Ok(id.to_string());
            }
        }
    }
    Err(format!("No target found for: {}", partial).into())
}

// --- Actions ---

#[derive(Deserialize)]
struct ActionRequest {
    action: String,
    #[serde(default)]
    args: HashMap<String, Value>,
}

fn str_arg(args: &HashMap<String, Value>, key: &str) -> Option<String> {
    args.get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
}

async fn handle_action(
    cdp: &mut CdpConnection,
    action: &str,
    args: &mut HashMap<String, Value>,
) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
    let agent = str_arg(args, "agent").unwrap_or_else(|| "Remote Agent".to_string());

    // Resolve partial target IDs
    if let Some(tid) = str_arg(args, "targetId") {
        if tid.len() < 32 {
            let full = resolve_target_id(cdp, &tid).await?;
            args.insert("targetId".to_string(), json!(full));
        }
    }

    match action {
        "tabs" => {
            let msg = cdp.send("Target.getTargets", json!({}), None).await?;
            let targets = msg["result"]["targetInfos"]
                .as_array()
                .ok_or("No targets")?;
            let tabs: Vec<Value> = targets
                .iter()
                .filter(|t| t["type"].as_str() == Some("page"))
                .map(|t| {
                    let id = t["targetId"].as_str().unwrap_or("");
                    json!({
                        "id": &id[..8.min(id.len())],
                        "fullId": id,
                        "title": t["title"],
                        "url": t["url"]
                    })
                })
                .collect();
            Ok(json!(tabs))
        }
        "open" => {
            let url = str_arg(args, "url").ok_or("Missing url")?;
            let msg = cdp
                .send("Target.createTarget", json!({ "url": url }), None)
                .await?;
            Ok(json!({ "opened": url, "targetId": msg["result"]["targetId"] }))
        }
        "close" => {
            let tid = str_arg(args, "targetId").ok_or("Missing targetId")?;
            cdp.send("Target.closeTarget", json!({ "targetId": tid }), None)
                .await?;
            Ok(json!({ "closed": tid }))
        }
        "read" => {
            let tid = str_arg(args, "targetId").ok_or("Missing targetId")?;
            let sid = cdp.get_session(&tid, &agent).await?;
            let msg = cdp
                .send(
                    "Runtime.evaluate",
                    json!({ "expression": "document.body.innerText", "returnByValue": true }),
                    Some(&sid),
                )
                .await?;
            Ok(json!({ "text": msg["result"]["result"]["value"] }))
        }
        "navigate" => {
            let tid = str_arg(args, "targetId").ok_or("Missing targetId")?;
            let url = str_arg(args, "url").ok_or("Missing url")?;
            let sid = cdp.get_session(&tid, &agent).await?;
            cdp.send("Page.navigate", json!({ "url": url }), Some(&sid))
                .await?;
            Ok(json!({ "navigated": url }))
        }
        "screenshot" => {
            let tid = str_arg(args, "targetId").ok_or("Missing targetId")?;
            let out_file =
                str_arg(args, "outFile").unwrap_or_else(|| "/tmp/screenshot.png".into());
            let sid = cdp.get_session(&tid, &agent).await?;
            let msg = cdp
                .send(
                    "Page.captureScreenshot",
                    json!({ "format": "png" }),
                    Some(&sid),
                )
                .await?;
            let b64 = msg["result"]["data"]
                .as_str()
                .ok_or("No screenshot data")?;
            let bytes = base64::engine::general_purpose::STANDARD.decode(b64)?;
            fs::write(&out_file, bytes)?;
            Ok(json!({ "saved": out_file }))
        }
        "js" => {
            let tid = str_arg(args, "targetId").ok_or("Missing targetId")?;
            let expr = str_arg(args, "expression").ok_or("Missing expression")?;
            let sid = cdp.get_session(&tid, &agent).await?;
            move_cursor_to_active(cdp, &sid).await;
            let msg = cdp
                .send(
                    "Runtime.evaluate",
                    json!({ "expression": expr, "returnByValue": true }),
                    Some(&sid),
                )
                .await?;
            Ok(msg["result"]["result"].clone())
        }
        "clear" => {
            let tid = str_arg(args, "targetId").ok_or("Missing targetId")?;
            let sid = cdp.get_session(&tid, &agent).await?;
            move_cursor_to_active(cdp, &sid).await;
            cdp.send(
                "Input.dispatchKeyEvent",
                json!({ "type": "rawKeyDown", "key": "a", "code": "KeyA", "commands": ["selectAll"] }),
                Some(&sid),
            )
            .await?;
            cdp.send(
                "Input.dispatchKeyEvent",
                json!({ "type": "keyUp", "key": "a", "code": "KeyA" }),
                Some(&sid),
            )
            .await?;
            cdp.send(
                "Input.dispatchKeyEvent",
                json!({ "type": "rawKeyDown", "key": "Backspace", "code": "Backspace", "windowsVirtualKeyCode": 8 }),
                Some(&sid),
            )
            .await?;
            cdp.send(
                "Input.dispatchKeyEvent",
                json!({ "type": "keyUp", "key": "Backspace", "code": "Backspace", "windowsVirtualKeyCode": 8 }),
                Some(&sid),
            )
            .await?;
            Ok(json!({ "cleared": true }))
        }
        "insert" => {
            let tid = str_arg(args, "targetId").ok_or("Missing targetId")?;
            let text = str_arg(args, "text").ok_or("Missing text")?;
            let sid = cdp.get_session(&tid, &agent).await?;
            move_cursor_to_active(cdp, &sid).await;
            cdp.send("Input.insertText", json!({ "text": text }), Some(&sid))
                .await?;
            Ok(json!({ "inserted": format!("{} chars", text.len()) }))
        }
        "type" => {
            let tid = str_arg(args, "targetId").ok_or("Missing targetId")?;
            let text = str_arg(args, "text").ok_or("Missing text")?;
            let sid = cdp.get_session(&tid, &agent).await?;
            move_cursor_to_active(cdp, &sid).await;
            for ch in text.chars() {
                if ch == '\n' {
                    cdp.send("Input.dispatchKeyEvent", json!({ "type": "rawKeyDown", "key": "Enter", "code": "Enter", "windowsVirtualKeyCode": 13, "nativeVirtualKeyCode": 13 }), Some(&sid)).await?;
                    cdp.send(
                        "Input.dispatchKeyEvent",
                        json!({ "type": "char", "text": "\r" }),
                        Some(&sid),
                    )
                    .await?;
                    cdp.send("Input.dispatchKeyEvent", json!({ "type": "keyUp", "key": "Enter", "code": "Enter", "windowsVirtualKeyCode": 13, "nativeVirtualKeyCode": 13 }), Some(&sid)).await?;
                } else {
                    let c = ch.to_string();
                    cdp.send(
                        "Input.dispatchKeyEvent",
                        json!({ "type": "rawKeyDown", "key": &c }),
                        Some(&sid),
                    )
                    .await?;
                    cdp.send(
                        "Input.dispatchKeyEvent",
                        json!({ "type": "char", "text": &c }),
                        Some(&sid),
                    )
                    .await?;
                    cdp.send(
                        "Input.dispatchKeyEvent",
                        json!({ "type": "keyUp", "key": &c }),
                        Some(&sid),
                    )
                    .await?;
                }
            }
            Ok(json!({ "typed": format!("{} chars", text.len()) }))
        }
        _ => Err(format!("Unknown action: {}", action).into()),
    }
}

// --- Server Mode ---

async fn start_server() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cdp = CdpConnection::new().await?;

    let addr: SocketAddr = ([127, 0, 0, 1], SERVER_PORT).into();
    let listener = TcpListener::bind(addr).await?;

    fs::write(PID_FILE, std::process::id().to_string())?;
    eprintln!(
        "CDP server listening on http://127.0.0.1:{}",
        SERVER_PORT
    );

    // Handle ctrl-c
    tokio::spawn(async {
        tokio::signal::ctrl_c().await.ok();
        let _ = fs::remove_file(PID_FILE);
        std::process::exit(0);
    });

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let cdp = cdp.clone();

        tokio::spawn(async move {
            let service = service_fn(move |req: Request<Incoming>| {
                let cdp = cdp.clone();
                async move {
                    if req.method() != Method::POST {
                        return Ok::<_, hyper::Error>(
                            Response::builder()
                                .status(StatusCode::METHOD_NOT_ALLOWED)
                                .body(Full::new(Bytes::new()))
                                .unwrap(),
                        );
                    }

                    let body = req.collect().await.unwrap().to_bytes();
                    let body_str = String::from_utf8_lossy(&body);

                    let response = match serde_json::from_str::<ActionRequest>(&body_str) {
                        Ok(mut action_req) => {
                            let mut cdp = cdp.lock().await;
                            match handle_action(
                                &mut cdp,
                                &action_req.action,
                                &mut action_req.args,
                            )
                            .await
                            {
                                Ok(result) => json!({ "ok": true, "result": result }),
                                Err(e) => json!({ "ok": false, "error": e.to_string() }),
                            }
                        }
                        Err(e) => json!({ "ok": false, "error": e.to_string() }),
                    };

                    Ok(Response::builder()
                        .header("Content-Type", "application/json")
                        .body(Full::new(Bytes::from(response.to_string())))
                        .unwrap())
                }
            });

            if let Err(e) = http1::Builder::new().serve_connection(io, service).await {
                eprintln!("Connection error: {}", e);
            }
        });
    }
}

// --- Client Mode ---

async fn client_request(
    action: &str,
    args: Value,
) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
    let body = json!({ "action": action, "args": args }).to_string();

    let client =
        hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
            .build_http();

    let req = Request::builder()
        .method(Method::POST)
        .uri(format!("http://127.0.0.1:{}", SERVER_PORT))
        .header("Content-Type", "application/json")
        .body(Full::new(Bytes::from(body)))
        .unwrap();

    let res = client
        .request(req)
        .await
        .map_err(|_| "Server not running? Start with: steady serve")?;

    let body = res.collect().await?.to_bytes();
    let parsed: Value = serde_json::from_slice(&body)?;

    if parsed["ok"].as_bool() != Some(true) {
        return Err(parsed["error"]
            .as_str()
            .unwrap_or("Unknown error")
            .into());
    }

    Ok(parsed["result"].clone())
}

// --- CLI ---

#[derive(Parser)]
#[command(name = "steady", about = "Chrome browser bridge for AI agents")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Agent name for the badge
    #[arg(long, global = true)]
    agent: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the CDP bridge server
    Serve,
    /// List all open Chrome tabs
    Tabs,
    /// Open a new tab
    Open { url: String },
    /// Close a tab
    Close { target_id: String },
    /// Read page text content
    Read { target_id: String },
    /// Navigate a tab to a new URL
    Navigate { target_id: String, url: String },
    /// Take a screenshot
    Screenshot {
        target_id: String,
        out_file: Option<String>,
    },
    /// Run JavaScript on a page
    Js {
        target_id: String,
        expression: Vec<String>,
    },
    /// Type text using keyboard simulation
    Type {
        target_id: String,
        text: Vec<String>,
    },
    /// Clear the focused input
    Clear { target_id: String },
    /// Insert text directly (no keyboard sim)
    Insert {
        target_id: String,
        text: Vec<String>,
    },
    /// Stop the server
    Stop,
}

type BoxError = Box<dyn std::error::Error + Send + Sync>;

async fn run() -> Result<(), BoxError> {
    let cli = Cli::parse();
    let agent = cli.agent.clone();

    match cli.command {
        Commands::Serve => start_server().await?,
        Commands::Tabs => {
            let result = client_request("tabs", json!({ "agent": agent })).await?;
            if let Some(tabs) = result.as_array() {
                for t in tabs {
                    println!(
                        "{}  {}  |  {}",
                        t["id"].as_str().unwrap_or(""),
                        t["title"].as_str().unwrap_or(""),
                        t["url"].as_str().unwrap_or("")
                    );
                }
            }
        }
        Commands::Open { url } => {
            let r = client_request("open", json!({ "url": url, "agent": agent })).await?;
            println!("Opened: {} | targetId: {}", r["opened"], r["targetId"]);
        }
        Commands::Close { target_id } => {
            let r =
                client_request("close", json!({ "targetId": target_id, "agent": agent })).await?;
            println!("Closed: {}", r["closed"]);
        }
        Commands::Read { target_id } => {
            let r =
                client_request("read", json!({ "targetId": target_id, "agent": agent })).await?;
            println!("{}", r["text"].as_str().unwrap_or(""));
        }
        Commands::Navigate { target_id, url } => {
            let r = client_request(
                "navigate",
                json!({ "targetId": target_id, "url": url, "agent": agent }),
            )
            .await?;
            println!("Navigated to: {}", r["navigated"]);
        }
        Commands::Screenshot {
            target_id,
            out_file,
        } => {
            let mut args = json!({ "targetId": target_id, "agent": agent });
            if let Some(f) = out_file {
                args["outFile"] = json!(f);
            }
            let r = client_request("screenshot", args).await?;
            println!("Saved screenshot to: {}", r["saved"]);
        }
        Commands::Js {
            target_id,
            expression,
        } => {
            let expr = expression.join(" ");
            let r = client_request(
                "js",
                json!({ "targetId": target_id, "expression": expr, "agent": agent }),
            )
            .await?;
            println!("{}", serde_json::to_string_pretty(&r)?);
        }
        Commands::Type { target_id, text } => {
            let t = text.join(" ");
            let r = client_request(
                "type",
                json!({ "targetId": target_id, "text": t, "agent": agent }),
            )
            .await?;
            println!("Typed: {}", r["typed"]);
        }
        Commands::Clear { target_id } => {
            client_request("clear", json!({ "targetId": target_id, "agent": agent })).await?;
            println!("Cleared");
        }
        Commands::Insert { target_id, text } => {
            let t = text.join(" ");
            let r = client_request(
                "insert",
                json!({ "targetId": target_id, "text": t, "agent": agent }),
            )
            .await?;
            println!("Inserted: {}", r["inserted"]);
        }
        Commands::Stop => {
            let pid = fs::read_to_string(PID_FILE)
                .map_err(|_| -> BoxError { "No PID file — server not running?".into() })?;
            let pid: u32 = pid.trim().parse()?;
            // Use nix-style kill via command since we don't need the libc crate just for this
            std::process::Command::new("kill")
                .arg(pid.to_string())
                .output()?;
            println!("Server stopped");
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}
