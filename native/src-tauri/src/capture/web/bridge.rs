//! The local capture bridge: a loopback HTTP endpoint the Relay browser
//! extension posts captures to.
//!
//! ## Why loopback and not native messaging
//!
//! Native messaging is the other supported browser↔desktop channel, and it
//! avoids sockets entirely. It also requires a per-browser registry entry
//! pointing at a second executable that the browser — not Relay — spawns,
//! and an `allowed_origins` list pinned to a specific extension id. Relay is
//! already running when a capture happens, so an in-process listener on
//! `127.0.0.1` is one moving part instead of three. See `docs/capture.md`
//! for the full comparison.
//!
//! ## Threat model
//!
//! The listener is bound to `127.0.0.1` only — never `0.0.0.0` — so nothing
//! off this machine can reach it. That leaves other *local* processes, which
//! is what the pairing token defends against: without it, any program on the
//! machine could write into the user's vault. Every route requires the token,
//! it is compared in constant time, and it is never logged.
//!
//! Browsers additionally enforce CORS on the extension's request, so the
//! responses here name exactly one allowed origin — the paired extension —
//! rather than `*`.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::MAX_PAYLOAD_BYTES;

/// Default port. Chosen high and unregistered; if it is taken, the bridge
/// falls back to an ephemeral port and reports the one it actually got, so a
/// port clash degrades into "check Settings for the port" rather than
/// "capture is broken".
pub const DEFAULT_PORT: u16 = 8765;

/// Ceiling on the request line plus headers. Generous for a real request,
/// small enough that a local process cannot make Relay buffer without bound
/// before authentication has happened.
const MAX_HEADER_BYTES: usize = 16 * 1024;

/// A client that connects and then says nothing holds a worker thread. Ten
/// seconds is far longer than a local POST needs.
const IO_TIMEOUT: Duration = Duration::from_secs(10);

/// Where the bridge writes its port and token so the pairing UI can show them.
pub const BRIDGE_STATE_FILE: &str = "capture-bridge.json";

/// What the pairing UI needs to describe the bridge, and what the extension
/// needs to reach it. The token is included because this file lives in the
/// user's own config directory, next to the settings it already contains.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeState {
    pub enabled: bool,
    pub port: u16,
    pub token: String,
    pub protocol_version: u32,
}

/// Generates a fresh pairing token.
///
/// Two v4 UUIDs' worth of randomness (256 bits) rendered as hex. Relay
/// already depends on `uuid` for ids; a token is not worth a second RNG
/// dependency.
pub fn generate_token() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

/// Compares two secrets without leaking their contents through timing.
pub fn tokens_match(expected: &str, provided: &str) -> bool {
    let expected = expected.as_bytes();
    let provided = provided.as_bytes();
    if expected.is_empty() || expected.len() != provided.len() {
        return false;
    }
    let mut diff = 0u8;
    for (a, b) in expected.iter().zip(provided.iter()) {
        diff |= a ^ b;
    }
    diff == 0
}

/// A parsed HTTP request, reduced to what the bridge actually inspects.
#[derive(Debug, Clone, Default)]
pub struct BridgeRequest {
    pub method: String,
    pub path: String,
    pub origin: Option<String>,
    pub token: Option<String>,
    pub content_length: Option<usize>,
    pub body: Vec<u8>,
}

/// What the bridge decided to do, before any of it touches a socket.
#[derive(Debug, Clone, PartialEq)]
pub enum BridgeOutcome {
    /// A CORS preflight: answer and do nothing else.
    Preflight,
    /// Authenticated liveness check.
    Health,
    /// An authenticated capture, with its payload bytes.
    Capture(Vec<u8>),
    /// Refused, with the status and machine-readable code to return.
    Refused { status: u16, code: &'static str, message: String },
}

#[derive(Debug, Serialize)]
struct ErrorBody<'a> {
    ok: bool,
    code: &'a str,
    message: &'a str,
}

