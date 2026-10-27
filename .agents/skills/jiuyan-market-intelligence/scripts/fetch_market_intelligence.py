#!/usr/bin/env python3
"""Fetch Jiuyangongshe community intelligence using browser-issued tokens."""

from __future__ import annotations

import argparse
import collections
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
        headers = {
            "Accept": "application/json, text/plain, */*",
            "Content-Type": "application/json",
            "Origin": "https://www.jiuyangongshe.com",
            "Referer": "https://www.jiuyangongshe.com/",
            "platform": "3",
            "X-Requested-With": "XMLHttpRequest",
            "timestamp": bundle["timestamp"],
            "token": bundle["token"],
            "User-Agent": (
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) "
                "AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36"
            ),
        }
        session = (bundle.get("session_token") or "").strip()
        if session:
            headers["Cookie"] = f"SESSION={session}"

        request = urllib.request.Request(
            BASE_URL + path,
            data=json.dumps(payload, ensure_ascii=False).encode("utf-8"),
            method="POST",
            headers=headers,
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


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Fetch Jiuyangongshe community articles and summarize stock mentions."
    )
    parser.add_argument("--pages", type=int, default=1, help="Number of pages to fetch")
    parser.add_argument("--page-size", type=int, default=20, help="Articles per page")
    parser.add_argument("--category-id", default="", help="Jiuyan category id")
    parser.add_argument("--article-type", type=int, default=0, help="0=all, 1=view, 2=news")
    parser.add_argument("--back-garden", type=int, default=0, help="Jiuyan back-garden selector")
    parser.add_argument("--compact", action="store_true", help="Print a compact summary")
    args = parser.parse_args()

    pages = max(1, args.pages)
    page_size = max(1, min(args.page_size, 50))
    articles: list[dict] = []
    for page in range(1, pages + 1):
        body = post(
            "/api/v2/article/community",
            {
                "category_id": args.category_id,
                "limit": page_size,
                "order": 1,
                "start": page,
                "type": args.article_type,
                "back_garden": args.back_garden,
            },
        )
        data = body.get("data") or {}
        result = data.get("result") if isinstance(data, dict) else None
        if not isinstance(result, list):
            raise SystemExit("Jiuyan community response did not contain data.result")
        articles.extend(result)
        if len(result) < page_size or not data.get("hasNext", True):
            break

    stock_counts: collections.Counter[str] = collections.Counter()
    for article in articles:
        for stock in article.get("stock_list") or []:
            name = (stock.get("name") or stock.get("code") or "").strip()
            if name:
                stock_counts[name] += 1

    output = {
        "articles": articles,
        "stock_mentions": [
            {"name": name, "mentions": count} for name, count in stock_counts.most_common()
        ],
    }
    if args.compact:
        output["articles"] = [
            {
                "article_id": article.get("article_id", ""),
                "title": article.get("title", ""),
                "subtitle": article.get("subtitle"),
                "nickname": (article.get("user") or {}).get("nickname", ""),
                "like_count": article.get("like_count", 0),
                "comment_count": article.get("comment_count", 0),
                "collect_count": article.get("collect_count", 0),
                "forward_count": article.get("forward_count", 0),
                "create_time": article.get("create_time", ""),
                "stock_list": article.get("stock_list") or [],
            }
            for article in articles
        ]
    json.dump(output, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")


if __name__ == "__main__":
    main()
