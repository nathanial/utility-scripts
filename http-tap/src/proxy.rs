use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::Context as _;
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::http::{HeaderMap, HeaderValue, Method, Request, Response, StatusCode, Uri};
use hyper::service::service_fn;
use hyper::upgrade;
use hyper::Error as HyperError;
use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use hyper_util::client::legacy::{connect::HttpConnector, Client};
use hyper_rustls::FixedServerNameResolver;
use hyper_util::rt::{TokioExecutor, TokioIo};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use crate::stats::{StatsEvent, StatsSender};
use rustls::{ClientConfig, SignatureScheme};
use rustls::client::danger::{ServerCertVerified, ServerCertVerifier, HandshakeSignatureValid};
use rustls_native_certs::load_native_certs;
use rustls::{RootCertStore, pki_types::CertificateDer, pki_types::PrivateKeyDer, pki_types::ServerName};
use rustls_pemfile::{certs, pkcs8_private_keys, rsa_private_keys};
use tokio::io::copy_bidirectional;
// (imports deduped above)

#[derive(Clone)]
pub struct Config {
    pub listen: SocketAddr,
    pub target_authority: String, // host:port
    pub target_scheme: &'static str, // "http"
    pub include_bodies: bool,
    pub max_body_bytes: usize,
    pub redact_header: Vec<String>,
    pub tls: Option<TlsConfig>,
    pub insecure_upstream: bool,
    pub stats: Option<StatsSender>,
    pub upstream_ca: Vec<std::path::PathBuf>,
    pub upstream_client_cert: Option<std::path::PathBuf>,
    pub upstream_client_key: Option<std::path::PathBuf>,
    pub upstream_server_name: Option<String>,
    pub upstream_host: Option<String>,
}

#[derive(Clone)]
pub struct TlsConfig {
    pub acceptor: tokio_rustls::TlsAcceptor,
}

