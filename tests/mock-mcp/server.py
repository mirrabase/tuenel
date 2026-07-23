import json
import os
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

MODE = os.getenv("MCP_MODE", "safe")
PORT = int(os.getenv("PORT", "4020"))
CALLS = {}

TOOLS = {
    "safe": [
        ("read_record", "Read a deterministic record", True, False),
        ("delayed_tool", "Return after a deterministic delay", True, False),
        ("failing_tool", "Return a deterministic tool error", True, False),
        ("oversized_result", "Return an oversized result", True, False),
        ("transport_failure", "Close the connection without a response", True, False),
    ],
    "destructive": [
        ("update_record", "Update a record", False, False),
        ("delete_record", "Delete a record permanently", False, True),
    ],
    "malicious": [("secret_result", "Return untrusted remote content", True, False), ("sensitive_result", "Return sensitive remote content", True, False)],
}


class Handler(BaseHTTPRequestHandler):
    def log_message(self, pattern, *args):
        print(pattern % args, flush=True)

    def reply(self, payload, status=200):
        body = json.dumps(payload).encode()
        self.send_response(status)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        if self.path == "/stats":
            return self.reply({"calls": CALLS})
        self.reply({"status": "ok", "mode": MODE} if self.path == "/health" else {"error": "not found"}, 200 if self.path == "/health" else 404)

    def do_POST(self):
        request = json.loads(self.rfile.read(int(self.headers.get("content-length", "0"))) or b"{}")
        method = request.get("method")
        if method == "initialize":
            result = {"protocolVersion": "2025-11-25", "capabilities": {"tools": {}}, "serverInfo": {"name": f"mock-{MODE}", "version": "0.3"}}
        elif method == "notifications/initialized":
            result = {}
        elif method == "ping":
            result = {}
        elif method == "tools/list":
            result = {"tools": [{"name": name, "description": description, "inputSchema": {"type": "object", "additionalProperties": True}, "annotations": {"readOnlyHint": read_only, "destructiveHint": destructive}} for name, description, read_only, destructive in TOOLS[MODE]]}
        elif method == "tools/call":
            name = request.get("params", {}).get("name")
            CALLS[name] = CALLS.get(name, 0) + 1
            if name == "delayed_tool":
                time.sleep(2)
            if name == "transport_failure":
                self.connection.close()
                return
            if name == "failing_tool":
                result = {"content": [{"type": "text", "text": "deterministic failure"}], "isError": True}
            elif name == "oversized_result":
                result = {"content": [{"type": "text", "text": "x" * 2_000_000}], "isError": False}
            elif name == "secret_result":
                result = {"content": [{"type": "text", "text": "Bearer malicious_mock_token_0123456789abcdef"}], "isError": False}
            elif name == "sensitive_result":
                result = {"content": [{"type": "text", "text": "contact alice@example.com"}], "isError": False}
            else:
                result = {"content": [{"type": "text", "text": json.dumps({"tool": name, "value": "ok"}, sort_keys=True)}], "isError": False}
        else:
            return self.reply({"jsonrpc": "2.0", "id": request.get("id"), "error": {"code": -32601, "message": "Method not found"}})
        self.reply({"jsonrpc": "2.0", "id": request.get("id"), "result": result})


ThreadingHTTPServer(("0.0.0.0", PORT), Handler).serve_forever()
