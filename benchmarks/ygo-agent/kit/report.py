"""The model's results from the rows the bench wrote.

    python3 benchmarks/ygo-agent/kit/report.py RUN.jsonl [RUN.jsonl ...]

For each matchup of each file: the games the model won, lost and drew, its
share with a standard error, the same for the games it went first and those
it went second, how long the games were and how much the server was asked.
Where two files hold the same matchup on the same deals (two checkpoints, or
two versions of the seat), the difference between them deal by deal.
"""

from __future__ import annotations

import json
import sys
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


def outcome(row: dict) -> tuple[int, float] | None:
    """The model's seat in a game (the named player's in a mirror) and what
    it got: 1 a win, 0 a loss, 0.5 no winner.  None for a game that failed."""
    game = row["game"]
    if "error" in game:
        return None
    seat = row["seat"] if row["a"] == MODEL else 1 - row["seat"]
    winner = game["winner"]
    return seat, 0.5 if winner not in (0, 1) else float(winner == seat)


def main() -> None:
    runs = {Path(arg).stem: load(Path(arg)) for arg in sys.argv[1:]}
    for name, matchups in runs.items():
        for (a, b), rows in matchups.items():
            other = b if a == MODEL else a
            played = [(row, outcome(row)) for row in rows]
            done = [(row, got) for row, got in played if got is not None]
            wins = sum(got[1] == 1 for _, got in done)
            draws = sum(got[1] == 0.5 for _, got in done)
            print(f"{name}: the model against {other}, {len(rows)} games ({len(rows) - len(done)} failed)")
            print(f"  won {wins}, lost {len(done) - wins - draws}, no winner {draws}: {share(wins, len(done))}")
            for seat, label in ((0, "going first"), (1, "going second")):
                mine = [got for _, got in done if got[0] == seat]
                print(f"    {label}: {sum(g[1] == 1 for g in mine)} of {len(mine)}, {share(sum(g[1] == 1 for g in mine), len(mine))}")
            turns = sum(row["game"]["turns"] for row, _ in done) / max(len(done), 1)
            asked = errors = 0
            kinds: dict[str, int] = {}
            for row, _ in done:
                for seat in row["game"].get("model_seats", []):
                    stats = seat["stats"]
                    asked += stats["asked"]
                    errors += stats["server_errors"]
                    for kind, count in stats["fallback_reasons"].items():
                        kinds[kind] = kinds.get(kind, 0) + count
            seats = sum(len(row["game"].get("model_seats", [])) for row, _ in done)
            print(f"  {turns:.1f} turns a game; {asked / max(seats, 1):.0f} requests a seat a game, {errors} not answered", end="")
            print(f" ({errors / asked:.2%})" if asked else "")
            for kind, count in sorted(kinds.items(), key=lambda item: -item[1]):
                print(f"    {count:>6}  {kind}")
    names = list(runs)
    for i, first in enumerate(names):
        for second in names[i + 1 :]:
            for pair in runs[first].keys() & runs[second].keys():
                if pair == (MODEL, MODEL):
                    continue
                one = {row["seed"]: outcome(row) for row in runs[first][pair]}
                two = {row["seed"]: outcome(row) for row in runs[second][pair]}
                deals = [seed for seed in one.keys() & two.keys() if one[seed] is not None and two[seed] is not None]
                if len(deals) < 2:
                    continue
                gaps = [one[seed][1] - two[seed][1] for seed in deals]
                mean = sum(gaps) / len(gaps)
                error = sqrt(sum((gap - mean) ** 2 for gap in gaps) / (len(gaps) - 1) / len(gaps))
                same = sum(one[seed][1] == two[seed][1] for seed in deals)
                print(
                    f"{first} against {second} on the same {len(deals)} deals of {pair[0]} and {pair[1]}: "
                    f"{mean:+.1%} ± {error:.1%} for {first}; the same result in {same}"
                )


if __name__ == "__main__":
    main()
