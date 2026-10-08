"""The results from the rows the bench wrote.

    python3 benchmarks/ygo-agent/kit/report.py RUN.jsonl [RUN.jsonl ...]

Rows of `matchup`: for each matchup of each file, the games the model won,
lost and drew, its share with a standard error, the same for the games it
went first and those it went second, how long the games were and how much
the server was asked.  Where two files hold the same matchup on the same
deals (two checkpoints, or two versions of the seat), the difference between
them deal by deal.

Rows of `search`, which hold each deal twice: the searching side's share
alone and with the search, the difference deal by deal, what the search did
in a game, and whether a searched game in which no answer was changed is the
plain game.
"""

from __future__ import annotations

import json
import sys
from collections import Counter
from math import sqrt
from pathlib import Path

MODEL = "ygo-agent"


def share(wins: int, games: int) -> str:
    if not games:
        return "-"
    p = wins / games
    return f"{p:6.1%} ± {sqrt(p * (1 - p) / games):.1%}"


def load(path: Path) -> dict[tuple[str, str], list[dict]]:
    found: dict[tuple[str, str], list[dict]] = {}
    with open(path, encoding="utf8") as handle:
        for line in handle:
            row = json.loads(line)
            found.setdefault((row["a"], row["b"]), []).append(row)
    return found


def got(game: dict, seat: int) -> float | None:
    """What `seat` got of a game: 1 a win, 0 a loss, 0.5 no winner.  None
    for a game that failed."""
    if "error" in game:
        return None
    winner = game["winner"]
    return 0.5 if winner not in (0, 1) else float(winner == seat)


def outcome(row: dict) -> tuple[int, float] | None:
    """The model's seat in a game (the named player's in a mirror) and what
    it got."""
    seat = row["seat"] if row["a"] == MODEL else 1 - row["seat"]
    result = got(row["game"], seat)
    return None if result is None else (seat, result)


def paired(gaps: list[float]) -> str:
    mean = sum(gaps) / len(gaps)
    error = sqrt(sum((gap - mean) ** 2 for gap in gaps) / (len(gaps) - 1) / len(gaps))
    return f"{mean:+.1%} ± {error:.1%}"


def asked(games: list[dict]) -> tuple[int, int, int, dict[str, int]]:
    """Requests, requests not answered, seats, and the kinds not answered."""
    requests = errors = seats = 0
    kinds: dict[str, int] = {}
    for game in games:
        for seat in game.get("model_seats", []):
            stats = seat["stats"]
            seats += 1
            requests += stats["asked"]
            errors += stats["server_errors"]
            for kind, count in stats["fallback_reasons"].items():
                kinds[kind] = kinds.get(kind, 0) + count
    return requests, errors, seats, kinds


def print_asked(games: list[dict]) -> None:
    requests, errors, seats, kinds = asked(games)
    if not seats:
        return
    print(f"  the model: {requests / seats:.0f} requests a seat a game, {errors} not answered ({errors / requests:.2%})")
    for kind, count in sorted(kinds.items(), key=lambda item: -item[1]):
        print(f"    {count:>6}  {kind}")


def matchup(name: str, pair: tuple[str, str], rows: list[dict]) -> None:
    other = pair[1] if pair[0] == MODEL else pair[0]
    done = [(row, result) for row in rows if (result := outcome(row)) is not None]
    wins = sum(result[1] == 1 for _, result in done)
    draws = sum(result[1] == 0.5 for _, result in done)
    print(f"{name}: the model against {other}, {len(rows)} games ({len(rows) - len(done)} failed)")
    print(f"  won {wins}, lost {len(done) - wins - draws}, no winner {draws}: {share(wins, len(done))}")
    for seat, label in ((0, "going first"), (1, "going second")):
        mine = [result for _, result in done if result[0] == seat]
        print(f"    {label}: {sum(r[1] == 1 for r in mine)} of {len(mine)}, {share(sum(r[1] == 1 for r in mine), len(mine))}")
    print(f"  {sum(row['game']['turns'] for row, _ in done) / max(len(done), 1):.1f} turns a game")
    print_asked([row["game"] for row, _ in done])


