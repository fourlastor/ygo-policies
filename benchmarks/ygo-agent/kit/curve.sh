#!/usr/bin/env bash
# How fast ygo-agent learns, measured against the pilot: its own trainer from
# scratch on the Blue-Eyes deck against itself, and what it saved on the way
# against the Blue-Eyes pilot and against its released model.
#
#   HOURS=10 bash benchmarks/ygo-agent/kit/curve.sh
#
# 1. Trains for HOURS with the settings their README gives for one GPU
#    (cleanba.py from scratch, self-play, one deck).  Their trainer saves a
#    checkpoint every 2^20 steps and keeps the last two, so one is copied as
#    it is written: after about 1, 2, 3, 5, 7, 10, 14, 20 ... million steps,
#    each two fifths further than the one before, and the last.
# 2. Plays every checkpoint kept against the pilot on the deals of
#    measured/22750M-pilot, and the released model on the same deals: where
#    the curve is heading.
# 3. Plays every checkpoint kept against the released model in their own
#    environment (battle.py).
# 4. Prints the curve: steps, games and hours of training against the share
#    of the games won, and where the pilot's level is passed.
#
# A step already done is not done again.  Training cannot be taken up where
# it stopped: a run that was interrupted is measured as far as it got with
#
#   TRAIN=0 bash benchmarks/ygo-agent/kit/curve.sh
#
# which also measures a run that is still training, beside it (then with
# DEVICE=cpu, the card being taken).  ygo-agent must be set up first with
# the GPU build of JAX:  GPU=1 bash benchmarks/ygo-agent/kit/setup.sh
#
# What it does is set by the environment:
#
#   HOURS=10                how long the training goes on
#   STEPS=                  and the steps it stops after, if sooner
#   ENVS=16 MINIBATCHES=8 RATE=1e-4   their README's: duels of each of the
#                           two actors, parts an update's steps are split in,
#                           learning rate.  A card that runs out of memory
#                           takes more MINIBATCHES
#   GAMES=400 SEED=860000   the games of a checkpoint against the pilot
#   PILOT=blue-eyes         the policy it plays them against: the pilot of
#                           step 2, or blue-eyes-improved
#   ROWS=pilot              the folder of OUT those games' rows go to.  With
#                           TRAIN=0 and another name, the checkpoints a run
#                           kept are measured again beside the first time:
#                           against another pilot, or another version of one
#   MIRROR=256              its games against the released model; 0 for none
#   WORKERS=32 BATCH=32 FRONTS=   as in search-model.sh
#   DEVICE=gpu              the server's and battle.py's; cpu works, slower
#   CHECKPOINT=0546_22750M  the released model  PORT=3014
#   OUT=benchmarks/ygo-agent/runs/curve   everything the run leaves
#   AGENT=/tmp/ygo-agent VENV=/tmp/ygo-agent-venv   where setup.sh put ygo-agent
set -euo pipefail
KIT=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$KIT/../../.." && pwd)
# The steps work from several folders: a path given from where this script
# is started is made whole first.
absolute() {
  case $1 in
    /*) printf '%s\n' "$1" ;;
    *) printf '%s/%s\n' "$PWD" "$1" ;;
  esac
}
AGENT=$(absolute "${AGENT:-/tmp/ygo-agent}")
VENV=$(absolute "${VENV:-/tmp/ygo-agent-venv}")
OUT=$(absolute "${OUT:-$ROOT/benchmarks/ygo-agent/runs/curve}")
TRAIN=${TRAIN:-1} HOURS=${HOURS:-10} STEPS=${STEPS:-}
ENVS=${ENVS:-16} MINIBATCHES=${MINIBATCHES:-8} RATE=${RATE:-1e-4}
GAMES=${GAMES:-400} SEED=${SEED:-860000} MIRROR=${MIRROR:-256}
PILOT=${PILOT:-blue-eyes} ROWS=${ROWS:-pilot}
WORKERS=${WORKERS:-32} BATCH=${BATCH:-32}
CORES=$(nproc)
FRONTS=${FRONTS:-$((CORES * 2 / 3 > 2 ? CORES * 2 / 3 : 2))}
DEVICE=${DEVICE:-gpu} CHECKPOINT=${CHECKPOINT:-0546_22750M} PORT=${PORT:-3014}

if [ ! -f "$AGENT/scripts/checkpoints/$CHECKPOINT.flax_model" ] || [ ! -x "$VENV/bin/python" ]; then
  echo "ygo-agent is not set up in $AGENT with $VENV: run kit/setup.sh first" >&2
  exit 1
fi
# An update of their trainer is 128 steps of each duel of the two actors, and
# it saves by its updates: every 2^20 steps here.  (SAVE, in updates, is for
# trying this script out: their trainer names a checkpoint by its millions
# of steps, and saves closer than that overwrite one another.)
UPDATE=$((2 * ENVS * 128))
SAVE=${SAVE:-$((1048576 / UPDATE))}
if ((ENVS < 1 || SAVE < 1)); then
  echo "ENVS is between 1 and 4096, and SAVE at least 1" >&2
  exit 1
fi
pins=$(git -C "$ROOT" submodule status --recursive)
if grep -q '^[-+U]' <<< "$pins"; then
  echo "a submodule is not at the commit this checkout pins; run" >&2
  echo "  git -C $ROOT submodule update --init --recursive" >&2
  exit 1
fi
case $ROWS in
  */* | checkpoints | saved | mirror | '')
    echo "ROWS names a folder of its own in $OUT" >&2
    exit 1
    ;;
