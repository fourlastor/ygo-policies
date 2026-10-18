"""Whole-roster summary of generic-mode runs on the same schedule: decks at the
55% mark (observed), the aggregate without the exempt decks, the weakest decks."""
import gzip, json, math, sys, os

def load(path):
    f = gzip.open(path, 'rt') if path.endswith('.gz') else open(path)
    return {(r['deck'], r['seed'], r['seat']): r for r in map(json.loads, f)}

def wilson(w, n, z=1.96):
    p = w / n; d = 1 + z * z / n
    c = (p + z * z / (2 * n)) / d; h = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / d
    return c - h, c + h

exempt = {'countdown'}
paths = sys.argv[1:]
names = [os.path.basename(p).replace('.gz', '').replace('.jsonl', '') for p in paths]
runs = [load(p) for p in paths]
keys = sorted(set.intersection(*[set(r) for r in runs]))
decks = sorted({k[0] for k in keys})
win = lambda r: int(r['game']['winner'] == r['seat'])
table = {}
for d in decks:
    ks = [k for k in keys if k[0] == d]
    table[d] = [(sum(win(run[k]) for k in ks), len(ks)) for run in runs]
for i, name in enumerate(names):
    req = [d for d in decks if d not in exempt]
    w = sum(table[d][i][0] for d in req); n = sum(table[d][i][1] for d in req)
    at = sum(1 for d in req if table[d][i][0] / table[d][i][1] >= 0.55)
    lb = sum(1 for d in req if wilson(*table[d][i])[0] >= 0.55)
    ub = sum(1 for d in req if wilson(*table[d][i])[1] < 0.55)
    print(f"{name:<14} {w}/{n} = {100*w/n:.1f}% over {len(req)} decks; observed >=55%: {at}; lower bound >=55%: {lb}; upper bound <55%: {ub}")
order = sorted(decks, key=lambda d: table[d][-1][0])
print(f"\n{'deck':<16}" + ''.join(f"{n:>14}" for n in names))
for d in order:
    print(f"{d:<16}" + ''.join(f"{w:>9}/{n:<4}" for w, n in table[d]))
