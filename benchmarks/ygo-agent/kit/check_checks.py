"""Read the README's full reference/mirror checks before interpreting pilot results."""
import argparse
import json
from pathlib import Path


def read(path):
    return json.loads(Path(path).read_text())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("references", help="summary JSON: ygo-agent against random and first")
    parser.add_argument("mirror", help="summary JSON: ygo-agent against itself")
    parser.add_argument("--native", default=str(Path(__file__).with_name("native-values-22750M.json")))
    args = parser.parse_args()
    refs, mirror, native = read(args.references), read(args.mirror), read(args.native)
    failures = []

    def check(ok, message):
        print(("PASS " if ok else "FAIL ") + message)
        if not ok:
            failures.append(message)

    for report in (refs, mirror):
        check(report["failures"] == 0, "no failed engine games")
        check(report["metadata"]["rules"].startswith("MasterRule5"), "Master Rule 5")
    for opponent in ("random", "first"):
        matches = [m for m in refs["matchups"] if {m["a"], m["b"]} == {"ygo-agent", opponent}]
        check(len(matches) == 1, f"one model matchup against {opponent}")
        if len(matches) != 1:
            continue
        m = matches[0]
        games = m["games"] - m["failures"]
        wins = m["wins"] if m["a"] == "ygo-agent" else m["losses"]
        check(games >= 1000, f"{opponent}: at least 1,000 complete games")
        check(wins >= 0.98 * games, f"{opponent}: {wins}/{games} model wins; minimum 98%")
        check(all(s["games"] >= 500 for s in m["by_seat"]), f"{opponent}: both seats represented")
    matches = [m for m in mirror["matchups"] if m["a"] == m["b"] == "ygo-agent"]
    check(len(matches) == 1, "one model mirror matchup")
    if len(matches) == 1:
        m = matches[0]
        check(m["games"] - m["failures"] >= 1000, "at least 1,000 complete mirror games")
        rate = m["model"]["second_seat_win_share"]
        check(rate is not None and 0.48 <= rate <= 0.56, f"second-seat win share {rate}; required 48 to 56%")
        for measured, baseline in zip(m["model"]["calibration"], native["bands"], strict=True):
            n = measured["decisions"]
            if n >= 2000:
                observed = measured["observed_win_share"]
                check(abs(observed - baseline["won"]) <= 0.05,
                      f"band {baseline['from']:.1f}-{baseline['to']:.1f}: {observed:.3%} vs {baseline['won']:.3%} ({n} decisions)")
            else:
                print(f"INFO band {baseline['from']:.1f}-{baseline['to']:.1f}: {n} decisions; below 2,000")
    for report in (refs, mirror):
        for m in report["matchups"]:
            stats = m["model"]
            check(stats["asked"] > 0 and stats["server_errors"] < 0.01 * stats["asked"],
                  f"{m['a']} vs {m['b']}: {stats['server_errors']}/{stats['asked']} server errors; required under 1%")
            print("INFO fallback reasons:", json.dumps(stats["fallback_reasons"], sort_keys=True))
            check(all(count < m["games"] for count in stats["fallback_games_by_kind"].values()),
                  f"{m['a']} vs {m['b']}: no fallback kind occurring in every game")
    print("MR1 parity omitted at the user's direction.")
    raise SystemExit(bool(failures))


if __name__ == "__main__":
    main()
