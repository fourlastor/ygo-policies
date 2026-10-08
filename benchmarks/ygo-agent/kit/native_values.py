"""How right a ygo-agent model's own estimate is, in its own environment.

    cd /tmp/ygo-agent/scripts
    /tmp/ygo-agent-venv/bin/python /path/to/kit/native_values.py \\
        checkpoints/0546_22750M.flax_model 512 native-values.json

The model plays the Blue-Eyes deck against itself, its first choice at every
decision.  At each decision the value it gives the seat to move is kept, as
the `win_rate` their server makes of it ((value + 1) / 2, held to 0..1), and
set against whether that seat won the game: by band of that estimate, how
many decisions and how often the seat won.  The same table from games played
through `serve.py` in another engine should look like this one.
"""

from __future__ import annotations

import json
import os
import sys
from dataclasses import asdict

os.environ.setdefault("JAX_PLATFORMS", "cpu")

import flax  # noqa: E402
import jax  # noqa: E402
import jax.numpy as jnp  # noqa: E402
import numpy as np  # noqa: E402
import ygoenv  # noqa: E402
from ygoai.rl.jax.agent import ModelArgs, RNNAgent  # noqa: E402
from ygoai.utils import init_ygopro  # noqa: E402

checkpoint, games, out = sys.argv[1], int(sys.argv[2]), sys.argv[3]
seed = int(sys.argv[4]) if len(sys.argv) > 4 else 0

deck = init_ygopro("YGOPro-v1", "english", "../assets/deck/BlueEyes.ydk", "code_list.txt")
envs = ygoenv.make(
    task_id="YGOPro-v1",
    env_type="gymnasium",
    num_envs=games,
    num_threads=min(games, 16),
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
def both(params, first, second, obs, to_move, done):
    """Each seat keeps its own memory of the game; only the seat to move
    advances it."""
    next_first, logits_first, value_first = agent.apply(params, obs, first)[:3]
    next_second, logits_second, value_second = agent.apply(params, obs, second)[:3]
    mine = (to_move == 0)[:, None]
    logits = jnp.where(mine, logits_first, logits_second)
    value = jnp.where(mine[:, 0], value_first.reshape(-1), value_second.reshape(-1))
    first = jax.tree.map(lambda new, old: jnp.where(mine, new, old), next_first, first)
    second = jax.tree.map(lambda new, old: jnp.where(mine, old, new), next_second, second)
    return first, second, logits.argmax(axis=-1), value


obs, infos = envs.reset()
to_move = infos["to_play"]
first, second = agent.init_rnn_state(games), agent.init_rnn_state(games)
kept: list[list[tuple[int, float]]] = [[] for _ in range(games)]
winners: list[int | None] = [None] * games
lengths = [0] * games
while any(winner is None for winner in winners):
    # As training and battle.py do: the model is not given the mask.
    given = {key: (None if key == "mask_" else value) for key, value in obs.items()}
    first, second, actions, values = both(params, first, second, given, jnp.array(to_move), None)
    values = np.array(values)
    for game in range(games):
        if winners[game] is None:
            kept[game].append((int(to_move[game]), float(values[game])))
            lengths[game] += 1
    moved = to_move
    obs, rewards, terminated, truncated, infos = envs.step(np.array(actions))
    done = np.logical_or(terminated, truncated)
    for game in np.flatnonzero(done):
        if winners[game] is None:
            # The last reward is the seat's that moved last.
            won = rewards[game] > 0
            winners[game] = -1 if rewards[game] == 0 else int(moved[game] if won else 1 - moved[game])
    reset = jnp.array(done)[:, None]
    first, second = jax.tree.map(lambda x: jnp.where(reset, 0, x), (first, second))
    to_move = infos["to_play"]

bands = [[0, 0] for _ in range(10)]
for game, decisions in enumerate(kept):
    if winners[game] in (0, 1):
        for seat, value in decisions:
            estimate = min(max((value + 1) / 2, 0.0), 1.0)
            band = bands[min(int(estimate * 10), 9)]
            band[0] += 1
            band[1] += seat == winners[game]
decided = [winner for winner in winners if winner in (0, 1)]
report = {
    "checkpoint": checkpoint,
    "games": games,
    "undecided": games - len(decided),
    "second_seat_wins": sum(decided) / len(decided),
    "decisions_per_game": sum(lengths) / games,
    "bands": [
        {"from": low / 10, "to": (low + 1) / 10, "decisions": count, "won": won / count if count else None}
        for low, (count, won) in enumerate(bands)
    ],
}
with open(out, "w") as handle:
    json.dump(report, handle, indent=1)
print(f"{games} games of {checkpoint} against itself: {report['decisions_per_game']:.0f} decisions a game, "
      f"the seat that goes second wins {report['second_seat_wins']:.1%}")
print("the model's estimate for the seat to move, and how often that seat won:")
for band in report["bands"]:
    won = "-" if band["won"] is None else f"{band['won']:.1%}"
    print(f"  {band['from']:.1f} to {band['to']:.1f}: {band['decisions']:>7} decisions, won {won}")
