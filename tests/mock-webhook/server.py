import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

EVENTS = []

class Handler(BaseHTTPRequestHandler):
    def reply(self, status, value):
        body = json.dumps(value).encode()
        self.send_response(status)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        self.reply(200, {"status": "ok", "count": len(EVENTS)})

    def do_POST(self):
        EVENTS.append(json.loads(self.rfile.read(int(self.headers.get("content-length", "0"))) or b"{}"))
        self.reply(202, {"accepted": True})

ThreadingHTTPServer(("0.0.0.0", 4030), Handler).serve_forever()
