# us-http-tap

A lightweight HTTP(S) tap/proxy for localhost development. It listens on a local port, forwards traffic to a target (e.g., your app on `127.0.0.1:8080`), and logs HTTP requests/responses.

- Binary: `us-http-tap`
- Tech: Rust + tokio + hyper (similar stack to `us-interactive-branch-delete`)

## Usage

```
us-http-tap --listen 127.0.0.1:8888 --target 127.0.0.1:8080 --include-bodies --max-body-bytes 4096
```

Then point your client at `http://127.0.0.1:8888` instead of the original port. The tool logs request lines, headers (with `Authorization`, `Cookie`, and `Set-Cookie` redacted by default), and optionally bodies.

Flags:
- `--listen <addr>`: Address to bind (default `127.0.0.1:8888`).
- `--target <host:port|url>`: Upstream endpoint (required). Use `https://…` to enable TLS upstream; `host:443` is also treated as HTTPS.
- `--include-bodies`: Log request/response bodies.
- `--max-body-bytes <n>`: Max bytes of each body to print (default 2048).
- `--redact-header name[,name]...`: Headers to redact.
- `-k, --insecure-upstream`: Disable TLS certificate/hostname verification for upstream HTTPS (development only).
- `--tui`: Launch a live table view with per-path method counts and recency (q to quit, c to clear).

HTTPS support:
- Upstream HTTPS: supported automatically when `--target` is `https://…` (system trust store via rustls-native-certs).
- You can bypass cert verification with `-k/--insecure-upstream` for local/dev certs.
- HTTPS listen: either provide `--listen-tls-cert/--listen-tls-key` (PEM), or use `--listen-self-signed` to auto‑generate a dev cert/key in memory for `localhost`, `127.0.0.1`, and `::1`.

WebSocket support:
- WebSocket (Upgrade) and WSS upstream are supported transparently. Point your WS client at the `--listen` port and use the same path; the proxy forwards the 101 handshake and tunnels frames.
- Current version tunnels frames without deep inspection. The TUI will show the initial GET upgrade under the path.
- Listening with TLS: provide a dev cert and key and point clients to `https://localhost:<port>`:
  ```
  us-http-tap --listen 127.0.0.1:8443 --listen-tls-cert ./localhost.crt --listen-tls-key ./localhost.key --target 127.0.0.1:8080
  ```
  The certificate should be PEM; self-signed works if your client trusts it.

Notes:
- Full MITM for arbitrary remote hostnames (dynamic per-host certs) is out of scope.
- Bodies are buffered to log/forward, so very large payloads may impact memory.

TUI example:
```
us-http-tap --tui --listen 127.0.0.1:8888 --target http://127.0.0.1:8080
```

HTTPS listen examples:
```
# Auto self‑signed (dev):
us-http-tap --listen 127.0.0.1:8443 --listen-self-signed --target 127.0.0.1:8080

# Explicit cert/key files:
us-http-tap --listen 127.0.0.1:8443 \
  --listen-tls-cert ./localhost.crt --listen-tls-key ./localhost.key \
  --target 127.0.0.1:8080
```
