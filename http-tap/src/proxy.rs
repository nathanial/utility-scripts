use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::Context as _;
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::http::{HeaderMap, HeaderValue, Method, Request, Response, StatusCode, Uri};
use hyper::service::service_fn;
use hyper::Error as HyperError;
use hyper_util::client::legacy::{connect::HttpConnector, Client};
use hyper_util::rt::{TokioExecutor, TokioIo};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

#[derive(Clone)]
pub struct Config {
    pub listen: SocketAddr,
    pub target_authority: String, // host:port
    pub target_scheme: &'static str, // "http"
    pub include_bodies: bool,
    pub max_body_bytes: usize,
    pub redact_header: Vec<String>,
}

pub async fn run_proxy(cfg: Config) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(cfg.listen)
        .await
        .with_context(|| format!("bind {}", cfg.listen))?;

    let client = {
        let mut http = HttpConnector::new();
        http.enforce_http(true); // HTTP only
        Client::builder(TokioExecutor::new()).build::<_, Full<Bytes>>(http)
    };

    let shared = Arc::new(ProxyState::new(cfg, client));

    eprintln!(
        "us-http-tap listening on http://{} → http://{}",
        shared.cfg.listen, shared.cfg.target_authority
    );

    loop {
        let (stream, addr) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let state = shared.clone();
        tokio::spawn(async move {
            let conn_id = state.next_conn_id();
            let svc = service_fn(move |req| handle(state.clone(), conn_id, addr, req));
            if let Err(err) = hyper::server::conn::http1::Builder::new()
                .serve_connection(io, svc)
                .await
            {
                eprintln!("[conn#{conn_id}] connection error: {err}");
            }
        });
    }
}

#[derive(Clone)]
struct ProxyState {
    cfg: Config,
    client: Client<HttpConnector, Full<Bytes>>,
    conn_seq: Arc<AtomicU64>,
}

impl ProxyState {
    fn new(cfg: Config, client: Client<HttpConnector, Full<Bytes>>) -> Self {
        Self {
            cfg,
            client,
            conn_seq: Arc::new(AtomicU64::new(1)),
        }
    }
    fn next_conn_id(&self) -> u64 {
        self.conn_seq.fetch_add(1, Ordering::Relaxed)
    }
}

async fn handle(
    state: Arc<ProxyState>,
    conn_id: u64,
    peer: SocketAddr,
    req: Request<Incoming>,
) -> Result<Response<Full<Bytes>>, HyperError> {
    let now = now_iso();

    let (req_parts, req_body_incoming) = req.into_parts();
    let req_bytes = match req_body_incoming.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(e) => {
            eprintln!("[conn#{conn_id}] {now} request body error: {e}");
            return Ok(simple_response(StatusCode::BAD_REQUEST, "body error"));
        }
    };

    let mut forwarded = Request::builder()
        .method(req_parts.method.clone())
        .version(req_parts.version)
        .uri(remap_uri(&req_parts.method, &req_parts.uri, &state.cfg))
        .body(Full::new(req_bytes.clone()))
        .expect("build request");

    copy_headers_forward(req_parts.headers, forwarded.headers_mut(), &state.cfg);

    log_request(&state.cfg, conn_id, &peer, &forwarded, &req_bytes, &now);

    let resp = match state.client.request(forwarded).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[conn#{conn_id}] {now} upstream error: {e}");
            return Ok(simple_response(
                StatusCode::BAD_GATEWAY,
                "upstream connection failed",
            ));
        }
    };

    let (resp_parts, resp_body_incoming) = resp.into_parts();
    let resp_bytes = match resp_body_incoming.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(e) => {
            eprintln!("[conn#{conn_id}] {now} response body error: {e}");
            return Ok(simple_response(StatusCode::BAD_GATEWAY, "upstream body error"));
        }
    };

    let mut out = Response::builder()
        .status(resp_parts.status)
        .version(resp_parts.version)
        .body(Full::new(resp_bytes.clone()))
        .expect("build response");

    *out.headers_mut() = resp_parts.headers;

    log_response(&state.cfg, conn_id, &out, &resp_bytes, &now);

    Ok(out)
}

fn simple_response(status: StatusCode, msg: &str) -> Response<Full<Bytes>> {
    Response::builder()
        .status(status)
        .header("content-type", "text/plain; charset=utf-8")
        .body(Full::new(Bytes::from(msg.to_string())))
        .unwrap()
}

fn remap_uri(method: &Method, uri: &Uri, cfg: &Config) -> Uri {
    // Preserve path and query, change scheme/authority to target.
    let path_and_query = uri
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/");
    let full = format!("{}://{}{}", cfg.target_scheme, cfg.target_authority, path_and_query);
    full.parse::<Uri>().unwrap_or_else(|_| Uri::from_static("/"))
}

fn copy_headers_forward(mut in_headers: HeaderMap, out_headers: &mut HeaderMap, cfg: &Config) {
    // Remove hop-by-hop headers per RFC 7230
    static HOP: &[&str] = &[
        "connection",
        "proxy-connection",
        "keep-alive",
        "transfer-encoding",
        "upgrade",
        "te",
        "trailer",
    ];
    for name in HOP {
        in_headers.remove(*name);
    }

    // Overwrite Host to target authority
    in_headers.insert(
        "host",
        HeaderValue::from_str(&cfg.target_authority).unwrap_or(HeaderValue::from_static("localhost")),
    );

    *out_headers = in_headers;
}

fn log_request(
    cfg: &Config,
    conn_id: u64,
    peer: &SocketAddr,
    req: &Request<Full<Bytes>>,
    body: &Bytes,
    now: &str,
) {
    println!(
        "\n[conn#{conn_id}] {now} REQUEST {} {} from {}",
        req.method(),
        req.uri(),
        peer
    );
    print_headers("→", req.headers(), &cfg.redact_header);
    if cfg.include_bodies {
        print_body("→", body, cfg.max_body_bytes);
    }
}

fn log_response(
    cfg: &Config,
    conn_id: u64,
    resp: &Response<Full<Bytes>>,
    body: &Bytes,
    now: &str,
) {
    println!("[conn#{conn_id}] {now} RESPONSE {}", resp.status());
    print_headers("←", resp.headers(), &cfg.redact_header);
    if cfg.include_bodies {
        print_body("←", body, cfg.max_body_bytes);
    }
}

fn print_headers(prefix: &str, headers: &HeaderMap, redact: &[String]) {
    let mut names: Vec<_> = headers.keys().map(|k| k.as_str()).collect();
    names.sort_unstable();
    for name in names {
        if let Some(val) = headers.get(name) {
            let display = if redact.iter().any(|r| r.eq_ignore_ascii_case(name)) {
                "<redacted>".to_string()
            } else {
                match val.to_str() {
                    Ok(s) => s.to_string(),
                    Err(_) => format!("<{} bytes>", val.as_bytes().len()),
                }
            };
            println!("{prefix} {name}: {display}");
        }
    }
}

fn print_body(prefix: &str, body: &Bytes, max: usize) {
    let take = body.len().min(max);
    if take == 0 {
        println!("{prefix} <no body>");
        return;
    }
    let slice = &body[..take];
    let printable = String::from_utf8_lossy(slice);
    if body.len() > take {
        println!("{prefix} body ({} / {} bytes, truncated):\n{}\n…", take, body.len(), printable);
    } else {
        println!("{prefix} body ({} bytes):\n{}", body.len(), printable);
    }
}

fn now_iso() -> String {
    OffsetDateTime::now_utc().format(&Rfc3339).unwrap_or_else(|_| "now".into())
}
