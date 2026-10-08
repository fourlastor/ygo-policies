"""A prediction server for ygo-agent's released model that shows the model
what its own environment showed it in training.

    cd /tmp/ygo-agent/scripts
    /tmp/ygo-agent-venv/bin/python /path/to/serve.py \\
        --checkpoint checkpoints/0546_22750M.flax_model --port 3013

It speaks the protocol of ygo-agent's own server (`ygoinf/ygoinf/server.py`
at commit 26293f8) and builds the model's input with that server's own code
(`ygoinf.features`), so a client written against it also works against
theirs.  It differs where their server differs from the environment the model
was trained in (`ygoenv/ygoenv/ygopro/ygopro.h`), which their README does not
say and which costs the model strength:

- `--card-ids off`: the environment gives the model no card identity in the
  table of cards (a card there is its place, position and printed-like
  numbers; identities reach the model through the options only), where their
  server writes one.  `on` is what their server does.
- "End Phase" is not offered where "Battle Phase" is (main phase), nor where
  "Main Phase 2" is (battle phase): the environment never offers both.  The
  option is answered with a probability of -1.
- `--history-cancel skip`: the environment keeps a seat's own earlier actions
  for the model, and leaves out the times it declined (passed a chain, said
  no); their server keeps those too.  `record` is what their server does.

A probability of -1 means the option was not put to the model: it was left
out as above, it lies beyond the model's 24 options, or it is a card already
selected.  Every failure comes back as `{"error": ...}` and leaves the
duel's `index` where it was: the client answers by a rule of its own, says
so in its report, and sends its next request with the same index.  What it
did by its own rule is not among the earlier actions the model is shown.

Run it with ygo-agent's `scripts/` as the working directory: `code_list.txt`
is read from there.
"""

from __future__ import annotations

import argparse
import json
import os
import threading
import time
import uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

os.environ.setdefault("JAX_PLATFORMS", "cpu")

import numpy as np  # noqa: E402
from ygoinf import features  # noqa: E402
from ygoinf.models import BattleCmdType, IdleCmdType, Input  # noqa: E402

CANCEL = features.action_act_to_id[features.ActionAct.cancel]


class Duel:
    """What the server keeps of one seat of one duel between two requests."""

    def __init__(self) -> None:
        self.rstate = features.init_rstate()
        self.history = features.HistoryActions()
        self.index = 0
        # Of the last request: the options as the model was given them, which
        # of them each answered position stands for, and when it was asked.
        self.actions: np.ndarray | None = None
        self.places: list[int | None] = []
        self.turn = 0
        self.phase = None
        self.touched = time.time()


def without_end_phase(message) -> int | None:
    """Take "End Phase" out where the environment would not offer it, and say
    where it stood among the client's options."""
    data = message.data
    if data.msg_type == "select_idlecmd":
        kinds = [command.cmd_type for command in data.idle_cmds]
        if IdleCmdType.to_bp in kinds and IdleCmdType.to_ep in kinds:
            place = kinds.index(IdleCmdType.to_ep)
            del data.idle_cmds[place]
            return place
    elif data.msg_type == "select_battlecmd":
        kinds = [command.cmd_type for command in data.battle_cmds]
        if BattleCmdType.to_m2 in kinds and BattleCmdType.to_ep in kinds:
            place = kinds.index(BattleCmdType.to_ep)
            del data.battle_cmds[place]
            return place
    return None


