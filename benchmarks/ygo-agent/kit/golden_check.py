"""The first three decisions of one game of ygo-agent's own environment, put
to `serve.py` as a client would put them, and compared with what the
environment showed the model and what the model answered there.

    cd /tmp/ygo-agent/scripts
    /tmp/ygo-agent-venv/bin/python /path/to/kit/golden_check.py \\
        checkpoints/0546_22750M.flax_model

The game is the Blue-Eyes mirror of seed 7 as `dump_first.py` plays it
(`golden.npz` holds what it dumped): the seat that goes first, after the
opponent's Maxx "C" in its Draw Phase.

1. Main Phase 1, hand of Blue-Eyes White Dragon, Trade-In, Return of the
   Dragon Lords, Trade-In, The Melody of Awakening Dragon.  The model
   activates the first Trade-In.
2. Where to put it: the model takes the second Spell & Trap Zone.
3. What to discard for it: only Blue-Eyes White Dragon can be.  A single
   option is not put to the model, and still belongs to what it did.
4. Main Phase 1 again, with the two cards drawn.

It writes the four requests and the answers to them beside itself
(`golden-requests.json`, `golden-answers.json`): the reference for a client.
Exit status 0 when everything agrees.
"""

from __future__ import annotations

import json
import os
import sqlite3
import sys
from pathlib import Path

os.environ.setdefault("JAX_PLATFORMS", "cpu")
HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import numpy as np  # noqa: E402
from ygoinf import features  # noqa: E402

import serve  # noqa: E402

BLUE_EYES, TRADE_IN, RETURN, MELODY, MAXX_C = 89631139, 38120068, 6853254, 48800175, 23434538

ATTRIBUTES = {0x01: "earth", 0x02: "water", 0x04: "fire", 0x08: "wind", 0x10: "light", 0x20: "dark", 0x40: "divine"}
RACES = [
    "warrior", "spellcaster", "fairy", "fiend", "zombie", "machine", "aqua", "pyro", "rock",
    "windbeast", "plant", "insect", "thunder", "dragon", "beast", "beast_warrior", "dinosaur",
    "fish", "sea_serpent", "reptile", "psycho", "devine", "creator_god", "wyrm", "cyberse", "illusion",
]  # fmt: skip
TYPES = {
    0x1: "monster", 0x2: "spell", 0x4: "trap", 0x10: "normal", 0x20: "effect", 0x40: "fusion",
    0x80: "ritual", 0x100: "trap_monster", 0x200: "spirit", 0x400: "union", 0x800: "dual",
    0x1000: "tuner", 0x2000: "synchro", 0x4000: "token", 0x10000: "quick_play",
    0x20000: "continuous", 0x40000: "equip", 0x80000: "field", 0x100000: "counter",
    0x200000: "flip", 0x400000: "toon", 0x800000: "xyz", 0x1000000: "pendulum",
    0x2000000: "special", 0x4000000: "link",
}  # fmt: skip


def printed(database: sqlite3.Connection, code: int) -> dict:
    """A card's printed data, in the words of the server's schema."""
    kind, attack, defense, level, race, attribute = database.execute(
        "select type, atk, def, level, race, attribute from datas where id = ?", (code,)
    ).fetchone()
    monster = bool(kind & 0x1)
    return {
        "types": [name for bit, name in TYPES.items() if kind & bit],
        "attribute": ATTRIBUTES.get(attribute, "none") if monster else "none",
        "race": next((RACES[i] for i in range(len(RACES)) if race == 1 << i), "none") if monster else "none",
        "level": level & 0xFF if monster else 0,
        "attack": max(attack, 0) if monster else 0,
        "defense": max(defense, 0) if monster else 0,
    }


