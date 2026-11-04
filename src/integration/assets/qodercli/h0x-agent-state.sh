#!/bin/sh
# managed by HyperOx; reinstalling the integration replaces this file.
# H0X_INTEGRATION_ID=qodercli
# H0X_INTEGRATION_VERSION=4

[ "${1:-}" = "session" ] || exit 0
[ "${H0X_ENV:-}" = "1" ] || exit 0
[ -n "${H0X_SOCKET_PATH:-}" ] || exit 0
[ -n "${H0X_PANE_ID:-}" ] || exit 0
command -v python3 >/dev/null 2>&1 || exit 0

python3 -c '
import json
import os
import subprocess
import sys
import time

try:
    payload = json.load(sys.stdin)
    session_id = payload.get("session_id")
    if not isinstance(session_id, str) or not session_id:
        raise ValueError
    subprocess.run(
        [
            os.environ.get("H0X_BIN_PATH") or "h0x",
            "pane", "report-agent-session", os.environ["H0X_PANE_ID"],
            "--source", "h0x:qodercli", "--agent", "qodercli",
            "--agent-session-id", session_id, "--seq", str(time.time_ns()),
        ],
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        timeout=1,
        check=False,
    )
except Exception:
    pass
' 2>/dev/null || true
