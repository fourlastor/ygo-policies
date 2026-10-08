"""Put the requests of games that were played to a running server again, and
compare its answers with the ones those games got.

    python3 check_server.py TRACED.jsonl --server http://127.0.0.1:3013 \\
        --protocol v1 --games 24 --threads 32

TRACED.jsonl is what `policy-bench matchup --trace true` writes: every
request the model's seat sent in each game, with the answer it got.  Each
seat's requests are sent again in their order, through the sessions of `/v0`
or with the duel's memory carried by the client (`/v1/predict`), `--threads`
duels at a time.  Said at the end: how many answers are the same to the last
digit, the largest gap in a probability and in the estimate, how many first
choices differ, and how many requests a second were answered.  It needs
nothing but Python.  Exit status 1 if a first choice differs or an answer
that was an error is one no more, or the other way round.
"""

from __future__ import annotations

import argparse
import http.client
import json
import threading
import time
from urllib.parse import urlparse


def duels(path: str, games: int):
    """Each seat of each game: its requests and the answers they got."""
    with open(path, encoding="utf8") as handle:
        for number, line in enumerate(handle):
            if number >= games:
                return
            for seat in json.loads(line)["game"]["model_seats"]:
                trace = seat["stats"]["trace"]
                if trace:
                    yield [(step["request"], step["answer"]) for step in trace]


def first(answer: dict) -> int | None:
    best = None
    for place, entry in enumerate(answer["predict_results"]["action_preds"]):
        if entry["prob"] != -1 and (best is None or entry["prob"] > best[1]):
            best = (place, entry["prob"])
    return None if best is None else best[0]


class Tally:
    def __init__(self) -> None:
        self.lock = threading.Lock()
        self.requests = self.same = self.choices = self.errors = self.failures = 0
        self.prob = self.value = 0.0

    def add(self, got: dict, wanted: dict) -> None:
        with self.lock:
            self.requests += 1
            if ("error" in got) != ("error" in wanted):
                self.errors += 1
                return
            if "error" in got:
                self.same += 1
                return
            mine, theirs = got["predict_results"], wanted["predict_results"]
            a = [entry["prob"] for entry in mine["action_preds"]]
            b = [entry["prob"] for entry in theirs["action_preds"]]
            if len(a) != len(b):
                self.failures += 1
                return
            self.same += a == b and mine["win_rate"] == theirs["win_rate"]
            self.prob = max(self.prob, max(abs(x - y) for x, y in zip(a, b)))
            self.value = max(self.value, abs(mine["win_rate"] - theirs["win_rate"]))
            self.choices += first(got) != first(wanted)


def replay(duel: list, address: tuple[str, int], protocol: str, tally: Tally) -> None:
    connection = http.client.HTTPConnection(*address, timeout=300)

    def post(path: str, body: dict) -> dict:
        connection.request("POST", path, json.dumps(body), {"Content-Type": "application/json"})
        return json.loads(connection.getresponse().read())

    if protocol == "v0":
        name = post("/v0/duels", {})["duelId"]
        for request, wanted in duel:
            tally.add(post(f"/v0/duels/{name}/predict", request), wanted)
        connection.request("DELETE", f"/v0/duels/{name}")
        connection.getresponse().read()
    else:
        state = None
        for request, wanted in duel:
            got = post("/v1/predict", {"input": request["input"], "prev_action_idx": request["prev_action_idx"], "state": state})
            state = got.get("state", state)
            tally.add(got, wanted)
    connection.close()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("traced")
    parser.add_argument("--server", default="http://127.0.0.1:3013")
    parser.add_argument("--protocol", choices=("v0", "v1"), default="v1")
    parser.add_argument("--games", type=int, default=24)
    parser.add_argument("--threads", type=int, default=1)
    parser.add_argument("--rounds", type=int, default=1, help="send everything this many times over, for the rate")
    args = parser.parse_args()
    url = urlparse(args.server)
    address = (url.hostname, url.port or 80)
    waiting = list(duels(args.traced, args.games)) * args.rounds
    tally = Tally()
    lock = threading.Lock()

    def work() -> None:
        while True:
            with lock:
                if not waiting:
                    return
                duel = waiting.pop()
            replay(duel, address, args.protocol, tally)

    started = time.time()
    threads = [threading.Thread(target=work) for _ in range(args.threads)]
    for thread in threads:
        thread.start()
    for thread in threads:
        thread.join()
    seconds = time.time() - started
    print(f"{tally.requests} requests through {args.protocol}, {args.threads} duels at a time: {tally.requests / seconds:.0f} a second")
    print(f"  the same answer to the last digit: {tally.same}")
    print(f"  largest gap in a probability {tally.prob:.2e}, in the estimate {tally.value:.2e}")
    print(f"  first choice differs: {tally.choices}; an error where there was none or none where there was one: {tally.errors}; answers of another length: {tally.failures}")
    raise SystemExit(bool(tally.choices or tally.errors or tally.failures))


if __name__ == "__main__":
    main()
