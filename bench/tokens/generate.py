"""Generates the benchmark datasets in data/ with a fixed seed.

Each dataset mimics the shape of a typical MCP tool result. Values are
synthetic, so the files can be committed and the results reproduced exactly.
"""

import json
import random
from datetime import datetime, timedelta, timezone
from pathlib import Path

WORDS = (
    "parser token format model context tool result error build release test "
    "value array object string number table index query filter field schema "
    "config server client request response stream cache user project issue "
    "commit branch merge review deploy update fix crash memory budget limit"
).split()
NAMES = "oleh alex maria ivan sofia taras olena andrii daria max".split()
STATES = ["open", "closed"]
LABELS = [("bug", "d73a4a"), ("enhancement", "a2eeef"), ("docs", "0075ca"),
          ("performance", "fbca04"), ("good first issue", "7057ff")]


def sentence(rng, low=6, high=14):
    words = [rng.choice(WORDS) for _ in range(rng.randint(low, high))]
    return " ".join(words).capitalize() + "."


def timestamp(rng):
    start = datetime(2026, 1, 1, tzinfo=timezone.utc)
    moment = start + timedelta(minutes=rng.randint(0, 300_000))
    return moment.strftime("%Y-%m-%dT%H:%M:%SZ")


def github_issues(rng):
    issues = []
    for number in range(1, 41):
        login = rng.choice(NAMES)
        issues.append({
            "number": 1000 + number,
            "title": sentence(rng, 4, 9)[:-1],
            "state": rng.choice(STATES),
            "user": {"login": login, "id": 10_000 + NAMES.index(login)},
            "labels": [{"name": name, "color": color}
                       for name, color in rng.sample(LABELS, rng.randint(0, 2))],
            "comments": rng.randint(0, 25),
            "created_at": timestamp(rng),
            "body": "\n".join(sentence(rng) for _ in range(rng.randint(1, 4))),
        })
    return {"repository": "axs-lang/axs", "total_count": len(issues), "items": issues}


def file_listing(rng):
    entries = []
    for index in range(150):
        directory = rng.choice(["src", "src/cli", "tests", "docs", "crates/axs-wasm/src"])
        kind = "file" if rng.random() < 0.9 else "dir"
        name = f"{rng.choice(WORDS)}_{index}" + (".rs" if kind == "file" else "")
        entries.append({
            "path": f"{directory}/{name}",
            "type": kind,
            "size": rng.randint(100, 90_000) if kind == "file" else 0,
            "modified": timestamp(rng),
        })
    return {"root": "/workspace/axs", "entries": entries}


def sql_rows(rng):
    rows = []
    for order_id in range(1, 121):
        name = rng.choice(NAMES)
        rows.append({
            "id": order_id,
            "customer": name.capitalize(),
            "email": f"{name}{order_id}@example.com",
            "total": round(rng.uniform(5, 900), 2),
            "items": rng.randint(1, 12),
            "status": rng.choice(["paid", "pending", "refunded", "shipped"]),
            "created_at": timestamp(rng),
        })
    return {"query": "SELECT * FROM orders LIMIT 120", "row_count": len(rows), "rows": rows}


def search_results(rng):
    results = []
    for rank in range(1, 21):
        slug = "-".join(rng.choice(WORDS) for _ in range(3))
        results.append({
            "rank": rank,
            "title": sentence(rng, 3, 7)[:-1],
            "url": f"https://docs.example.com/{slug}",
            "snippet": sentence(rng, 18, 30),
            "score": round(1 - rank * 0.031 - rng.random() * 0.01, 4),
        })
    return {"query": "token efficient data format", "results": results}


def nested_config(rng):
    services = {}
    for name in ["api", "worker", "scheduler", "gateway"]:
        services[name] = {
            "image": f"registry.example.com/{name}:1.{rng.randint(0, 20)}.{rng.randint(0, 9)}",
            "replicas": rng.randint(1, 6),
            "resources": {
                "limits": {"cpu": f"{rng.choice([250, 500, 1000])}m", "memory": f"{rng.choice([256, 512, 1024])}Mi"},
                "requests": {"cpu": "100m", "memory": "128Mi"},
            },
            "env": {f"{w.upper()}_{i}": rng.choice(["true", "false", sentence(rng, 1, 2)[:-1]])
                    for i, w in enumerate(rng.sample(WORDS, 4))},
            "ports": [rng.choice([80, 443, 8080, 9090])],
            "healthcheck": {"path": "/healthz", "interval_seconds": 10, "timeout_seconds": 2},
        }
    return {"version": 3, "environment": "staging", "services": services}


DATASETS = {
    "github_issues": github_issues,
    "file_listing": file_listing,
    "sql_rows": sql_rows,
    "search_results": search_results,
    "nested_config": nested_config,
}


def main():
    data = Path(__file__).parent / "data"
    data.mkdir(exist_ok=True)
    for index, (name, build) in enumerate(DATASETS.items()):
        value = build(random.Random(0xA85 + index))
        (data / f"{name}.json").write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n")
        print(f"wrote data/{name}.json")


if __name__ == "__main__":
    main()
