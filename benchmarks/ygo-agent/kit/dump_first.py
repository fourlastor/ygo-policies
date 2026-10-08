"""Dump what ygo-agent's own environment shows the model at the first decisions of a game.

Run from ygo-agent's `scripts/` directory.  Writes the observation arrays of
each decision of the watched seat, with the model's probabilities and value,
to an .npz file, and prints the environment's own log of the game beside it.
"""

import os
import sys

os.environ.setdefault("JAX_PLATFORMS", "cpu")

import numpy as np

import ygoenv
from ygoai.utils import init_ygopro

seed, decisions, out = int(sys.argv[1]), int(sys.argv[2]), sys.argv[3]
checkpoint = sys.argv[4] if len(sys.argv) > 4 else None

deck = init_ygopro("YGOPro-v1", "english", "../assets/deck/BlueEyes.ydk", "code_list.txt")
envs = ygoenv.make(
    task_id="YGOPro-v1",
    env_type="gymnasium",
    num_envs=1,
    num_threads=1,
    seed=seed,
    deck1=deck,
    deck2=deck,
    player=0,
    max_options=24,
    n_history_actions=32,
    play_mode="bot",
    async_reset=False,
    verbose=True,
    record=False,
)
obs_space = envs.observation_space
obs, infos = envs.reset()

predict = None
if checkpoint:
    import flax
    import jax
    import jax.numpy as jnp
    from ygoai.rl.jax.agent import ModelArgs, RNNAgent
    from dataclasses import asdict

    agent = RNNAgent(**asdict(ModelArgs()), embedding_shape=None)
    sample = jax.tree.map(lambda x: jnp.array([x]), obs_space.sample())
    rstate = agent.init_rnn_state(1)
    params = jax.jit(agent.init)(jax.random.PRNGKey(0), sample, rstate)
    with open(checkpoint, "rb") as f:
        params = flax.serialization.from_bytes(params, f.read())

    @jax.jit
    def predict(params, rstate, obs):
        next_rstate, logits, value = agent.apply(params, obs, rstate)[:3]
        return next_rstate, jax.nn.softmax(logits, axis=-1), value

saved = {}
for step in range(decisions):
    record = {key: np.array(value[0]) for key, value in obs.items()}
    record["num_options"] = np.array(infos["num_options"][0])
    action = 0
    if predict is not None:
        # As training and battle.py do: the model is not given the mask.
        given = {key: (None if key == "mask_" else value) for key, value in obs.items()}
        rstate, probs, value = predict(params, rstate, given)
        record["probs"] = np.array(probs[0])
        record["value"] = np.array(value[0])
        action = int(np.array(probs[0]).argmax())
        print(f"## decision {step}: probs {np.round(np.array(probs[0])[: int(infos['num_options'][0])], 4).tolist()} value {float(np.array(value).reshape(-1)[0]):+.4f} -> {action}")
    for key, value in record.items():
        saved[f"{step}/{key}"] = value
    obs, rewards, terminated, truncated, infos = envs.step(np.array([action]))
    if terminated[0] or truncated[0]:
        break
np.savez(out, **saved)
print(f"## saved {len(saved)} arrays to {out}")