/// Whether a request's `Origin` may talk to the bridge.
///
/// Only browser-extension origins are accepted. A page on the open web
/// cannot forge one of these — the browser sets `Origin` itself — so this
/// keeps a random website from driving the bridge even if it learns the port.
/// A missing `Origin` is allowed because non-browser callers (curl in a
/// support session, an integration test) do not send one and are already
/// gated by the token.
pub fn is_allowed_origin(origin: &str) -> bool {
    origin.starts_with("chrome-extension://")
        || origin.starts_with("moz-extension://")
        || origin.starts_with("extension://")
        || origin.starts_with("safari-web-extension://")
}

/// Decides what to do with a parsed request. Pure, so every refusal path is
/// testable without opening a socket.
pub fn route(request: &BridgeRequest, token: &str) -> BridgeOutcome {
    if let Some(origin) = &request.origin {
        if !is_allowed_origin(origin) {
            return BridgeOutcome::Refused {
                status: 403,
                code: "ORIGIN_NOT_ALLOWED",
                message: "Only the Relay browser extension may use the capture bridge."
                    .to_string(),
            };
        }
    }

    if request.method == "OPTIONS" {
        return BridgeOutcome::Preflight;
    }

    if !tokens_match(token, request.token.as_deref().unwrap_or("")) {
        return BridgeOutcome::Refused {
            status: 401,
            code: "PAIRING_TOKEN_INVALID",
            message: "This capture was not signed with Relay's pairing token. Re-pair the \
                      extension from Relay's Capture settings."
                .to_string(),
        };
    }

    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/v1/health") => BridgeOutcome::Health,
        ("POST", "/v1/capture") => {
            if request.body.len() > MAX_PAYLOAD_BYTES {
                return BridgeOutcome::Refused {
                    status: 413,
                    code: "PAYLOAD_TOO_LARGE",
                    message: format!(
                        "That page is larger than Relay's {} MB capture limit.",
                        MAX_PAYLOAD_BYTES / (1024 * 1024)
                    ),
                };
            }
            BridgeOutcome::Capture(request.body.clone())
        }
        _ => BridgeOutcome::Refused {
            status: 404,
            code: "NO_SUCH_ROUTE",
            message: "Unknown capture bridge route.".to_string(),
        },
    }
}

/// Builds an HTTP response with the CORS headers a browser extension needs.
pub fn build_response(status: u16, origin: Option<&str>, body: &str) -> String {
    let reason = match status {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        413 => "Payload Too Large",
        422 => "Unprocessable Entity",
        _ => "Internal Server Error",
    };

    // Echo exactly the one origin that was allowed rather than `*`: with a
    // wildcard, any extension on the machine that guessed the port could
    // read responses.
    let allow_origin = origin.filter(|o| is_allowed_origin(o)).unwrap_or("null");

    format!(
        "HTTP/1.1 {status} {reason}\r\n\
         Content-Type: application/json; charset=utf-8\r\n\
         Content-Length: {len}\r\n\
         Access-Control-Allow-Origin: {allow_origin}\r\n\
         Access-Control-Allow-Methods: POST, GET, OPTIONS\r\n\
         Access-Control-Allow-Headers: content-type, x-relay-token\r\n\
         Access-Control-Max-Age: 600\r\n\
         Access-Control-Allow-Private-Network: true\r\n\
         Vary: Origin\r\n\
         Cache-Control: no-store\r\n\
         X-Content-Type-Options: nosniff\r\n\
         Connection: close\r\n\
         \r\n\
         {body}",
        status = status,
        reason = reason,
        len = body.len(),
        allow_origin = allow_origin,
        body = body
    )
}

