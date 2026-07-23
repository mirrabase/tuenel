# T12 — MCP explorer and tool invocation

**State:** mock fixture.

1. Open **MCP Explorer** and filter servers/tools.
2. Open a tool schema and submit valid JSON arguments.
3. Submit malformed JSON, missing required fields, and an oversized payload.
4. Exercise safe, destructive, and malicious-result fixtures.
5. Verify block, redact, warn, success, timeout, and error results.
6. Retry with the same idempotency key.

**Pass:** dangerous results are blocked, sensitive output is redacted, warnings are visible, and an idempotent retry does not duplicate the result.
