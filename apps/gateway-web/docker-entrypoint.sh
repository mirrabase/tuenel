#!/bin/sh
set -eu

secret=${WEB_SESSION_SECRET:-}
if [ "${#secret}" -lt 32 ]; then
  echo "WEB_SESSION_SECRET must be at least 32 characters" >&2
  exit 1
fi
if [ "$secret" = "development-only-web-session-secret" ]; then
  echo "WARNING: WEB_SESSION_SECRET uses the known development example; replace it before production" >&2
fi

exec "$@"
