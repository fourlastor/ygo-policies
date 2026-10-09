"""The curve of a run of `curve.sh`: what ygo-agent's trainer reached after so
many steps, against the pilot and against its own released model.

    python3 benchmarks/ygo-agent/kit/curve.py RUN [--rows FOLDER]

RUN is the folder the run left: `kept.tsv` (the steps of each checkpoint kept
and the seconds of training until then), `train.log` (their trainer's lines),
`pilot/*.jsonl` (the rows of each checkpoint's games against the pilot) and
`mirror/*.txt` (what battle.py said of its games against the released model).
`--rows` names another folder of RUN than `pilot`: the same checkpoints
measured against another pilot, or another version of one.

Games are estimated: their trainer says every few updates how many steps the
games that ended lately took, and a stretch of steps is divided by that.
"""

from __future__ import annotations

import re
import sys
from math import exp, log, sqrt
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from report import MODEL, load, outcome  # noqa: E402

SAID = re.compile(r"^global_step=(\d+), avg_return=\S+ avg_length=(\d+)")
WON = re.compile(r"win_rate=([0-9.]+)")


def games_until(log_path: Path) -> list[tuple[int, float]]:
    """The games played until each of the trainer's lines, by its steps."""
    found = [(0, 0.0)]
    if not log_path.exists():
        return found
    for line in log_path.read_text(encoding="utf8", errors="replace").splitlines():
        said = SAID.match(line)
        if said and int(said[2]) > 0 and int(said[1]) > found[-1][0]:
            steps = int(said[1])
            found.append((steps, found[-1][1] + (steps - found[-1][0]) / int(said[2])))
    return found


def games_at(told: list[tuple[int, float]], steps: int) -> float | None:
    """The games after `steps`, between the two lines around them; past the
    last line at its pace."""
    if len(told) < 2:
        return None
    for (before, low), (after, high) in zip(told, told[1:]):
        if steps <= after:
            return low + (high - low) * (steps - before) / (after - before)
    (before, low), (after, high) = told[-2], told[-1]
    return high + (high - low) * (steps - after) / (after - before)


def against_the_pilot(rows: Path, pilots: set[str]) -> tuple[float, int, int] | None:
    """What the model got (a game without a winner counts half), the games
    that were played, and those that failed.  The policies it met are added
    to `pilots`."""
    if not rows.exists():
        return None
    got = played = failed = 0.0
    for sides, games in load(rows).items():
        pilots.update(side for side in sides if side != MODEL)
        for row in games:
            result = outcome(row)
            if result is None:
                failed += 1
            else:
                played += 1
                got += result[1]
    return got, int(played), int(failed)


def against_the_model(result: Path) -> tuple[float, int] | None:
    """The share battle.py gave the checkpoint, and of how many games."""
    if not result.exists():
        return None
    text = result.read_text(encoding="utf8", errors="replace")
    shares = WON.findall(text)
    games = re.search(r"^games=(\d+)", text, re.M)
    return (float(shares[-1]), int(games[1]) if games else 0) if shares else None


def share(part: float, games: int) -> str:
    error = sqrt(part * (1 - part) / games) if games else 0.0
    return f"{part:6.1%} ± {error:.1%}"


def crossing(points: list[tuple[int, float]], level: float = 0.5) -> tuple[int, int, float] | None:
    """The first two checkpoints the share passes `level` between, and the
    steps at which a line through them, over the steps' logarithm, meets it."""
    for (before, low), (after, high) in zip(points, points[1:]):
        if low < level <= high:
            place = (level - low) / (high - low)
            return before, after, exp(log(before) + place * (log(after) - log(before)))
    return None


def report(run: Path, rows: str = "pilot") -> None:
    kept = [
        (int(steps), int(seconds))
        for steps, seconds in (
            line.split("\t") for line in (run / "kept.tsv").read_text().splitlines() if line
        )
    ]
    told = games_until(run / "train.log")
    print(f"ygo-agent's trainer from scratch on the Blue-Eyes deck against itself: {run}")
    lines: list[str] = []
    pilots: set[str] = set()
    points: list[tuple[int, float]] = []
    reached: dict[int, tuple[float | None, int]] = {}
    for steps, seconds in kept:
        name = f"steps-{steps:012d}"
        games = games_at(told, steps)
        reached[steps] = (games, seconds)
        pilot = against_the_pilot(run / rows / f"{name}.jsonl", pilots)
        model = against_the_model(run / "mirror" / f"{name}.txt")
        if pilot is None:
            pilot_said = "not played"
        else:
            got, played, failed = pilot
            pilot_said = f"{got:g} of {played}, {share(got / played, played)}"
            pilot_said += f" ({failed} failed)" if failed else ""
            points.append((steps, got / played))
        model_said = "not played" if model is None else share(model[0], model[1])
        lines.append(
            f"  {steps:>13,}  {'-' if games is None else format(round(games), ','):>9}  "
            f"{seconds / 3600:6.2f}   {pilot_said:<28}  {model_said}"
        )
    print(f"the pilot: {', '.join(sorted(pilots)) or 'not played'} (the rows of `{rows}`)")
    print(
        f"  {'steps':>13}  {'games':>9}  {'hours':>6}   "
        f"{'against the pilot':<28}  against the released model"
    )
    print("\n".join(lines))
    if not points:
        return
    print()
    passed = crossing(points)
    if passed is None:
        best = max(points, key=lambda point: point[1])
        side = "under" if best[1] < 0.5 else "over"
        print(
            f"the pilot's level (half the games) is not passed between these checkpoints: "
            f"the model is {side} it throughout, {best[1]:.1%} at its best after {best[0]:,} steps"
        )
        return
    before, after, steps = passed
    # Games by the trainer's own lines; hours at the pace between the two.
    games = games_at(told, round(steps))
    seconds = reached[before][1] + (reached[after][1] - reached[before][1]) * (steps - before) / (
        after - before
    )
    print(
        f"the pilot's level (half the games) is passed between {before:,} and {after:,} steps: "
        f"at about {round(steps):,} steps, {'-' if games is None else format(round(games), ',')} "
        f"games, {seconds / 3600:.1f} hours of this training"
    )


if __name__ == "__main__":
    arguments = sys.argv[1:]
    folder = "pilot"
    if "--rows" in arguments:
        place = arguments.index("--rows")
        if place + 1 >= len(arguments):
            sys.exit(__doc__)
        folder = arguments[place + 1]
        del arguments[place : place + 2]
    if len(arguments) != 1:
        sys.exit(__doc__)
    report(Path(arguments[0]), folder)
