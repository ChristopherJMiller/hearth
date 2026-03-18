#!/usr/bin/env python3
"""Stateful mock Hearth API server for NixOS VM integration tests.

Maintains enrollment state in memory and responds with JSON matching
the exact field names expected by the Rust serde deserialization in
hearth-common's api_types.rs.

Usage:
    python3 mock-api.py --port 3000
"""

import argparse
import json
import re
import sys
import uuid
from http.server import HTTPServer, BaseHTTPRequestHandler


# ---------------------------------------------------------------------------
# In-memory state
# ---------------------------------------------------------------------------

enrollments: dict[str, dict] = {}
"""machine_id -> enrollment record"""

heartbeats: list[dict] = []
"""Append-only log of heartbeat payloads received."""


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def json_response(handler: BaseHTTPRequestHandler, status: int, body: dict) -> None:
    """Send a JSON response."""
    payload = json.dumps(body).encode()
    handler.send_response(status)
    handler.send_header("Content-Type", "application/json")
    handler.send_header("Content-Length", str(len(payload)))
    handler.end_headers()
    handler.wfile.write(payload)


def read_body(handler: BaseHTTPRequestHandler) -> dict:
    """Read and parse the JSON request body. Returns {} on failure."""
    try:
        length = int(handler.headers.get("Content-Length", 0))
        if length == 0:
            return {}
        raw = handler.rfile.read(length)
        return json.loads(raw)
    except Exception:
        return {}


# ---------------------------------------------------------------------------
# Route patterns (compiled once)
# ---------------------------------------------------------------------------

RE_ENROLLMENT_STATUS = re.compile(
    r"^/api/v1/machines/(?P<id>[0-9a-fA-F-]+)/enrollment-status$"
)
RE_TEST_APPROVE = re.compile(
    r"^/api/v1/test/approve/(?P<id>[0-9a-fA-F-]+)$"
)
RE_TEST_ENROLLMENT = re.compile(
    r"^/api/v1/test/enrollments/(?P<id>[0-9a-fA-F-]+)$"
)


# ---------------------------------------------------------------------------
# Request handler
# ---------------------------------------------------------------------------

class MockApiHandler(BaseHTTPRequestHandler):
    """Handles all mock API routes."""

    # Silence per-request log lines on stdout; we log to stderr instead.
    def log_message(self, fmt, *args):
        sys.stderr.write("[mock-api] %s - %s\n" % (self.address_string(), fmt % args))

    # -- GET ----------------------------------------------------------------

    def do_GET(self):
        path = self.path.split("?")[0]  # strip query string

        # Health checks
        if path in ("/health", "/api/v1/health"):
            json_response(self, 200, {"status": "ok"})
            return

        # GET /api/v1/machines/<id>/enrollment-status
        m = RE_ENROLLMENT_STATUS.match(path)
        if m:
            machine_id = m.group("id")
            record = enrollments.get(machine_id)
            if record is None:
                json_response(self, 404, {"error": "not found"})
                return

            resp: dict = {
                "machine_id": machine_id,
                "status": record["status"],
                "message": record.get("message", ""),
            }

            # When approved, include provisioning fields.
            if record["status"] == "approved":
                resp["machine_token"] = record.get(
                    "machine_token", f"test-machine-token-{machine_id}"
                )
                resp["target_closure"] = record.get(
                    "target_closure", "/nix/store/fake-system-closure"
                )
                resp["cache_url"] = record.get("cache_url", None)
                resp["cache_token"] = record.get("cache_token", None)
                resp["disko_config"] = record.get("disko_config", None)
                resp["enrolled_by"] = record.get("enrolled_by", None)

            json_response(self, 200, resp)
            return

        # GET /api/v1/test/enrollments — list all enrollments (test introspection)
        if path == "/api/v1/test/enrollments":
            json_response(self, 200, enrollments)
            return

        # GET /api/v1/test/enrollments/<id> — single enrollment (test introspection)
        m = RE_TEST_ENROLLMENT.match(path)
        if m:
            machine_id = m.group("id")
            record = enrollments.get(machine_id)
            if record is None:
                json_response(self, 404, {"error": "not found"})
                return
            json_response(self, 200, {**record, "machine_id": machine_id})
            return

        # GET /api/v1/test/heartbeats — list all received heartbeats (test introspection)
        if path == "/api/v1/test/heartbeats":
            json_response(self, 200, {"heartbeats": heartbeats})
            return

        # Fallback
        json_response(self, 404, {"error": "not found"})

    # -- POST ---------------------------------------------------------------

    def do_POST(self):
        path = self.path.split("?")[0]

        # POST /api/v1/enroll
        if path == "/api/v1/enroll":
            body = read_body(self)
            machine_id = str(uuid.uuid4())
            enrollments[machine_id] = {
                "status": "pending",
                "hostname": body.get("hostname"),
                "hardware_fingerprint": body.get("hardware_fingerprint"),
                "os_version": body.get("os_version"),
                "role_hint": body.get("role_hint"),
                "hardware_report": body.get("hardware_report"),
                "serial_number": body.get("serial_number"),
                "hardware_config": body.get("hardware_config"),
                "message": "Enrollment received",
            }
            json_response(self, 200, {
                "machine_id": machine_id,
                "status": "pending",
                "message": "Enrollment received",
            })
            return

        # POST /api/v1/test/approve/<id>  (test-only)
        m = RE_TEST_APPROVE.match(path)
        if m:
            machine_id = m.group("id")
            record = enrollments.get(machine_id)
            if record is None:
                json_response(self, 404, {"error": "not found"})
                return
            body = read_body(self)
            record["status"] = "approved"
            # Allow overrides from the request body
            for key in ("target_closure", "machine_token", "disko_config",
                        "cache_url", "cache_token", "enrolled_by"):
                if key in body:
                    record[key] = body[key]
            json_response(self, 200, {
                "status": "approved",
                "machine_id": machine_id,
            })
            return

        # POST /api/v1/heartbeat
        if path == "/api/v1/heartbeat":
            body = read_body(self)
            heartbeats.append(body)
            # Return a minimal HeartbeatResponse matching the Rust struct.
            json_response(self, 200, {
                "target_closure": None,
                "pending_installs": [],
                "active_deployment_id": None,
                "cache_url": None,
                "cache_token": None,
                "machine_token": None,
                "pending_actions": [],
                "pending_user_envs": [],
                "status": "ok",
            })
            return

        # Fallback
        json_response(self, 404, {"error": "not found"})


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(description="Hearth mock API server")
    parser.add_argument("--port", type=int, default=3000, help="Listen port")
    args = parser.parse_args()

    server = HTTPServer(("0.0.0.0", args.port), MockApiHandler)
    sys.stderr.write(f"[mock-api] Listening on 0.0.0.0:{args.port}\n")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        sys.stderr.write("[mock-api] Shutting down\n")
        server.server_close()


if __name__ == "__main__":
    main()