def predict(
    model,
    duel: Duel,
    request: dict,
    *,
    card_ids: bool,
    history_cancel: bool,
    shown_to: list | None = None,
) -> dict:
    """Answer one request.  ``shown_to`` collects what the model was shown,
    for `golden_check.py`."""
    if int(request["index"]) != duel.index:
        raise ValueError(f"index mismatch: expected {duel.index}, got {request['index']}")
    if duel.actions is not None:
        # What the seat did at the request before this one, once: a request
        # that fails below leaves nothing to tell of at the next.
        actions, places, duel.actions = duel.actions, duel.places, None
        taken = int(request["prev_action_idx"])
        if not 0 <= taken < len(places) or places[taken] is None:
            raise ValueError(f"prev_action_idx {taken} is no option the model was given")
        action = actions[places[taken]]
        if history_cancel or action[4] != CANCEL:
            duel.history.update(action, duel.turn, duel.phase)
    given = Input(**request["input"])

    left_out = without_end_phase(given.action_msg)
    legal = features.get_legal_actions(given.action_msg)
    cards, specs = features.encode_cards(given.cards)
    actions = np.zeros((features.MAX_ACTIONS, features.N_ACTION_FEATURES), dtype=np.uint8)
    for place, action in enumerate(legal[: features.MAX_ACTIONS]):
        action.card_index, identity = features.find_spec_info(specs, action.spec)
        if action.card_id == 0:
            action.card_id = identity
        actions[place] = features.encode_action(action)
    if not card_ids:
        cards[:, 0:2] = 0
    shown = {
        "cards_": cards,
        "global_": features.encode_global(given.global_, given.cards),
        "actions_": actions,
        "h_actions_": duel.history.encode(given.global_.turn),
    }
    if shown_to is not None:
        shown_to.append(shown)
    if len(legal) == 1:
        # As in the environment, a single option is taken without asking.
        probs, win_rate = [1.0], -1.0
    else:
        duel.rstate, probs, value = model.predict(duel.rstate, shown)
        probs = features.revert_pad_truncate(list(probs), len(legal))
        win_rate = (value + 1) / 2
    probs, responses, can_finish = features.add_skipped_back(probs, legal, given.action_msg)
    # For a choice of cards the answer has one entry a card, in the client's
    # order, and a last one for "no more": their server loses the tail of it
    # where cards are already selected.
    can_finish = list(can_finish) + [False] * (len(probs) - len(can_finish))
    if left_out is not None:
        probs.insert(left_out, -1)
        responses.insert(left_out, 7 if given.action_msg.data.msg_type == "select_idlecmd" else 3)
        can_finish.insert(left_out, False)
    # The options the model was given, in the order of the answer.
    places: list[int | None] = []
    for prob in probs:
        places.append(None if prob == -1 else sum(place is not None for place in places))
    duel.actions, duel.places = actions, places
    duel.turn, duel.phase = given.global_.turn, given.global_.phase
    duel.index += 1
    duel.touched = time.time()
    return {
        "predict_results": {
            "action_preds": [
                {"prob": float(prob), "response": int(response), "can_finish": bool(finish)}
                for prob, response, finish in zip(probs, responses, can_finish, strict=True)
            ],
            "win_rate": float(win_rate),
        },
        "index": duel.index,
    }


def serve(args: argparse.Namespace) -> None:
    features.init_code_list(args.code_list)
    model = features.Predictor.load(args.checkpoint, args.threads)
    duels: dict[str, Duel] = {}
    lock = threading.Lock()
    log = open(args.log, "a", encoding="utf8") if args.log else None  # noqa: SIM115

    class Handler(BaseHTTPRequestHandler):
        def reply(self, status: int, body: object) -> None:
            data = body.encode() if isinstance(body, str) else json.dumps(body).encode()
            self.send_response(status)
            self.send_header("Content-Type", "text/plain" if isinstance(body, str) else "application/json")
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)

        def do_GET(self) -> None:  # noqa: N802
            self.reply(200, "OK")

        def do_DELETE(self) -> None:  # noqa: N802
            with lock:
                duels.pop(self.path.rstrip("/").rsplit("/", 1)[-1], None)
            self.send_response(204)
            self.end_headers()

        def do_POST(self) -> None:  # noqa: N802
            parts = self.path.strip("/").split("/")
            body = self.rfile.read(int(self.headers.get("Content-Length") or 0))
            with lock:
                if parts == ["v0", "duels"]:
                    name = str(uuid.uuid4())
                    duels[name] = Duel()
                    self.reply(200, {"duelId": name, "index": 0})
                    return
                if len(parts) != 4 or parts[:2] != ["v0", "duels"] or parts[3] != "predict":
                    self.reply(404, {"error": f"no such path: {self.path}"})
                    return
                duel = duels.get(parts[2])
                try:
                    if duel is None:
                        raise ValueError(f"duel {parts[2]} not found")
                    request = json.loads(body)
                    answer = predict(
                        model,
                        duel,
                        request,
                        card_ids=args.card_ids == "on",
                        history_cancel=args.history_cancel == "record",
                    )
                except Exception as error:  # noqa: BLE001 - the client answers by a rule of its own
                    answer = {"error": f"{type(error).__name__}: {error}"}
                if log is not None:
                    log.write(json.dumps({"duel": parts[2], "request": json.loads(body or b"null"), "answer": answer}) + "\n")
                    log.flush()
                self.reply(200, answer)

        def log_message(self, *_: object) -> None:
            return

    with ThreadingHTTPServer((args.host, args.port), Handler) as server:
        print(
            f"serving {args.checkpoint} on http://{args.host}:{args.port} "
            f"(card ids {args.card_ids}, history of declined actions: {args.history_cancel})",
            flush=True,
        )
        server.serve_forever()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--checkpoint", required=True, help="a .flax_model or .tflite file")
    parser.add_argument("--code-list", default="code_list.txt")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=3013)
    parser.add_argument("--threads", type=int, default=1)
    parser.add_argument("--card-ids", choices=("off", "on"), default="off")
    parser.add_argument("--history-cancel", choices=("skip", "record"), default="skip")
    parser.add_argument("--log", help="append every request and answer to this file, one JSON a line")
    serve(parser.parse_args())


if __name__ == "__main__":
    main()
