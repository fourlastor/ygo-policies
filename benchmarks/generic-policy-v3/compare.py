"""Per-deck wins of several generic-mode runs on the same schedule, paired by (deck, seed, seat)."""
import gzip, json, sys, math, statistics, os
from collections import defaultdict

def load(path):
    rows = {}
    for line in (gzip.open(path, 'rt') if path.endswith('.gz') else open(path)):
        r = json.loads(line)
        rows[(r['deck'], r['seed'], r['seat'])] = r
    return rows

def win(r):
    g = r['game']
    return 1 if g['winner'] == r['seat'] else 0

paths = sys.argv[1:]
names = [os.path.basename(p).replace('.gz', '').replace('.jsonl', '') for p in paths]
runs = [load(p) for p in paths]
keys = sorted(set.intersection(*[set(r) for r in runs]))
decks = sorted({k[0] for k in keys})
print(f"{'deck':<16}" + ''.join(f"{n:>18}" for n in names))
totals = [0] * len(runs)
for deck in decks:
    ks = [k for k in keys if k[0] == deck]
    cells = []
    for i, run in enumerate(runs):
        w = sum(win(run[k]) for k in ks)
        totals[i] += w
        cells.append(f"{w:>3}/{len(ks):<3}")
    print(f"{deck:<16}" + ''.join(f"{c:>18}" for c in cells))
n = len(keys)
print(f"{'total':<16}" + ''.join(f"{t:>8}/{n} {100*t/n:5.1f}%" for t in totals))
# paired difference of each run against the first
for i in range(1, len(runs)):
    d = [win(runs[i][k]) - win(runs[0][k]) for k in keys]
    mean = statistics.mean(d); sd = statistics.stdev(d)
    se = sd / math.sqrt(n)
    up = sum(1 for x in d if x > 0); down = sum(1 for x in d if x < 0)
    print(f"{names[i]} vs {names[0]}: {100*mean:+.1f} points [{100*(mean-1.96*se):+.1f}, {100*(mean+1.96*se):+.1f}], {up} gained, {down} lost")
# search telemetry
for name, run in zip(names, runs):
    g = [run[k]['game'] for k in keys]
    s = lambda key: sum(x.get(key, 0) or 0 for x in g)
    lat = [ms for x in g for ms in x.get('search_decision_ms', [])]
    lat.sort()
    q = lambda p: lat[min(len(lat) - 1, int(p * len(lat)))] if lat else 0
    devs = sum(len(x.get('deviations', [])) for x in g)
    secs = sum(run[k].get('seconds', 0) for k in keys)
    print(f"{name}: searched {s('searched')} facing_set {s('facing_set')} in_chain {s('in_chain')} unsafe {s('unsafe_selections')} deviations {devs} playouts {s('playouts')} failed {s('failed_playouts')} unresolved {s('unresolved_playouts')} | search ms p50 {q(.5):.0f} p95 {q(.95):.0f} max {lat[-1] if lat else 0:.0f} | game seconds total {secs:.0f}")