pub async fn run_proxy(cfg: Config) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(cfg.listen)
        .await
        .with_context(|| format!("bind {}", cfg.listen))?;

    let client = {
        let https = build_https_connector(&cfg)?;
        Client::builder(TokioExecutor::new()).build::<_, Full<Bytes>>(https)
    };

    let shared = Arc::new(ProxyState::new(cfg, client));

    let listen_scheme = if shared.cfg.tls.is_some() { "https" } else { "http" };
    let upstream_scheme = shared.cfg.target_scheme;
    eprintln!(
        "us-http-tap listening on {}://{} → {}://{}",
        listen_scheme,
        shared.cfg.listen,
        upstream_scheme,
        shared.cfg.target_authority
    );

    loop {
        let (stream, addr) = listener.accept().await?;
        let state = shared.clone();
        if let Some(tls) = &shared.cfg.tls {
            let acceptor = tls.acceptor.clone();
            tokio::spawn(async move {
                match acceptor.accept(stream).await {
                    Ok(tls_stream) => {
                        let io = TokioIo::new(tls_stream);
                        let conn_id = state.next_conn_id();
            let svc = service_fn(move |req| handle(state.clone(), conn_id, addr, req));
                        if let Err(err) = hyper::server::conn::http1::Builder::new()
                            .serve_connection(io, svc)
                            .await
                        {
                            eprintln!("[conn#{conn_id}] connection error: {err}");
                        }
                    }
                    Err(err) => {
                        eprintln!("TLS accept error from {}: {}", addr, err);
                    }
                }
            });
        } else {
            tokio::spawn(async move {
                let io = TokioIo::new(stream);
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
}

#[derive(Clone)]
struct ProxyState {
    cfg: Config,
    client: Client<HttpsConnector<HttpConnector>, Full<Bytes>>,
    conn_seq: Arc<AtomicU64>,
}

impl ProxyState {
    fn new(cfg: Config, client: Client<HttpsConnector<HttpConnector>, Full<Bytes>>) -> Self {
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

    // WebSocket upgrade path: tunnel bytes after 101 handshake
    if is_websocket_upgrade(req.headers()) {
        // Preserve required WS hop-by-hop headers for the upstream handshake.
        let conn_hdr = req
            .headers()
            .get(hyper::http::header::CONNECTION)
            .cloned();
        let upgr_hdr = req
            .headers()
            .get(hyper::http::header::UPGRADE)
            .cloned();
        let ws_key = req
            .headers()
            .get("sec-websocket-key")
            .cloned();
        let ws_ver = req
            .headers()
            .get("sec-websocket-version")
            .cloned();
        let ws_proto = req
            .headers()
            .get("sec-websocket-protocol")
            .cloned();
        let ws_ext = req
            .headers()
            .get("sec-websocket-extensions")
            .cloned();

        // Rebuild forwarded request without body
        let mut forwarded = Request::builder()
            .method(req.method().clone())
            .version(req.version())
            .uri(remap_uri(&req.method(), req.uri(), &state.cfg))
            .body(Full::new(Bytes::new()))
            .expect("build ws request");
        copy_headers_forward(req.headers().clone(), forwarded.headers_mut(), &state.cfg);
        if let Some(v) = conn_hdr { forwarded.headers_mut().insert(hyper::http::header::CONNECTION, v); }
        if let Some(v) = upgr_hdr { forwarded.headers_mut().insert(hyper::http::header::UPGRADE, v); }
        if let Some(v) = ws_key { forwarded.headers_mut().insert("sec-websocket-key", v); }
        if let Some(v) = ws_ver { forwarded.headers_mut().insert("sec-websocket-version", v); }
        if let Some(v) = ws_proto { forwarded.headers_mut().insert("sec-websocket-protocol", v); }
        if let Some(v) = ws_ext { forwarded.headers_mut().insert("sec-websocket-extensions", v); }

        // Perform upstream handshake
        let mut upstream_resp = match state.client.request(forwarded).await {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[conn#{conn_id}] {now} upstream WS handshake error: {e}");
                return Ok(simple_response(StatusCode::BAD_GATEWAY, "upstream WS handshake failed"));
            }
        };

        if upstream_resp.status() != StatusCode::SWITCHING_PROTOCOLS {
            eprintln!(
                "[conn#{conn_id}] {now} upstream WS expected 101, got {}",
                upstream_resp.status()
            );
            return Ok(simple_response(StatusCode::BAD_GATEWAY, "upstream did not switch protocols"));
        }

        // Build response to client with upstream headers
        let upstream_headers = upstream_resp.headers().clone();
        let mut client_resp_builder = Response::builder().status(StatusCode::SWITCHING_PROTOCOLS);
        {
            let h = client_resp_builder.headers_mut().unwrap();
            *h = upstream_headers;
        }
        let client_resp = client_resp_builder
            .body(Full::new(Bytes::new()))
            .expect("ws 101 resp");

        // Spawn tunnel task after connection upgrades
        let state_clone = state.clone();
        tokio::spawn(async move {
            let now = now_iso();
            match (upgrade::on(req).await, upgrade::on(upstream_resp).await) {
                (Ok(down), Ok(up)) => {
                    let mut down = TokioIo::new(down);
                    let mut up = TokioIo::new(up);
                    let _ = copy_bidirectional(&mut down, &mut up).await;
                }
                (Err(e), _) | (_, Err(e)) => {
                    eprintln!("[conn#{conn_id}] {now} WS upgrade tunnel error: {e}");
                }
            }
            drop(state_clone);
        });

        return Ok(client_resp);
    }

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
    if let Some(tx) = &state.cfg.stats {
        let path = req_parts
            .uri
            .path_and_query()
            .map(|pq| pq.as_str().to_string())
            .unwrap_or_else(|| "/".to_string());
        let _ = tx.send(StatsEvent {
            method: req_parts.method.clone(),
            path,
            at: std::time::SystemTime::now(),
        });
    }

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

    // Overwrite Host to target authority, unless explicitly overridden
    let host_value = cfg
        .upstream_host
        .as_deref()
        .unwrap_or(&cfg.target_authority);
    in_headers.insert(
        "host",
        HeaderValue::from_str(host_value).unwrap_or(HeaderValue::from_static("localhost")),
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

fn is_websocket_upgrade(headers: &HeaderMap) -> bool {
    let upgrade = headers
        .get(hyper::http::header::UPGRADE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.eq_ignore_ascii_case("websocket"))
        .unwrap_or(false);
    if !upgrade {
        return false;
    }
    // Connection header may contain comma-separated tokens
    headers
        .get(hyper::http::header::CONNECTION)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').any(|t| t.trim().eq_ignore_ascii_case("upgrade")))
        .unwrap_or(false)
}

#[derive(Debug)]
struct NoVerifier;

impl ServerCertVerifier for NoVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![
            SignatureScheme::RSA_PKCS1_SHA256,
            SignatureScheme::RSA_PKCS1_SHA384,
            SignatureScheme::RSA_PKCS1_SHA512,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::RSA_PSS_SHA384,
            SignatureScheme::RSA_PSS_SHA512,
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::ECDSA_NISTP384_SHA384,
            SignatureScheme::ED25519,
            SignatureScheme::ED448,
        ]
    }
}

fn build_https_connector(cfg: &Config) -> anyhow::Result<HttpsConnector<HttpConnector>> {
    if cfg.insecure_upstream {
        let no_verify = Arc::new(NoVerifier);
        let tls_cfg = ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(no_verify)
            .with_no_client_auth();
        let mut b = HttpsConnectorBuilder::new().with_tls_config(tls_cfg).https_or_http();
        if let Some(name) = &cfg.upstream_server_name {
            let sn = ServerName::try_from(name.clone())?;
            b = b.with_server_name_resolver(FixedServerNameResolver::new(sn));
        }
        Ok(b.enable_http1().build())
    } else {
        // Build root store from native + optional extra CAs
        let mut roots = RootCertStore::empty();
        let native = load_native_certs();
        for cert in native.certs {
            let _ = roots.add(cert);
        }
        // Load extra CAs
        for path in &cfg.upstream_ca {
            if let Ok(file) = std::fs::File::open(path) {
                let mut reader = std::io::BufReader::new(file);
                for c in certs(&mut reader) {
                    match c {
                        Ok(der) => {
                            let _ = roots.add(CertificateDer::from(der));
                        }
                        Err(_) => {}
                    }
                }
            } else {
                eprintln!("Warning: unable to open upstream CA file: {}", path.display());
            }
        }

        let builder = rustls::ClientConfig::builder().with_root_certificates(roots);
        let tls_cfg = if let (Some(cert_path), Some(key_path)) = (&cfg.upstream_client_cert, &cfg.upstream_client_key) {
            // Load client cert chain
            let chain: Vec<CertificateDer<'static>> = match std::fs::File::open(cert_path) {
                Ok(f) => {
                    let mut r = std::io::BufReader::new(f);
                    certs(&mut r)
                        .filter_map(|c| c.ok())
                        .map(|d| CertificateDer::from(d))
                        .collect()
                }
                Err(_) => Vec::new(),
            };
            // Load client key
            let key_der: Option<PrivateKeyDer<'static>> = match std::fs::File::open(key_path) {
                Ok(f) => {
                    let mut r = std::io::BufReader::new(f);
                    let mut keys: Vec<PrivateKeyDer> = pkcs8_private_keys(&mut r)
                        .filter_map(|k| k.ok())
                        .map(PrivateKeyDer::from)
                        .collect();
                    if keys.is_empty() {
                        if let Ok(f2) = std::fs::File::open(key_path) {
                            let mut r2 = std::io::BufReader::new(f2);
                            keys = rsa_private_keys(&mut r2)
                                .filter_map(|k| k.ok())
                                .map(PrivateKeyDer::from)
                                .collect();
                        }
                    }
                    keys.into_iter().next()
                }
                Err(_) => None,
            };
            if !chain.is_empty() {
                if let Some(k) = key_der {
                    match builder.clone().with_client_auth_cert(chain, k) {
                        Ok(cfg) => cfg,
                        Err(e) => {
                            eprintln!("Warning: invalid client cert/key for upstream mTLS: {}", e);
                            builder.clone().with_no_client_auth()
                        }
                    }
                } else {
                    eprintln!("Warning: upstream client key not found or invalid; proceeding without client auth");
                    builder.clone().with_no_client_auth()
                }
            } else {
                eprintln!("Warning: upstream client cert chain empty; proceeding without client auth");
                builder.clone().with_no_client_auth()
            }
        } else {
            builder.with_no_client_auth()
        };

        let mut b = HttpsConnectorBuilder::new().with_tls_config(tls_cfg).https_or_http();
        if let Some(name) = &cfg.upstream_server_name {
            let sn = ServerName::try_from(name.clone())?;
            b = b.with_server_name_resolver(FixedServerNameResolver::new(sn));
        }
        Ok(b.enable_http1().build())
    }
}
