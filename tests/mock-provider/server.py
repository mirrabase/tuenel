import base64
import json
import os
import time
from urllib.parse import parse_qs, urlparse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

import jwt
from cryptography.hazmat.primitives.asymmetric import rsa


PRIVATE_KEY = rsa.generate_private_key(public_exponent=65537, key_size=2048)
PUBLIC_KEY = PRIVATE_KEY.public_key().public_numbers()
ISSUER = os.environ.get("MOCK_ISSUER", "http://localhost:4010")
AUDIENCE = os.environ.get("MOCK_AUDIENCE", "gateway")
TENANT = os.environ.get("MOCK_TENANT", "demo-tenant")
PORT = int(os.environ.get("PORT", "4010"))
COUNTS = {}
LAST = {}


def b64(value):
    size = (value.bit_length() + 7) // 8
    return base64.urlsafe_b64encode(value.to_bytes(size, "big")).rstrip(b"=").decode()


class Handler(BaseHTTPRequestHandler):
    def log_message(self, pattern, *args):
        print(pattern % args, flush=True)

    def send_json(self, status, payload):
        body = json.dumps(payload).encode()
        self.send_response(status)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        parsed = urlparse(self.path)
        if parsed.path == "/health":
            return self.send_json(200, {"status": "ok"})
        if parsed.path == "/stats":
            return self.send_json(200, {"counts": COUNTS, "last": LAST})
        if parsed.path == "/jwks":
            return self.send_json(200, {"keys": [{
                "kty": "RSA", "kid": "mock-key", "alg": "RS256", "use": "sig",
                "n": b64(PUBLIC_KEY.n), "e": b64(PUBLIC_KEY.e),
            }]})
        if parsed.path == "/token":
            now = int(time.time())
            query = parse_qs(parsed.query)
            token = jwt.encode({
                "sub": query.get("sub", ["demo-user"])[0], "tenant_id": query.get("tenant", [TENANT])[0], "project_id": query.get("project", [None])[0], "roles": ["gateway_admin"],
                "iss": ISSUER, "aud": AUDIENCE, "iat": now, "exp": now + 3600,
            }, PRIVATE_KEY, algorithm="RS256", headers={"kid": "mock-key"})
            return self.send_json(200, {"token": token})
        self.send_json(404, {"error": "not found"})

    def do_POST(self):
        length = int(self.headers.get("content-length", "0"))
        request = json.loads(self.rfile.read(length) or b"{}")
        COUNTS[self.path.split("?", 1)[0]] = COUNTS.get(self.path.split("?", 1)[0], 0) + 1
        LAST[self.path.split("?", 1)[0]] = request
        if self.path == "/v1/responses":
            return self.send_json(200, {"id": "resp-mock", "status": "completed", "output": [{"content": [{"text": "hello"}]}], "usage": {"input_tokens": 2, "output_tokens": 1}})
        if self.path == "/v1/embeddings":
            inputs = request.get("input", [])
            if isinstance(inputs, str):
                inputs = [inputs]
            return self.send_json(200, {"data": [{"embedding": [0.1, 0.2, 0.3]} for _ in inputs], "usage": {"prompt_tokens": len(inputs)}})
        if self.path == "/v1/messages":
            return self.send_json(200, {"id": "anthropic-mock", "content": [{"type": "text", "text": "hello"}], "stop_reason": "end_turn", "usage": {"input_tokens": 2, "output_tokens": 1}})
        if self.path.startswith("/v1beta/models/") and self.path.split("?", 1)[0].endswith(":generateContent"):
            return self.send_json(200, {"candidates": [{"content": {"parts": [{"text": "hello"}]}}], "usageMetadata": {"promptTokenCount": 2, "candidatesTokenCount": 1}})
        if self.path != "/v1/chat/completions":
            return self.send_json(404, {"error": "not found"})
        if request.get("stream"):
            chunks = [
                {"id": "chatcmpl-mock", "choices": [{"delta": {"content": "hello"}, "finish_reason": None}]},
                {"id": "chatcmpl-mock", "choices": [{"delta": {}, "finish_reason": "stop"}]},
                {"id": "chatcmpl-mock", "choices": [], "usage": {"prompt_tokens": 2, "completion_tokens": 1}},
            ]
            body = "".join(f"data: {json.dumps(chunk)}\n\n" for chunk in chunks) + "data: [DONE]\n\n"
            encoded = body.encode()
            self.send_response(200)
            self.send_header("content-type", "text/event-stream")
            self.send_header("content-length", str(len(encoded)))
            self.end_headers()
            self.wfile.write(encoded)
            return
        self.send_json(200, {
            "id": "chatcmpl-mock",
            "choices": [{"message": {"role": "assistant", "content": "hello"}, "finish_reason": "stop"}],
            "usage": {"prompt_tokens": 2, "completion_tokens": 1, "total_tokens": 3},
        })


ThreadingHTTPServer(("0.0.0.0", PORT), Handler).serve_forever()