esac
mkdir -p "$OUT/checkpoints" "$OUT/saved" "$OUT/$ROWS" "$OUT/mirror"

if [ "$TRAIN" != 0 ]; then
  if [ -e "$OUT/train.log" ]; then
    echo "$OUT/train.log exists: name another OUT, or TRAIN=0 to measure what it kept" >&2
    exit 1
  fi
  echo "1. Training for $HOURS hours; their trainer's own lines are in $OUT/train.log"
  limit=()
  [ -n "$STEPS" ] && limit=(--total_timesteps "$STEPS")
  (cd "$AGENT/scripts" && exec "$VENV/bin/python" -u cleanba.py \
    --actor-device-ids 0 --learner-device-ids 0 --deck ../assets/deck/BlueEyes.ydk \
    --local-num_envs "$ENVS" --num-minibatches "$MINIBATCHES" --learning-rate "$RATE" \
    --vloss_clip 1.0 --save_interval "$SAVE" --eval_interval 1000000000 \
    --seed 0 --tb_dir None --ckpt_dir "$OUT/saved" "${limit[@]}") > "$OUT/train.log" 2>&1 &
  trainer=$!
  trap 'kill "$trainer" 2> /dev/null || true' EXIT
  started=$(date +%s)
  until=$(awk -v start="$started" -v hours="$HOURS" 'BEGIN { printf "%d", start + hours * 3600 }')
  # Saves seen, and the steps of the last checkpoint kept.
  seen=0 last=0
  # keep STEPS FILE: a checkpoint of the trainer, copied under its steps.
  keep() {
    local speed
    speed=$(grep -o ' SPS: [0-9]*' "$OUT/train.log" | tail -1 | grep -o '[0-9]*$' || true)
    cp "$2" "$OUT/checkpoints/steps-$(printf '%012d' "$1").flax_model"
    printf '%s\t%s\n' "$1" $(($(date +%s) - started)) >> "$OUT/kept.tsv"
    last=$1
    echo "   kept after $1 steps, $((($(date +%s) - started) / 60)) minutes${speed:+, $speed steps a second}"
  }
  # The trainer's saves since the last look: its newest file is whole when
  # its line is written, an older one may be gone.
  look() {
    local saves steps
    mapfile -t saves < <(grep '^Saved model to ' "$OUT/train.log" || true)
    if ((${#saves[@]} > seen)); then
      seen=${#saves[@]}
      steps=$((seen * SAVE * UPDATE))
      if (($1 || steps * 5 >= last * 7)) && ((steps > last)); then
        keep "$steps" "${saves[seen - 1]#Saved model to }"
      fi
    fi
  }
  while kill -0 "$trainer" 2> /dev/null; do
    if (($(date +%s) >= until)); then
      echo "   $HOURS hours: stopping the trainer"
      # It does not answer to an interrupt.
      kill -TERM "$trainer" 2> /dev/null || true
      break
    fi
    look 0
    sleep "${POLL:-20}"
  done
  wait "$trainer" 2> /dev/null || true
  trap - EXIT
  # The last one saved, whether or not it is two fifths further.
  seen=$((seen > 0 ? seen - 1 : 0))
  look 1
  if ! ls "$OUT/checkpoints"/*.flax_model > /dev/null 2>&1; then
    echo "the trainer kept nothing: see $OUT/train.log" >&2
    exit 1
  fi
fi

(cd "$ROOT" && cargo build --release -p ygo-policies-bench -p ygo-policies-ffi)

# play NAME FILE: the model saved in FILE against the pilot, its rows under
# NAME in the folder of ROWS.  The server is on that model while its games
# are played, and stopped again.
play() {
  local name=$1 rows="$OUT/$ROWS/$1.jsonl"
  [ -e "$rows" ] && return 0
  (cd "$AGENT/scripts" && exec "$VENV/bin/python" -u "$KIT/serve.py" \
    --checkpoint "$2" --port "$PORT" --device "$DEVICE" --batch "$BATCH" \
    --fronts "$FRONTS") > "$OUT/$ROWS/$name.server.log" 2>&1 &
  server=$!
  trap 'kill "$server" 2> /dev/null || true' EXIT
  # The server says that it is serving once all its fronts are up. The
  # first of them answers a moment before that: an answer is not the sign.
  for _ in $(seq 1 600); do
    grep -q '^serving' "$OUT/$ROWS/$name.server.log" && break
    if ! kill -0 "$server" 2> /dev/null; then
      echo "the server did not start; the end of $OUT/$ROWS/$name.server.log:" >&2
      tail -5 "$OUT/$ROWS/$name.server.log" >&2
      exit 1
    fi
    sleep 0.5
  done
  if ! grep -q '^serving' "$OUT/$ROWS/$name.server.log"; then
    echo "the server is not up after five minutes; the end of $OUT/$ROWS/$name.server.log:" >&2
    tail -5 "$OUT/$ROWS/$name.server.log" >&2
    exit 1
  fi
  (cd "$ROOT" && target/release/policy-bench matchup --rules mr5 \
    --policies ygo-agent --opponents "$PILOT" --server "http://127.0.0.1:$PORT" \
    --carried true --games "$GAMES" --seed "$SEED" --workers "$WORKERS" \
    --output "$rows.part") | tail -1 | sed "s/^/   $name: /"
  mv "$rows.part" "$rows"
  kill "$server" 2> /dev/null || true
  wait "$server" 2> /dev/null || true
  trap - EXIT
}

echo "2. Each checkpoint against $PILOT, $GAMES games, and the released model"
for kept in "$OUT/checkpoints"/*.flax_model; do
  [ -e "$kept" ] || { echo "no checkpoint in $OUT/checkpoints" >&2; exit 1; }
  play "$(basename "$kept" .flax_model)" "$kept"
done
play "released-$CHECKPOINT" "$AGENT/scripts/checkpoints/$CHECKPOINT.flax_model"

if [ "$MIRROR" != 0 ]; then
  echo "3. Each checkpoint against the released model, $MIRROR games"
  for kept in "$OUT/checkpoints"/*.flax_model; do
    name=$(basename "$kept" .flax_model)
    result="$OUT/mirror/$name.txt"
    [ -e "$result" ] && continue
    device=()
    [ "$DEVICE" = cpu ] && device=(--xla_device cpu)
    echo "games=$MIRROR" > "$result.part"
    (cd "$AGENT/scripts" && "$VENV/bin/python" -u battle.py "${device[@]}" \
      --num_episodes "$MIRROR" --deck ../assets/deck/BlueEyes.ydk --seed 0 \
      --checkpoint1 "$kept" --checkpoint2 "checkpoints/$CHECKPOINT.flax_model") \
      >> "$result.part" 2> "$OUT/mirror/$name.log"
    grep -q 'win_rate=' "$result.part" \
      || { echo "battle.py gave no result: see $OUT/mirror/$name.log" >&2; exit 1; }
    mv "$result.part" "$result"
    grep 'win_rate=' "$result" | tail -1 | sed "s/^/   $name: /"
  done
fi

report=report.txt
[ "$ROWS" = pilot ] || report="report-$ROWS.txt"
python3 "$KIT/curve.py" "$OUT" --rows "$ROWS" | tee "$OUT/$report"
echo "The checkpoints, the rows and this report ($report) are in $OUT."