def requests(deck_file: Path, database: sqlite3.Connection) -> list[dict]:
    deck = [int(line) for line in deck_file.read_text().splitlines() if line.strip().isdigit()]
    main, extra = deck[:40], deck[40:54]

    def card(code: int | None, location: str, sequence: int, controller: str, position: str) -> dict:
        """A card as the schema wants it.  A card the seat cannot see has code
        0, no position and nothing else."""
        entry = {
            "code": code or 0,
            "location": location,
            "sequence": sequence,
            "controller": controller,
            "position": position,
            "overlay_sequence": -1,
            "attribute": "none",
            "race": "none",
            "level": 0,
            "counter": 0,
            "negated": False,
            "attack": 0,
            "defense": 0,
            "types": [],
        }
        if code:
            entry.update(printed(database, code))
        return entry

    def table(hand: list[int], drawn: list[int], field: list[tuple[int, int]], grave: list[int]) -> list[dict]:
        """Every card of both players, the seat's own first: Deck, hand,
        Monster Zones, Spell & Trap Zones, Graveyard, banished, Extra Deck."""
        rest = list(main)
        for code in [*hand, *drawn, *(code for code, _ in field), *grave]:
            if code in rest:
                rest.remove(code)
        cards = [card(code, "deck", 0, "me", "facedown") for code in rest]
        cards += [card(code, "hand", place, "me", "facedown") for place, code in enumerate(hand)]
        cards += [card(code, "szone", zone, "me", "faceup") for code, zone in field]
        cards += [card(code, "grave", place, "me", "faceup") for place, code in enumerate(grave)]
        cards += [card(code, "extra", 0, "me", "facedown") for code in extra]
        cards += [card(None, "deck", 0, "opponent", "none") for _ in range(35)]
        cards += [card(None, "hand", place, "opponent", "none") for place in range(4)]
        cards += [card(MAXX_C, "grave", 0, "opponent", "faceup")]
        cards += [card(None, "extra", 0, "opponent", "none") for _ in range(14)]
        return cards

    def world(cards: list[dict]) -> dict:
        return {
            "global": {"my_lp": 8000, "op_lp": 8000, "turn": 1, "phase": "main1", "is_first": True, "is_my_turn": True},
            "cards": cards,
        }

    def held(hand: list[int], place: int) -> dict:
        return {"code": hand[place], "controller": "me", "location": "hand", "sequence": place}

    def idle(hand: list[int], sets: list[int], activates: list[int]) -> dict:
        commands = [
            {"cmd_type": "set", "data": {"card_info": held(hand, place), "effect_description": 0, "response": place}}
            for place in sets
        ]
        commands += [
            {"cmd_type": "activate", "data": {"card_info": held(hand, place), "effect_description": 0, "response": place}}
            for place in activates
        ]
        # The first turn has no Battle Phase: "End Phase" is offered.
        commands += [{"cmd_type": "to_ep"}]
        return {"data": {"msg_type": "select_idlecmd", "idle_cmds": commands}}

    first = [BLUE_EYES, TRADE_IN, RETURN, TRADE_IN, MELODY]
    paying = [BLUE_EYES, RETURN, TRADE_IN, MELODY]
    after = [RETURN, TRADE_IN, MELODY, BLUE_EYES, TRADE_IN]
    return [
        # 1. The model answers 4: activate the Trade-In at hand place 2.
        {
            "input": {**world(table(first, [], [], [])), "action_msg": idle(first, [1, 2, 3, 4], [1, 3, 4])},
            "prev_action_idx": 0,
            "index": 0,
        },
        # 2. Which zone.  The model answers 1: the second.
        {
            "input": {
                **world(table(first, [], [], [])),
                "action_msg": {
                    "data": {
                        "msg_type": "select_place",
                        "count": 1,
                        "places": [{"controller": "me", "location": "szone", "sequence": zone} for zone in range(5)],
                    }
                },
            },
            "prev_action_idx": 4,
            "index": 1,
        },
        # 3. The cost: one card, one candidate.
        {
            "input": {
                **world(table(paying, [], [(TRADE_IN, 1)], [])),
                "action_msg": {
                    "data": {
                        "msg_type": "select_card",
                        "cancelable": False,
                        "min": 1,
                        "max": 1,
                        "cards": [
                            {
                                "location": {"controller": "me", "location": "hand", "sequence": 0, "overlay_sequence": -1},
                                "response": 0,
                            }
                        ],
                        "selected": [],
                    }
                },
            },
            "prev_action_idx": 1,
            "index": 2,
        },
        # 4. Blue-Eyes White Dragon and Trade-In are in the Graveyard, and the
        #    two cards drawn are at the end of the hand.
        {
            "input": {
                **world(table(after, [], [], [BLUE_EYES, TRADE_IN])),
                "action_msg": idle(after, [0, 1, 2, 4], [0, 1, 2, 4]),
            },
            "prev_action_idx": 0,
            "index": 3,
        },
    ]


def rows(array: np.ndarray) -> list[tuple[int, ...]]:
    return sorted(tuple(int(value) for value in row) for row in array)


def main() -> None:
    checkpoint = sys.argv[1]
    database = sqlite3.connect("../assets/locale/en/cards.cdb")
    asked = requests(Path("../assets/deck/BlueEyes.ydk"), database)
    features.init_code_list("code_list.txt")
    model = features.Predictor.load(checkpoint, 1)
    theirs = np.load(HERE / "golden.npz")
    failures = 0

    def check(what: str, good: bool) -> None:
        nonlocal failures
        failures += not good
        print(f"  {'ok  ' if good else 'FAIL'} {what}")

    answers = []
    for name, card_ids in (("as the environment shows the cards (no identities)", False), ("with identities, as their server sends them", True)):
        print(name)
        duel, shown = serve.Duel(), []
        got = [
            serve.predict(model, duel, json.loads(json.dumps(request)), card_ids=card_ids, history_cancel=False, shown_to=shown)
            for request in asked
        ]
        # The third request has one option: it is the environment's decisions 0, 1 and 2.
        for decision, place in enumerate((0, 1, 3)):
            mine = shown[place]
            probs = [entry["prob"] for entry in got[place]["predict_results"]["action_preds"]]
            wanted = theirs[f"{decision}/probs"][: len(probs)]
            value = 2 * got[place]["predict_results"]["win_rate"] - 1
            close = np.allclose(probs, wanted, atol=2e-4) and abs(value - float(theirs[f"{decision}/value"].reshape(-1)[0])) < 2e-3
            if not card_ids:
                for key in ("global_", "actions_", "h_actions_"):
                    check(f"decision {decision}: {key} as the environment's", np.array_equal(mine[key], theirs[f"{decision}/{key}"]))
                check(f"decision {decision}: the cards as the environment's", rows(mine["cards_"]) == rows(theirs[f"{decision}/cards_"]))
                check(f"decision {decision}: the model's answer as in the environment {np.round(probs, 4).tolist()}", close)
            else:
                print(f"       decision {decision}: {np.round(probs, 4).tolist()} value {value:+.4f} ({'the same' if close else 'another'} answer)")
        if not card_ids:
            answers = got
            single = got[2]["predict_results"]
            check(
                "the single option is answered without the model: the card, and no \"no more\"",
                [entry["prob"] for entry in single["action_preds"]] == [1.0, -1.0] and single["win_rate"] == -1.0,
            )
    (HERE / "golden-requests.json").write_text(json.dumps(asked, indent=1) + "\n", encoding="utf8")
    (HERE / "golden-answers.json").write_text(json.dumps(answers, indent=1) + "\n", encoding="utf8")
    print("everything agrees" if not failures else f"{failures} checks failed")
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