pub fn error_body(code: &str, message: &str) -> String {
    serde_json::to_string(&ErrorBody {
        ok: false,
        code,
        message,
    })
    .unwrap_or_else(|_| r#"{"ok":false,"code":"INTERNAL","message":"error"}"#.to_string())
}

/// Reads and parses one HTTP request, enforcing the header and body caps
/// while reading rather than after.
fn read_request(stream: &TcpStream) -> Result<BridgeRequest, String> {
    let mut reader = BufReader::new(stream);
    let mut request = BridgeRequest::default();
    let mut header_bytes = 0usize;

    let mut line = String::new();
    reader
        .read_line(&mut line)
        .map_err(|e| format!("could not read request line: {e}"))?;
    header_bytes += line.len();

    let mut parts = line.split_whitespace();
    request.method = parts.next().unwrap_or_default().to_uppercase();
    let raw_path = parts.next().unwrap_or_default();
    request.path = raw_path.split(['?', '#']).next().unwrap_or("").to_string();

    loop {
        let mut header = String::new();
        let read = reader
            .read_line(&mut header)
            .map_err(|e| format!("could not read headers: {e}"))?;
        header_bytes += read;
        if header_bytes > MAX_HEADER_BYTES {
            return Err("request headers exceeded the size limit".to_string());
        }
        if read == 0 || header.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            let value = value.trim().to_string();
            match name.trim().to_ascii_lowercase().as_str() {
                "origin" => request.origin = Some(value),
                "x-relay-token" => request.token = Some(value),
                "content-length" => request.content_length = value.parse::<usize>().ok(),
                _ => {}
            }
        }
    }

    if let Some(len) = request.content_length {
        if len > MAX_PAYLOAD_BYTES {
            return Err("declared body exceeds the capture size limit".to_string());
        }
        let mut body = vec![0u8; len];
        reader
            .read_exact(&mut body)
            .map_err(|e| format!("could not read request body: {e}"))?;
        request.body = body;
    }

    Ok(request)
}

/// A running bridge. Dropping the handle does not stop the listener; call
/// [`BridgeHandle::stop`] — which is what `save_settings` does when the user
/// turns capture off.
pub struct BridgeHandle {
    pub port: u16,
    pub token: String,
    stop: Arc<AtomicBool>,
}

impl BridgeHandle {
    /// Signals the accept loop to exit. The loop notices within one accept
    /// timeout, so this returns immediately rather than joining.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }

    pub fn is_running(&self) -> bool {
        !self.stop.load(Ordering::SeqCst)
    }
}

/// Starts the loopback listener.
///
/// `on_capture` runs on a worker thread, off the Tauri main thread and off
/// the async runtime, and returns the JSON body to send back. It is called
/// only for authenticated requests that passed every size and origin check.
pub fn start<F>(preferred_port: u16, token: String, on_capture: F) -> Result<BridgeHandle, String>
where
    F: Fn(&[u8]) -> (u16, String) + Send + Sync + 'static,
{
    let listener = bind(preferred_port)?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("capture bridge could not report its port: {e}"))?
        .port();

    // A blocking accept would never observe the stop flag; a short timeout
    // makes shutdown bounded without a second signalling channel.
    listener
        .set_nonblocking(true)
        .map_err(|e| format!("capture bridge could not configure its listener: {e}"))?;

    let stop = Arc::new(AtomicBool::new(false));
    let handle = BridgeHandle {
        port,
        token: token.clone(),
        stop: stop.clone(),
    };

    let on_capture = Arc::new(on_capture);
    std::thread::Builder::new()
        .name("relay-capture-bridge".to_string())
        .spawn(move || {
            tracing::info!("[Capture] Bridge listening on 127.0.0.1:{}", port);
            while !stop.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _peer)) => {
                        let token = token.clone();
                        let on_capture = on_capture.clone();
                        // One thread per connection so a stalled client
                        // cannot block the next capture. Captures are a
                        // human-paced event; there is no pool to justify.
                        let _ = std::thread::Builder::new()
                            .name("relay-capture-conn".to_string())
                            .spawn(move || handle_connection(stream, &token, on_capture.as_ref()));
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(120));
                    }
                    Err(e) => {
                        tracing::warn!("[Capture] Bridge accept failed: {}", e);
                        std::thread::sleep(Duration::from_millis(500));
                    }
                }
            }
            tracing::info!("[Capture] Bridge on port {} stopped", port);
        })
        .map_err(|e| format!("capture bridge thread could not start: {e}"))?;

    Ok(handle)
}

