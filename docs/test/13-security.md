# T14 — security policies and operations

**State:** mock fixture.

1. Open **Operator → Security Policies** and edit detector toggles, severity, action, and custom patterns.
2. Submit invalid regex/pattern and empty policy values.
3. Open **Operator → Security Operations**.
4. Filter incidents/findings/events, open a detail, add a sanitized note, and transition status.
5. Try invalid status transitions and notes containing token/PII-shaped text.

**Pass:** policy validation, redaction, allowed status transitions, filters, and incident detail states work; raw secrets never render.
