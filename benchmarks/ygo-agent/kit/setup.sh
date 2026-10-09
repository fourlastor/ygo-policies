#!/usr/bin/env bash
# ygo-agent as its released checkpoints need it, built from source.
#
#   bash benchmarks/ygo-agent/kit/setup.sh
#
# Clones https://github.com/sbl1996/ygo-agent at the last commit its released
# checkpoints work with, builds its environment (ygopro-core, Lua 5.4.6 and
# the C++ libraries, through xmake), fetches the card database and the card
# scripts that commit pins, the four checkpoints of its release v0.1, and the
# Python packages of mid-2024 into a virtual environment of its own.  Then it
# checks the result: a few games, and the golden test of `serve.py`.
#
# Everything goes under AGENT, BUILD, VENV and SCRIPTS (by default in /tmp,
# about 1.5 GB); nothing is installed into the system.  It needs git, curl,
# a C++ compiler, CMake and uv.  A step already done is not done again.
#
#   GPU=1 bash benchmarks/ygo-agent/kit/setup.sh
#
# also installs JAX's build for CUDA 12 with the CUDA libraries it needs
# (4 GB more; an NVIDIA driver is all the machine must have), for the server
# to answer many requests at once on the GPU.  The checks at the end still
# run on the CPU.
#
# Why this commit: the latest one (dbf5142, 2024-08-16) numbers 13,472 cards
# where the checkpoints know 864, and a model there sees other cards than
# the ones on the table.  26293f8 (2024-07-30) is the last with the list the
# checkpoints were trained on, and the one their README's own figures are of.
set -euo pipefail
KIT=$(cd "$(dirname "$0")" && pwd)
# The steps below work from several folders: a path given from where this
# script is started is made whole first.
absolute() {
  case $1 in
    /*) printf '%s\n' "$1" ;;
    *) printf '%s/%s\n' "$PWD" "$1" ;;
  esac
}
AGENT=$(absolute "${AGENT:-/tmp/ygo-agent}")
BUILD=$(absolute "${BUILD:-/tmp/ygo-agent-build}")
VENV=$(absolute "${VENV:-/tmp/ygo-agent-venv}")
SCRIPTS=$(absolute "${SCRIPTS:-/tmp/ygopro-scripts}")
COMMIT=26293f82e1e53aabb09b2afd8bb7ccb5b9d91973
SCRIPTS_COMMIT=44eff41fa27fb15defb95f9e98d9d5c596ed1433
DATABASE=https://github.com/mycard/ygopro-database/raw/f288cd7d353467eb3816babcbb04620e11f64e8a/locales
RELEASE=https://github.com/sbl1996/ygo-agent/releases/download/v0.1
XMAKE=https://github.com/xmake-io/xmake/releases/download/v2.9.9/xmake-bundle-v2.9.9.linux.x86_64

# 1. The source, at the commit, with what this machine's compiler needs:
#    Lua 5.4.6 built for the engine where the system has 5.1, an include GCC
#    15 no longer adds by itself, and six constructor calls it finds
#    ambiguous.  Only the environment the checkpoints use is built.
if [ ! -d "$AGENT/.git" ]; then
  git clone https://github.com/sbl1996/ygo-agent.git "$AGENT"
fi
if [ "$(git -C "$AGENT" rev-parse HEAD)" != "$COMMIT" ]; then
  git -C "$AGENT" checkout -q "$COMMIT"
fi
if git -C "$AGENT" apply --check "$KIT/build-26293f8.diff" 2> /dev/null; then
  git -C "$AGENT" apply "$KIT/build-26293f8.diff"
elif ! git -C "$AGENT" apply --reverse --check "$KIT/build-26293f8.diff" 2> /dev/null; then
  echo "$AGENT holds other changes than build-26293f8.diff" >&2
  exit 1
fi

# 2. xmake, as one file, and the Python environment.
mkdir -p "$BUILD/bin"
if [ ! -x "$BUILD/bin/xmake" ]; then
  curl -fsSL -o "$BUILD/bin/xmake" "$XMAKE"
  chmod +x "$BUILD/bin/xmake"
fi
if [ ! -x "$VENV/bin/python" ]; then
  uv venv --python 3.11 "$VENV"
fi
# JAX is the last version their README allows; the others are of its time.
# A later tyro takes the `scripts/torch` folder for PyTorch and fails.
JAX=("jax==0.4.28" "jaxlib==0.4.28")
if [ -n "${GPU:-}" ]; then
  JAX+=("jax[cuda12]==0.4.28")
fi
(cd "$AGENT" && uv pip install --python "$VENV/bin/python" \
  "${JAX[@]}" "flax==0.8.5" "optax==0.2.2" "chex==0.1.86" \
  "scipy==1.13.1" "orbax-checkpoint==0.5.20" "numpy==1.26.4" "distrax==0.1.5" \
  "tyro==0.8.5" -e ygoenv -e ygoinf -e .)

# 3. The environment.
export XMAKE_GLOBALDIR=$BUILD XMAKE_COLORTERM=nocolor
export PATH=$VENV/bin:$BUILD/bin:$PATH
if [ ! -f "$AGENT/ygoenv/ygoenv/ygopro/ygopro_ygoenv.so" ]; then
  (cd "$AGENT" && xmake f -y -m release && xmake b ygopro_ygoenv)
fi

# 4. The cards and their scripts, as that commit's Makefile pins them.
for locale in en:en-US zh:zh-CN; do
  short=${locale%%:*}
  mkdir -p "$AGENT/assets/locale/$short"
  for file in cards.cdb strings.conf; do
    if [ ! -f "$AGENT/assets/locale/$short/$file" ]; then
      curl -fsSL -o "$AGENT/assets/locale/$short/$file" "$DATABASE/${locale##*:}/$file"
    fi
  done
done
if [ ! -d "$SCRIPTS/.git" ]; then
  git clone --filter=blob:none --no-checkout https://github.com/mycard/ygopro-scripts.git "$SCRIPTS"
fi
if [ "$(git -C "$SCRIPTS" rev-parse HEAD 2> /dev/null)" != "$SCRIPTS_COMMIT" ]; then
  git -C "$SCRIPTS" checkout -q "$SCRIPTS_COMMIT"
fi
ln -sfn "$SCRIPTS" "$AGENT/scripts/script"

# 5. The checkpoints of release v0.1: 11.3, 16.5 and 22.75 billion steps,
#    the last being the one their README says had over 100M games, and the
#    one their server is deployed with.
mkdir -p "$AGENT/scripts/checkpoints"
for name in 0546_11300M.flax_model 0546_16500M.flax_model 0546_22750M.flax_model 0546_26550M.tflite; do
  if [ ! -f "$AGENT/scripts/checkpoints/$name" ]; then
    curl -fsSL -o "$AGENT/scripts/checkpoints/$name" "$RELEASE/$name"
  fi
done

# 6. Their eval.py gives the model a mask of what is visible, which training
#    and their battle.py take out: with it a trained model loses to random
#    play.  eval_masked.py is eval.py with the one line battle.py has.
python - "$AGENT/scripts" << 'PY'
import sys
from pathlib import Path

scripts = Path(sys.argv[1])
source = (scripts / "eval.py").read_text()
old_import = "from ygoai.rl.utils import RecordEpisodeStatistics"
old_wrap = "    envs = RecordEpisodeStatistics(envs)\n"
assert source.count(old_import) == 1 and source.count(old_wrap) == 1
(scripts / "eval_masked.py").write_text(
    source.replace(old_import, old_import + ", EnvPreprocess").replace(
        old_wrap,
        "    # As in battle.py and in training: the model is not given the visibility mask.\n"
        "    envs = EnvPreprocess(envs, skip_mask=True)\n" + old_wrap,
    )
)
PY
cp "$KIT/dump_first.py" "$AGENT/scripts/dump_first.py"

# 7. Does it work.  Their scripts read code_list.txt and script/ from the
#    working directory: everything of theirs is run from scripts/.
cd "$AGENT/scripts"
export JAX_PLATFORMS=${JAX_PLATFORMS:-cpu}
echo "The 22.75-billion-step model against their first-option player, 64 games:"
python -u eval_masked.py --deck ../assets/deck/BlueEyes.ydk \
  --checkpoint checkpoints/0546_22750M.flax_model --num_episodes 64 --num_envs 32 \
  --bot_type greedy --xla_device cpu --seed 0 2> /dev/null | grep '^len='
python -u "$KIT/golden_check.py" checkpoints/0546_22750M.flax_model 2> /dev/null | tail -1
echo "ygo-agent is ready in $AGENT; its Python is $VENV/bin/python."