/// Binds loopback only, falling back to an ephemeral port if the preferred
/// one is taken.
fn bind(preferred_port: u16) -> Result<TcpListener, String> {
    let preferred = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, preferred_port));
    match TcpListener::bind(preferred) {
        Ok(listener) => Ok(listener),
        Err(e) => {
            tracing::warn!(
                "[Capture] Port {} unavailable ({}); falling back to an ephemeral port",
                preferred_port,
                e
            );
            TcpListener::bind(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)))
                .map_err(|e| format!("capture bridge could not bind a loopback port: {e}"))
        }
    }
}

fn handle_connection<F>(stream: TcpStream, token: &str, on_capture: &F)
where
    F: Fn(&[u8]) -> (u16, String),
{
    let _ = stream.set_read_timeout(Some(IO_TIMEOUT));
    let _ = stream.set_write_timeout(Some(IO_TIMEOUT));

    let mut write_stream = match stream.try_clone() {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("[Capture] Bridge could not clone its stream: {}", e);
            return;
        }
    };

    let request = match read_request(&stream) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("[Capture] Rejected a malformed bridge request: {}", e);
            let body = error_body("BAD_REQUEST", "Malformed capture request.");
            let _ = write_stream.write_all(build_response(400, None, &body).as_bytes());
            return;
        }
    };

    let origin = request.origin.clone();
    let (status, body) = match route(&request, token) {
        BridgeOutcome::Preflight => (204, String::new()),
        BridgeOutcome::Health => (
            200,
            serde_json::json!({
                "ok": true,
                "app": "relay",
                "protocol_version": super::PROTOCOL_VERSION,
                "version": env!("CARGO_PKG_VERSION"),
            })
            .to_string(),
        ),
        BridgeOutcome::Capture(bytes) => on_capture(&bytes),
        BridgeOutcome::Refused {
            status,
            code,
            message,
        } => {
            // Logged without the token or the body: a refusal is worth
            // knowing about, its contents are not worth writing to a log file.
            tracing::warn!(
                "[Capture] Bridge refused {} {} — {}",
                request.method,
                request.path,
                code
            );
            (status, error_body(code, &message))
        }
    };

    let _ = write_stream.write_all(build_response(status, origin.as_deref(), &body).as_bytes());
    let _ = write_stream.flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = "0123456789abcdef";

    fn request(method: &str, path: &str) -> BridgeRequest {
        BridgeRequest {
            method: method.to_string(),
            path: path.to_string(),
            origin: Some("chrome-extension://abcdefghijklmnop".to_string()),
            token: Some(TOKEN.to_string()),
            content_length: None,
            body: Vec::new(),
        }
    }

    #[test]
    fn tokens_compare_by_full_length_only() {
        assert!(tokens_match(TOKEN, TOKEN));
        assert!(!tokens_match(TOKEN, "0123456789abcde"));
        assert!(!tokens_match(TOKEN, "0123456789abcdeg"));
        assert!(!tokens_match(TOKEN, ""));
        // An unset token must never match an unset header.
        assert!(!tokens_match("", ""));
    }

    #[test]
    fn generated_tokens_are_long_and_unique() {
        let a = generate_token();
        let b = generate_token();
        assert_eq!(a.len(), 64);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }

    #[test]
    fn a_capture_needs_the_pairing_token() {
        let mut req = request("POST", "/v1/capture");
        req.token = Some("wrong".to_string());
        assert!(matches!(
            route(&req, TOKEN),
            BridgeOutcome::Refused {
                code: "PAIRING_TOKEN_INVALID",
                ..
            }
        ));

        req.token = None;
        assert!(matches!(
            route(&req, TOKEN),
            BridgeOutcome::Refused {
                code: "PAIRING_TOKEN_INVALID",
                ..
            }
        ));
    }

    #[test]
    fn health_also_needs_the_token() {
        let mut req = request("GET", "/v1/health");
        assert!(matches!(route(&req, TOKEN), BridgeOutcome::Health));
        req.token = None;
        assert!(matches!(
            route(&req, TOKEN),
            BridgeOutcome::Refused { status: 401, .. }
        ));
    }

    #[test]
    fn a_web_page_origin_is_refused_before_the_token_is_even_checked() {
        let mut req = request("POST", "/v1/capture");
        req.origin = Some("https://evil.example".to_string());
        req.token = Some(TOKEN.to_string());
        assert!(matches!(
            route(&req, TOKEN),
            BridgeOutcome::Refused {
                status: 403,
                code: "ORIGIN_NOT_ALLOWED",
                ..
            }
        ));
    }

    #[test]
    fn extension_origins_are_recognised_across_browsers() {
        assert!(is_allowed_origin("chrome-extension://aaaa"));
        assert!(is_allowed_origin("moz-extension://bbbb"));
        assert!(is_allowed_origin("safari-web-extension://cccc"));
        assert!(!is_allowed_origin("https://chrome-extension.example"));
        assert!(!is_allowed_origin("http://127.0.0.1:8765"));
    }

    #[test]
    fn preflight_is_answered_without_a_token() {
        let mut req = request("OPTIONS", "/v1/capture");
        req.token = None;
        assert_eq!(route(&req, TOKEN), BridgeOutcome::Preflight);
    }

    #[test]
    fn oversized_bodies_are_refused_before_parsing() {
        let mut req = request("POST", "/v1/capture");
        req.body = vec![b'x'; MAX_PAYLOAD_BYTES + 1];
        assert!(matches!(
            route(&req, TOKEN),
            BridgeOutcome::Refused {
                status: 413,
                code: "PAYLOAD_TOO_LARGE",
                ..
            }
        ));
    }

    #[test]
    fn unknown_routes_are_not_found() {
        assert!(matches!(
            route(&request("GET", "/v1/vault"), TOKEN),
            BridgeOutcome::Refused { status: 404, .. }
        ));
    }

    #[test]
    fn responses_never_wildcard_the_allowed_origin() {
        let allowed = build_response(200, Some("chrome-extension://abc"), "{}");
        assert!(allowed.contains("Access-Control-Allow-Origin: chrome-extension://abc"));

        let hostile = build_response(200, Some("https://evil.example"), "{}");
        assert!(hostile.contains("Access-Control-Allow-Origin: null"));
        assert!(!hostile.contains('*'));

        let anonymous = build_response(200, None, "{}");
        assert!(anonymous.contains("Access-Control-Allow-Origin: null"));
    }

    #[test]
    fn responses_declare_their_length_and_refuse_sniffing() {
        let response = build_response(200, None, r#"{"ok":true}"#);
        assert!(response.contains("Content-Length: 11"));
        assert!(response.contains("X-Content-Type-Options: nosniff"));
        assert!(response.starts_with("HTTP/1.1 200 OK"));
    }

    #[test]
    fn the_bridge_binds_loopback_and_accepts_a_real_capture() {
        use std::io::Write as _;
        use std::net::TcpStream;

        let token = generate_token();
        let handle = start(0, token.clone(), |bytes| {
            (200, format!(r#"{{"ok":true,"bytes":{}}}"#, bytes.len()))
        })
        .expect("bridge should start");

        let mut stream =
            TcpStream::connect(("127.0.0.1", handle.port)).expect("bridge should accept loopback");
        let body = r#"{"protocol_version":1}"#;
        let request = format!(
            "POST /v1/capture HTTP/1.1\r\nHost: 127.0.0.1\r\nOrigin: chrome-extension://abc\r\n\
             X-Relay-Token: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            token,
            body.len(),
            body
        );
        stream.write_all(request.as_bytes()).unwrap();
        stream.flush().unwrap();

        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert!(response.starts_with("HTTP/1.1 200 OK"), "got: {response}");
        assert!(response.contains(r#""bytes":22"#));

        handle.stop();
    }

    #[test]
    fn the_bridge_rejects_an_unauthenticated_local_process() {
        use std::io::Write as _;
        use std::net::TcpStream;

        let handle = start(0, generate_token(), |_| (200, "{}".to_string())).unwrap();
        let mut stream = TcpStream::connect(("127.0.0.1", handle.port)).unwrap();
        stream
            .write_all(b"GET /v1/health HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
            .unwrap();

        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert!(response.starts_with("HTTP/1.1 401"), "got: {response}");
        assert!(response.contains("PAIRING_TOKEN_INVALID"));

        handle.stop();
    }
}
