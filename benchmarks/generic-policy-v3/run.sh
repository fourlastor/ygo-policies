#!/bin/bash
# Reproduce a run of this folder with the repository's build.
#   build:  cargo build --release -p ygo-policies-ffi -p ygo-policies-bench
#   frozen specialists: build main at 7ddad06 (`git archive 7ddad06`) the same way and pass
#   its libygo_policies.so as BASELINE; it reproduces astra's frozen baseline game for game.
# Usage: BASELINE=/path/to/7ddad06/libygo_policies.so run.sh ARM OUTPUT [DECKS] [SEED]
#   ARM: plain | mcts32 | staged (the measured defaults) | staged-strict (no chains, no Set monsters)
# The 16-deck schedule is the default; DECKS=all with SEED=2700000 is the roster.
set -eu
R=$(cd "$(dirname "$0")/../.." && pwd)
ARM=$1; OUT=$2
DECKS=${3:-deckout,exodia,chain-burn,psychic,burn,crystal,draconic-might,lightsworn,monarch,pyramid,rock-block,volcanic,demise,gishki,heroes,blackwing}
SEED=${4:-2600000}
B=(taskset -c 8-23 "$R/target/release/policy-bench" generic --baseline "${BASELINE:?frozen specialist library}"
   --library "$R/target/release/libygo_policies.so" --cards "$R/vendor/BabelCdb/cards.cdb" --decks "$R/decks"
   --scripts "$R/vendor/CardScripts" --effects true --limit 8192 --policies "$DECKS" --seed "$SEED"
   --games 32 --workers 16 --output "$OUT")
case "$ARM" in
plain) "${B[@]}" --plain true ;;
mcts32) "${B[@]}" --method mcts --depth 1 --horizon 1 --millis 0 --worlds 4 --max-candidates 8 --max-playouts 256 \
  --mcts-actions 8 --mcts-decisions 384 --mcts-iterations 32 --mcts-nodes 1024 --mcts-replies 1 --mcts-terminal false \
  --mcts-validate-roots false --mcts-worlds 4 --mcts-adversarial-opponent false --mcts-root-warmup false --opponent-model generic ;;
staged) "${B[@]}" --method staged --opponent-model generic ;;
staged-strict) "${B[@]}" --method staged --search-chains false --resample-set-monsters false --opponent-model generic ;;
*) echo "unknown arm $ARM" >&2; exit 2 ;;
esac
