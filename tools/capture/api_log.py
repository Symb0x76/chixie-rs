"""mitmproxy addon: log ChiXie backend traffic as JSON lines with secrets redacted.

    mitmdump -q -s tools/capture/api_log.py --set api_log=captures/api/session.jsonl

Logged per request: method, path, query, request body, status, response body. Redacted: the
`token` header, Authorization, cookies, any JSON field whose name contains token/password/secret/
key, and long opaque strings (font blobs, Fernet tokens) are cut to their length.
"""
import json
import re

from mitmproxy import ctx, http

HOST = "chixie.com.cn"
SECRET_KEY = re.compile(r"token|password|passwd|secret|key|authorization|cookie", re.I)
MAX_STR = 300


def scrub(v):
    if isinstance(v, dict):
        return {k: ("<redacted>" if SECRET_KEY.search(k) else scrub(x)) for k, x in v.items()}
    if isinstance(v, list):
        out = [scrub(x) for x in v[:20]]
        if len(v) > 20:
            out.append(f"<{len(v) - 20} more items>")
        return out
    if isinstance(v, str) and len(v) > MAX_STR:
        return f"<string, {len(v)} chars>"
    return v


def body(raw: bytes, ctype: str):
    if not raw:
        return None
    if "json" in ctype or raw[:1] in (b"{", b"["):
        try:
            return scrub(json.loads(raw))
        except ValueError:
            pass
    if "text" in ctype or "form" in ctype:
        text = raw.decode("utf-8", "replace")
        return text if len(text) <= MAX_STR else f"<text, {len(text)} chars>"
    return f"<{ctype or 'binary'}, {len(raw)} bytes>"


class ApiLog:
    def load(self, loader):
        loader.add_option("api_log", str, "api.jsonl", "output JSONL file")

    def response(self, flow: http.HTTPFlow):
        if HOST not in flow.request.pretty_host:
            return
        req, resp = flow.request, flow.response
        rec = {
            "method": req.method,
            "path": req.path.split("?")[0],
            "query": scrub(dict(req.query)),
            "headers": {k: ("<redacted>" if SECRET_KEY.search(k) else v) for k, v in req.headers.items()
                        if k.lower() in ("content-type", "token", "user-agent", "authorization")},
            "request": body(req.raw_content or b"", req.headers.get("content-type", "")),
            "status": resp.status_code,
            "response": body(resp.raw_content or b"", resp.headers.get("content-type", "")),
        }
        with open(ctx.options.api_log, "a", encoding="utf-8") as f:
            f.write(json.dumps(rec, ensure_ascii=False) + "\n")


addons = [ApiLog()]
