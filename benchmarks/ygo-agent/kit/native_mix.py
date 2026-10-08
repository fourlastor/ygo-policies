"""What a ygo-agent model is asked in its own environment, playing the
Blue-Eyes deck against itself: how many decisions a game, of which kind,
with how many options, and how many of them after a reveal.

    cd /tmp/ygo-agent/scripts
    /tmp/ygo-agent-venv/bin/python /path/to/kit/native_mix.py \\
        checkpoints/0546_22750M.flax_model 256 native-mix.json

The same tally of the requests a client sends `serve.py` from another engine
should look like this one.  Decisions with one option are not put to the
model there and are not counted here.
"""

from __future__ import annotations

import json
import sys
from collections import Counter
from dataclasses import asdict

import flax
import jax
import jax.numpy as jnp
import numpy as np
import ygoenv
from ygoai.rl.jax.agent import ModelArgs, RNNAgent
from ygoai.utils import init_ygopro

checkpoint, games, out = sys.argv[1], int(sys.argv[2]), sys.argv[3]
seed = int(sys.argv[4]) if len(sys.argv) > 4 else 0
NAMES = {
    1: "select_idlecmd", 2: "select_chain", 3: "select_card", 4: "select_tribute", 5: "select_position",
    6: "select_effectyn", 7: "select_yesno", 8: "select_battlecmd", 9: "select_unselect_card",
    10: "select_option", 11: "select_place", 12: "select_sum", 13: "select_disfield",
    14: "announce_attrib", 15: "announce_number",
}  # fmt: skip

deck = init_ygopro("YGOPro-v1", "english", "../assets/deck/BlueEyes.ydk", "code_list.txt")
envs = ygoenv.make(
    task_id="YGOPro-v1",
    env_type="gymnasium",
    num_envs=games,
    num_threads=min(games, 8),
    seed=seed,
    deck1=deck,
    deck2=deck,
    player=-1,
    max_options=24,
    n_history_actions=32,
    play_mode="self",
    async_reset=False,
    verbose=False,
    record=False,
)
agent = RNNAgent(**asdict(ModelArgs()), embedding_shape=None)
sample = jax.tree.map(lambda x: jnp.array([x]), envs.observation_space.sample())
params = jax.jit(agent.init)(jax.random.PRNGKey(0), sample, agent.init_rnn_state(1))
with open(checkpoint, "rb") as handle:
    params = flax.serialization.from_bytes(params, handle.read())


@jax.jit
def both(params, first, second, obs, to_move):
    next_first, logits_first, _ = agent.apply(params, obs, first)[:3]
    next_second, logits_second, _ = agent.apply(params, obs, second)[:3]
    mine = (to_move == 0)[:, None]
    logits = jnp.where(mine, logits_first, logits_second)
    first = jax.tree.map(lambda new, old: jnp.where(mine, new, old), next_first, first)
    second = jax.tree.map(lambda new, old: jnp.where(mine, old, new), next_second, second)
    return first, second, logits.argmax(axis=-1)


obs, infos = envs.reset()
to_move = infos["to_play"]
first, second = agent.init_rnn_state(games), agent.init_rnn_state(games)
live = np.ones(games, dtype=bool)
kinds, options, taken = Counter(), Counter(), Counter()
decisions = after_reveal = 0
while live.any():
    given = {key: (None if key == "mask_" else value) for key, value in obs.items()}
    first, second, actions = both(params, first, second, given, jnp.array(to_move))
    actions = np.array(actions)
    rows = obs["actions_"]
    count = rows.any(axis=-1).sum(axis=-1)
    cards = obs["cards_"]
    piles = (cards[:, :, 4] == 1) & np.isin(cards[:, :, 2], (1, 2, 7))
    window = (piles & (cards[:, :, 5] != 0)).any(axis=-1)
    for game in np.flatnonzero(live):
        kind = NAMES.get(int(rows[game, 0, 3]), str(int(rows[game, 0, 3])))
        kinds[kind] += 1
        options[kind] += int(count[game])
        # Of a chance to respond: how often the model passes (act 9 is "cancel").
        if kind == "select_chain":
            taken["pass" if rows[game, actions[game], 4] == 9 else "activate"] += 1
        decisions += 1
        after_reveal += bool(window[game])
    obs, rewards, terminated, truncated, infos = envs.step(actions)
    done = np.logical_or(terminated, truncated)
    live &= ~done
    reset = jnp.array(done)[:, None]
    first, second = jax.tree.map(lambda x: jnp.where(reset, 0, x), (first, second))
    to_move = infos["to_play"]

report = {
    "checkpoint": checkpoint,
    "games": games,
    "decisions_per_game": decisions / games,
    "after_reveal": after_reveal / decisions,
    "kinds": {kind: {"share": n / decisions, "per_game": n / games, "options": options[kind] / n} for kind, n in kinds.most_common()},
    "chain": dict(taken),
}
with open(out, "w") as handle:
    json.dump(report, handle, indent=1)
print(f"{games} games: {report['decisions_per_game']:.1f} decisions a game put to the model, {report['after_reveal']:.1%} after a reveal")
for kind, row in report["kinds"].items():
    print(f"  {kind:22} {row['share']:6.1%}  {row['per_game']:6.1f} a game  {row['options']:5.1f} options")
print("  at a chance to respond:", dict(taken))
