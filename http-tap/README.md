# us-http-tap

A lightweight HTTP tap/proxy for localhost development. It listens on a local port, forwards traffic to a target (e.g., your app on `127.0.0.1:8080`), and logs HTTP requests/responses.

- Binary: `us-http-tap`
- Tech: Rust + tokio + hyper (similar stack to `us-interactive-branch-delete`)

## Usage

```
us-http-tap --listen 127.0.0.1:8888 --target 127.0.0.1:8080 --include-bodies --max-body-bytes 4096
```

Then point your client at `http://127.0.0.1:8888` instead of the original port. The tool logs request lines, headers (with `Authorization`, `Cookie`, and `Set-Cookie` redacted by default), and optionally bodies.

Flags:
- `--listen <addr>`: Address to bind (default `127.0.0.1:8888`).
- `--target <host:port|url>`: Upstream HTTP endpoint (required).
- `--include-bodies`: Log request/response bodies.
- `--max-body-bytes <n>`: Max bytes of each body to print (default 2048).
- `--redact-header name[,name]...`: Headers to redact.

Notes:
- Only HTTP (plaintext) is supported; TLS interception is not.
- Bodies are buffered to log/forward, so very large payloads may impact memory.