def searched(name: str, pair: tuple[str, str], rows: list[dict]) -> None:
    done = [row for row in rows if got(row["baseline"], row["seat"]) is not None and got(row["search"], row["seat"]) is not None]
    stand_in = done[0]["search"].get("stand_in") if done else None
    print(f"{name}: {pair[0]} with a search against {pair[1]}, {len(rows)} deals ({len(rows) - len(done)} failed)"
          + (f"; in the try-outs {pair[1]} is played by {stand_in}" if stand_in else ""))
    for label, text in (("baseline", "alone"), ("search", "with the search")):
        results = [(row["seat"], got(row[label], row["seat"])) for row in done]
        wins = sum(result == 1 for _, result in results)
        draws = sum(result == 0.5 for _, result in results)
        print(f"  {text}: won {wins}, lost {len(done) - wins - draws}, no winner {draws}: {share(wins, len(done))}")
        for seat, where in ((0, "going first"), (1, "going second")):
            mine = [result for at, result in results if at == seat]
            print(f"    {where}: {sum(r == 1 for r in mine)} of {len(mine)}, {share(sum(r == 1 for r in mine), len(mine))}")
    if len(done) > 1:
        gaps = [got(row["search"], row["seat"]) - got(row["baseline"], row["seat"]) for row in done]
        gained = sum(gap > 0 for gap in gaps)
        lost = sum(gap < 0 for gap in gaps)
        print(f"  deal by deal the search is worth {paired(gaps)}: {gained} deals turned its way, {lost} against")
    games = [row["search"] for row in done]
    count = max(len(games), 1)
    mean = lambda key: sum(game[key] for game in games) / count  # noqa: E731
    changed = Counter(deviation["kind"] for game in games for deviation in game["deviations"])
    print(f"  a searched game: {mean('seconds'):.0f} seconds, {mean('searched'):.1f} decisions searched, "
          f"{mean('in_chain'):.1f} left to the pilot in a chain and {mean('facing_set'):.1f} facing a face-down monster, "
          f"{mean('playouts'):.0f} try-outs ({sum(game['failed_playouts'] for game in games)} failed in all)")
    print(f"  answers changed: {sum(changed.values()) / count:.2f} a game", dict(changed.most_common()))
    quiet = [row for row in done if not row["search"]["deviations"]]
    same = sum(row["search"]["digest"] == row["baseline"]["digest"] for row in quiet)
    print(f"  searched games with no answer changed: {len(quiet)}, of which {same} are the plain game answer for answer")
    print_asked([row[label] for row in done for label in ("baseline", "search")])


def main() -> None:
    runs = {Path(arg).stem: load(Path(arg)) for arg in sys.argv[1:]}
    plain: dict[str, dict[tuple[str, str], list[dict]]] = {}
    for name, matchups in runs.items():
        for pair, rows in matchups.items():
            if "search" in rows[0]:
                searched(name, pair, rows)
            else:
                matchup(name, pair, rows)
                plain.setdefault(name, {})[pair] = rows
    names = list(plain)
    for i, first in enumerate(names):
        for second in names[i + 1 :]:
            for pair in plain[first].keys() & plain[second].keys():
                if pair == (MODEL, MODEL):
                    continue
                one = {row["seed"]: outcome(row) for row in plain[first][pair]}
                two = {row["seed"]: outcome(row) for row in plain[second][pair]}
                deals = [seed for seed in one.keys() & two.keys() if one[seed] is not None and two[seed] is not None]
                if len(deals) < 2:
                    continue
                gaps = [one[seed][1] - two[seed][1] for seed in deals]
                same = sum(one[seed][1] == two[seed][1] for seed in deals)
                print(
                    f"{first} against {second} on the same {len(deals)} deals of {pair[0]} and {pair[1]}: "
                    f"{paired(gaps)} for {first}; the same result in {same}"
                )


if __name__ == "__main__":
    main()
