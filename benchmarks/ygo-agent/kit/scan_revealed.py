"""What ygo-agent's own environment shows a seat of the opponent's hidden
piles (Deck, hand, Extra Deck) and face-down cards: is a card the opponent
revealed ever shown, and what changes after a reveal.

    cd /tmp/ygo-agent/scripts
    /tmp/ygo-agent-venv/bin/python /path/to/kit/scan_revealed.py 64 4000

Random play of the Blue-Eyes deck against itself, both seats.
"""

from __future__ import annotations

import sys

import numpy as np
import ygoenv
from ygoai.utils import init_ygopro

games, steps = int(sys.argv[1]), int(sys.argv[2])
seed = int(sys.argv[3]) if len(sys.argv) > 3 else 0
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
rng = np.random.default_rng(seed)
obs, infos = envs.reset()
DECK, HAND, MZONE, SZONE, EXTRA = 1, 2, 3, 4, 7
decisions = 0
pile_rows = 0  # rows of the opponent's Deck, hand and Extra Deck
pile_position = {}  # their position id -> rows
pile_shown = 0  # of them, rows with any statistic or type
windows = 0  # decisions where such rows carry a position
shown_decisions = 0
facedown_field = 0
facedown_field_shown = 0
for _ in range(steps):
    cards = obs["cards_"]
    theirs = cards[:, :, 4] == 1
    pile = theirs & np.isin(cards[:, :, 2], (DECK, HAND, EXTRA))
    shown = pile & (cards[:, :, 7:].any(axis=-1) | cards[:, :, 0:2].any(axis=-1))
    decisions += games
    pile_rows += int(pile.sum())
    for position, count in zip(*np.unique(cards[:, :, 5][pile], return_counts=True)):
        pile_position[int(position)] = pile_position.get(int(position), 0) + int(count)
    pile_shown += int(shown.sum())
    windows += int((pile & (cards[:, :, 5] != 0)).any(axis=-1).sum())
    shown_decisions += int(shown.any(axis=-1).sum())
    down = theirs & np.isin(cards[:, :, 2], (MZONE, SZONE)) & np.isin(cards[:, :, 5], (2, 6, 7)) & (cards[:, :, 6] == 0)
    facedown_field += int(down.sum())
    facedown_field_shown += int((down & cards[:, :, 7:].any(axis=-1)).sum())
    options = obs["actions_"].any(axis=-1).sum(axis=-1)
    actions = (rng.random(games) * np.maximum(options, 1)).astype(np.int32)
    obs, rewards, terminated, truncated, infos = envs.step(actions)

print(f"{decisions} decisions of random play")
print(f"rows of the opponent's Deck, hand and Extra Deck: {pile_rows}, by position id {pile_position}")
print(f"  of them shown with a statistic, a type or an identity: {pile_shown}, at {shown_decisions} decisions")
print(f"decisions where those rows carry a position (after a reveal, until the chain is solved): {windows}")
print(f"the opponent's face-down cards on the field: {facedown_field} rows, shown: {facedown_field_shown}")
