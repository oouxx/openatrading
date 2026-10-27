#!/usr/bin/env python3
"""Fetch Jiuyangongshe sector movement groups using browser-issued tokens."""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import subprocess
import sys
from pathlib import Path
import urllib.error
import urllib.request

BASE_URL = "https://web-api.jiuyangongshe.com/jystock-app"


_TOKEN_CACHE = None


def env(name: str) -> str:
    value = os.environ.get(name, "").strip()
    if not value:
        raise SystemExit(f"{name} is not set.")
    return value


def load_token_bundle(force: bool = False) -> dict:
    global _TOKEN_CACHE
    if not force:
        timestamp = os.environ.get("JIUYAN_TIMESTAMP", "").strip()
        token = os.environ.get("JIUYAN_TOKEN", "").strip()
        if timestamp and token:
            return {
                "timestamp": timestamp,
                "token": token,
                "session_token": os.environ.get("JIUYAN_SESSION_TOKEN", "").strip() or None,
            }
        if _TOKEN_CACHE is not None:
            return _TOKEN_CACHE

    script = Path(__file__).with_name("get_token.mjs")
    try:
        result = subprocess.run(
            ["node", str(script)],
            check=False,
            capture_output=True,
            text=True,
            timeout=60,
        )
    except FileNotFoundError as exc:
        raise SystemExit("node is required to acquire a fresh Jiuyan token") from exc
    except subprocess.TimeoutExpired as exc:
        raise SystemExit("Timed out while acquiring a fresh Jiuyan token") from exc
    if result.returncode != 0:
        raise SystemExit(result.stderr.strip() or "Could not acquire a fresh Jiuyan token")
    try:
        bundle = json.loads(result.stdout)
    except json.JSONDecodeError as exc:
        raise SystemExit("Token helper returned invalid JSON") from exc
    if not bundle.get("timestamp") or not bundle.get("token"):
        raise SystemExit("Token helper did not return timestamp/token")
    session = os.environ.get("JIUYAN_SESSION_TOKEN", "").strip()
    if not session:
        session = (bundle.get("session_token") or "").strip()
    if session:
        bundle["session_token"] = session
    else:
        bundle.pop("session_token", None)
    _TOKEN_CACHE = bundle
    return bundle


def post(path: str, payload: dict) -> dict:
    for attempt in range(2):
        bundle = load_token_bundle(force=attempt > 0)
        session = (bundle.get("session_token") or "").strip()
        if not session:
            raise SystemExit(
                "JIUYAN_SESSION_TOKEN is required for sector movements. "
                "Use a logged-in browser profile or provide the SESSION cookie."
            )
        request = urllib.request.Request(
            BASE_URL + path,
            data=json.dumps(payload, ensure_ascii=False).encode("utf-8"),
            method="POST",
            headers={
                "Accept": "application/json, text/plain, */*",
                "Content-Type": "application/json",
                "Origin": "https://www.jiuyangongshe.com",
                "Referer": "https://www.jiuyangongshe.com/",
                "platform": "3",
                "X-Requested-With": "XMLHttpRequest",
                "timestamp": bundle["timestamp"],
                "token": bundle["token"],
                "Cookie": f"SESSION={session}",
                "User-Agent": (
                    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) "
                    "AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36"
                ),
            },
        )
        try:
            with urllib.request.urlopen(request, timeout=30) as response:
                body = json.load(response)
        except urllib.error.HTTPError as exc:
            raise SystemExit(f"Jiuyan HTTP error: {exc.code}") from exc
        except urllib.error.URLError as exc:
            raise SystemExit(f"Jiuyan request failed: {exc.reason}") from exc
        except json.JSONDecodeError as exc:
            raise SystemExit("Jiuyan response was not valid JSON") from exc

        code = str(body.get("errCode", "0"))
        message = body.get("msg", "")
        if code == "110" and attempt == 0:
            continue
        if code == "110":
            raise SystemExit("Jiuyan dynamic token expired after refresh (errCode=110).")
        if code in {"1", "401"}:
            raise SystemExit("Jiuyan session expired. Refresh JIUYAN_SESSION_TOKEN.")
        if code != "0":
            raise SystemExit(f"Jiuyan request failed: errCode={code}, msg={message}")
        return body
    raise SystemExit("Jiuyan request failed after token refresh")


def beijing_today() -> str:
    return (dt.datetime.now(dt.timezone.utc) + dt.timedelta(hours=8)).date().isoformat()


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Fetch and print Jiuyangongshe sector movement groups."
    )
    parser.add_argument("--date", default=beijing_today(), help="Trading date YYYY-MM-DD")
    parser.add_argument("--pc", type=int, default=1, help="Jiuyan PC client flag")
    parser.add_argument("--compact", action="store_true", help="Print a compact summary")
    args = parser.parse_args()

    body = post("/api/v1/action/field", {"date": args.date, "pc": args.pc})
    data = body.get("data")
    if args.compact:
        groups = data if isinstance(data, list) else []
        output = [
            {
                "name": group.get("name", ""),
                "count": group.get("count", 0),
                "reason": group.get("reason", ""),
                "stocks": [
                    {
                        "code": item.get("code", ""),
                        "name": item.get("name", ""),
                        "action": (item.get("article") or {}).get("action_info"),
                    }
                    for item in group.get("list", [])
                ],
            }
            for group in groups
        ]
    else:
        output = data
    json.dump(output, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")


if __name__ == "__main__":
    main()
