#!/usr/bin/env bash
# The Blue-Eyes pilot with a search against ygo-agent's model, the model
# itself playing its seat in the search's try-outs.
#
#   bash benchmarks/ygo-agent/kit/search-model.sh
#
# Builds the bench, starts the server on the GPU with the model answering
# many requests at once, plays the deals and prints the result.  ygo-agent
# must be set up first, with the GPU build of JAX:
#
#   GPU=1 bash benchmarks/ygo-agent/kit/setup.sh
#
# What it does is set by the environment:
#
#   GAMES=200 SEED=860000   the deals: the first GAMES of measured/22750M-pilot
#   WORKERS=96              deals played at once; each waits on the server
#   BATCH=32                requests the model answers together; every batch
#                           costs the same, full or not (the server's log says
#                           how full they are)
#   FRONTS=8                processes of the server that take the requests
#   WORLDS=8 CONFIRM=32 FINAL=96   the search's stages; fewer worlds, fewer requests
#   DEVICE=gpu              cpu works, twenty times slower
#   CHECKPOINT=0546_22750M  PORT=3013
#   OUT=benchmarks/ygo-agent/runs/search-model   where the rows and the report go
#   AGENT=/tmp/ygo-agent VENV=/tmp/ygo-agent-venv   where setup.sh put ygo-agent
#
# A game asks the server some 180,000 times with the stages as they are, 250
# times with the pilot standing in for the model; see ../SEARCH.md.
set -euo pipefail
KIT=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$KIT/../../.." && pwd)
AGENT=${AGENT:-/tmp/ygo-agent}
VENV=${VENV:-/tmp/ygo-agent-venv}
GAMES=${GAMES:-200} SEED=${SEED:-860000}
WORKERS=${WORKERS:-96} BATCH=${BATCH:-32} FRONTS=${FRONTS:-8}
WORLDS=${WORLDS:-8} CONFIRM=${CONFIRM:-32} FINAL=${FINAL:-96}
DEVICE=${DEVICE:-gpu} CHECKPOINT=${CHECKPOINT:-0546_22750M} PORT=${PORT:-3013}
OUT=${OUT:-$ROOT/benchmarks/ygo-agent/runs/search-model}

if [ ! -f "$AGENT/scripts/checkpoints/$CHECKPOINT.flax_model" ] || [ ! -x "$VENV/bin/python" ]; then
  echo "ygo-agent is not set up in $AGENT with $VENV: run kit/setup.sh first" >&2
  exit 1
fi
mkdir -p "$OUT"
if [ -s "$OUT/search-model.jsonl" ]; then
  echo "$OUT/search-model.jsonl exists: name another OUT" >&2
  exit 1
fi

(cd "$ROOT" && cargo build --release -p ygo-policies-bench -p ygo-policies-ffi)

# The server, stopped again when this script ends however it ends.
(cd "$AGENT/scripts" && exec "$VENV/bin/python" -u "$KIT/serve.py" \
  --checkpoint "checkpoints/$CHECKPOINT.flax_model" --port "$PORT" \
  --device "$DEVICE" --batch "$BATCH" --fronts "$FRONTS" --report 60) > "$OUT/server.log" 2>&1 &
server=$!
trap 'kill "$server" 2> /dev/null || true' EXIT
for _ in $(seq 1 150); do
  if curl -s -m 2 "http://127.0.0.1:$PORT/" > /dev/null 2>&1; then
    break
  fi
  if ! kill -0 "$server" 2> /dev/null; then
    echo "the server did not start: see $OUT/server.log" >&2
    exit 1
  fi
  sleep 2
done
grep '^serving' "$OUT/server.log" || { echo "the server does not answer: see $OUT/server.log" >&2; exit 1; }

(cd "$ROOT" && target/release/policy-bench search --rules mr5 \
  --policies blue-eyes --opponents ygo-agent --stand-in ygo-agent \
  --server "http://127.0.0.1:$PORT" --games "$GAMES" --seed "$SEED" --workers "$WORKERS" \
  --worlds "$WORLDS" --confirm "$CONFIRM" --final "$FINAL" \
  --record true --output "$OUT/search-model.jsonl")

grep 'requests put to the model' "$OUT/server.log" | tail -1 || true
python3 "$KIT/report.py" "$OUT/search-model.jsonl" | tee "$OUT/report.txt"
echo "The rows, the server's log and this report are in $OUT."
