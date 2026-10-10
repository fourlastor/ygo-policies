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
#   FRONTS=                 processes of the server that take the requests:
#                           two thirds of this machine's processors
#   WORLDS=8 CONFIRM=32 FINAL=96   the search's stages; fewer worlds, fewer requests
#   STAND_IN=ygo-agent      who plays the model's seat in the try-outs: the
#                           model itself, or a policy such as blue-eyes.  With
#                           a policy the run is the one to compare with, deal
#                           by deal: on the same server the plain games of
#                           the two runs are the same games (minutes, not
#                           hours: a try-out then asks the server nothing)
#   DEVICE=gpu              cpu works, twenty times slower
#   CHECKPOINT=0546_22750M  PORT=3013
#   OUT=benchmarks/ygo-agent/runs/search-model   where the rows and the report go
#   AGENT=/tmp/ygo-agent VENV=/tmp/ygo-agent-venv   where setup.sh put ygo-agent
#
# A game asks the server some 180,000 times with the stages as they are, 250
# times with the pilot standing in for the model, and a game's requests go one
# after another: the first games are over in minutes, the longest take hours
# whatever runs beside them.  How fast the run goes shows in the server's log,
# a line a minute:
#
#   tail -f benchmarks/ygo-agent/runs/search-model/server.log
#
# See ../SEARCH.md for what the lines say and what a run costs.
set -euo pipefail
KIT=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$KIT/../../.." && pwd)
# The bench runs from the checkout and the server from ygo-agent's folder: a
# path given from where this script is started is made whole first.
absolute() {
  case $1 in
    /*) printf '%s\n' "$1" ;;
    *) printf '%s/%s\n' "$PWD" "$1" ;;
  esac
}
AGENT=$(absolute "${AGENT:-/tmp/ygo-agent}")
VENV=$(absolute "${VENV:-/tmp/ygo-agent-venv}")
GAMES=${GAMES:-200} SEED=${SEED:-860000}
WORKERS=${WORKERS:-96} BATCH=${BATCH:-32}
CORES=$(nproc)
FRONTS=${FRONTS:-$((CORES * 2 / 3 > 2 ? CORES * 2 / 3 : 2))}
WORLDS=${WORLDS:-8} CONFIRM=${CONFIRM:-32} FINAL=${FINAL:-96}
STAND_IN=${STAND_IN:-ygo-agent}
DEVICE=${DEVICE:-gpu} CHECKPOINT=${CHECKPOINT:-0546_22750M} PORT=${PORT:-3013}
OUT=$(absolute "${OUT:-$ROOT/benchmarks/ygo-agent/runs/search-model}")

if [ ! -f "$AGENT/scripts/checkpoints/$CHECKPOINT.flax_model" ] || [ ! -x "$VENV/bin/python" ]; then
  echo "ygo-agent is not set up in $AGENT with $VENV: run kit/setup.sh first" >&2
  exit 1
fi
# The engine and the pilots are compiled in, the scripts and the cards are
# read when a duel starts: all of them at the commits this checkout pins.
pins=$(git -C "$ROOT" submodule status --recursive)
if grep -q '^[-+U]' <<< "$pins"; then
  echo "a submodule is not at the commit this checkout pins; run" >&2
  echo "  git -C $ROOT submodule update --init --recursive" >&2
  exit 1
fi
mkdir -p "$OUT"
if [ -e "$OUT/search-model.jsonl" ]; then
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
# The server says that it is serving once all its fronts are up. The first of
# them answers a moment before that: an answer is not the sign.
for _ in $(seq 1 600); do
  grep -q '^serving' "$OUT/server.log" && break
  if ! kill -0 "$server" 2> /dev/null; then
    echo "the server did not start; the end of $OUT/server.log:" >&2
    tail -5 "$OUT/server.log" >&2
    exit 1
  fi
  sleep 0.5
done
if ! grep '^serving' "$OUT/server.log"; then
  echo "the server is not up after five minutes; the end of $OUT/server.log:" >&2
  tail -5 "$OUT/server.log" >&2
  exit 1
fi

echo "$GAMES deals, $WORKERS at once; the server says how far it is every minute in $OUT/server.log"
(cd "$ROOT" && target/release/policy-bench search --rules mr5 \
  --policies blue-eyes --opponents ygo-agent --stand-in "$STAND_IN" --carried true \
  --server "http://127.0.0.1:$PORT" --games "$GAMES" --seed "$SEED" --workers "$WORKERS" \
  --worlds "$WORLDS" --confirm "$CONFIRM" --final "$FINAL" \
  --record true --output "$OUT/search-model.jsonl")

grep 'requests put to the model' "$OUT/server.log" | tail -1 || true
python3 "$KIT/report.py" "$OUT/search-model.jsonl" | tee "$OUT/report.txt"
echo "The rows, the server's log and this report are in $OUT."
