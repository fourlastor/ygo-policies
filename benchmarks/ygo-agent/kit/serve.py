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

Many requests at once.  The model answers a request alone in 4 ms on a CPU
and 128 of them together in 18 ms on a GTX 1060.  With `--batch N` the
requests that are waiting are answered together, N at a time (fewer are
filled up with empty ones, so that every batch is the same size: on a GPU a
request's answer is then the same to the last bit whatever shares its
batch; on a CPU it can differ in the last digit).  `--fronts N` starts N
processes that read the requests and build the model's input, which is the
rest of the work.  `--device gpu` puts the model on the GPU.

    serve.py --checkpoint ... --device gpu --batch 128 --fronts 6

A duel without a session, `POST /v1/predict`.  The server keeps a duel's
memory (the model's own state and the seat's earlier actions) between the
requests of `/v0/duels/ID/predict`.  `/v1/predict` keeps nothing: the answer
carries the memory as `state`, and the client sends it back with its next
request (`{"input": .., "prev_action_idx": .., "state": ..}`, no state at
the duel's first request).  A client can then go on from one point of a duel
in several directions, each with its own copy, and any front can answer any
request.  After an `error` the answer still carries a state, to be sent
with the next request.  With more than one front only `/v1/predict` is
served.

Run it with ygo-agent's `scripts/` as the working directory: `code_list.txt`
is read from there.
"""

from __future__ import annotations

import argparse
import base64
import json
import multiprocessing
import os
import queue
import signal
import socket
import sys
import threading
import time
import uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

import numpy as np
from ygoinf import features
from ygoinf.models import BattleCmdType, IdleCmdType, Input, Phase

CANCEL = features.action_act_to_id[features.ActionAct.cancel]
ACTIONS_SHAPE = (features.MAX_ACTIONS, features.N_ACTION_FEATURES)
STATE_SHAPES = (
    ((1, features.N_RNN_CHANNELS), np.float32),
    ((1, features.N_RNN_CHANNELS), np.float32),
    (features.H_ACTIONS_SHAPE, np.uint8),
    (ACTIONS_SHAPE, np.uint8),
)


class Duel:
    """What is kept of one seat of one duel between two requests."""

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

    def dumps(self) -> dict:
        """All of it, as a client of `/v1/predict` carries it."""
        actions = self.actions if self.actions is not None else np.zeros(ACTIONS_SHAPE, dtype=np.uint8)
        parts = (*self.rstate, self.history.h_actions, actions)
        data = b"".join(np.asarray(part, dtype=kind).tobytes() for part, (_, kind) in zip(parts, STATE_SHAPES, strict=True))
        return {
            "index": self.index,
            "at": self.history.ha_p,
            "turn": self.turn,
            "phase": None if self.phase is None else self.phase.value,
            "places": self.places if self.actions is not None else None,
            "bytes": base64.b64encode(data).decode(),
        }

    @classmethod
    def loads(cls, state: dict | None) -> Duel:
        duel = cls()
        if state is None:
            return duel
        data = base64.b64decode(state["bytes"])
        parts, offset = [], 0
        for shape, kind in STATE_SHAPES:
            size = int(np.prod(shape)) * np.dtype(kind).itemsize
            parts.append(np.frombuffer(data, dtype=kind, count=int(np.prod(shape)), offset=offset).reshape(shape).copy())
            offset += size
        if offset != len(data):
            raise ValueError("state of another size")
        duel.rstate = (parts[0], parts[1])
        duel.history.h_actions = parts[2]
        duel.history.ha_p = int(state["at"])
        duel.index = int(state["index"])
        duel.turn = int(state["turn"])
        duel.phase = None if state["phase"] is None else Phase(state["phase"])
        if state["places"] is not None:
            duel.actions, duel.places = parts[3], list(state["places"])
        return duel


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


class Asked:
    """One request, up to where the model answers it."""

    def __init__(self, given, legal, left_out, actions, shown) -> None:
        self.given, self.legal, self.left_out, self.actions, self.shown = given, legal, left_out, actions, shown

    @property
    def single(self) -> bool:
        """As in the environment, a single option is taken without asking."""
        return len(self.legal) == 1


def before(duel: Duel, request: dict, *, card_ids: bool, history_cancel: bool) -> Asked:
    """Take a request as far as the model: what the seat did at the request
    before it joins its earlier actions, and the model's input is built."""
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
    actions = np.zeros(ACTIONS_SHAPE, dtype=np.uint8)
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
    return Asked(given, legal, left_out, actions, shown)


def after(duel: Duel, asked: Asked, probs: list[float] | None, value: float | None) -> dict:
    """The answer to a request once the model has given its part (`probs`
    for the 24 places of its input and `value`; neither for a single option)."""
    given, legal, left_out = asked.given, asked.legal, asked.left_out
    if asked.single:
        probs, win_rate = [1.0], -1.0
    else:
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
    duel.actions, duel.places = asked.actions, places
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


def predict(
    model,
    duel: Duel,
    request: dict,
    *,
    card_ids: bool,
    history_cancel: bool,
    shown_to: list | None = None,
) -> dict:
    """Answer one request of a session with a model of this process, one
    request at a time.  ``shown_to`` collects what the model was shown, for
    `golden_check.py`."""
    if int(request["index"]) != duel.index:
        raise ValueError(f"index mismatch: expected {duel.index}, got {request['index']}")
    asked = before(duel, request, card_ids=card_ids, history_cancel=history_cancel)
    if shown_to is not None:
        shown_to.append(asked.shown)
    if asked.single:
        return after(duel, asked, None, None)
    duel.rstate, probs, value = model.predict(duel.rstate, asked.shown)
    return after(duel, asked, probs, value)


class Model:
    """The model in the server's first process, asked by the fronts."""

    def __init__(self, checkpoint: str, batch: int) -> None:
        import jax.numpy as jnp
        from ygoinf.jax_inf import get_probs_and_value, load_model

        self.jnp, self.run, self.batch = jnp, get_probs_and_value, batch
        self.shapes = {key: (value.shape, value.dtype) for key, value in features.sample_input().items()}
        self.params = load_model(checkpoint, features.init_rstate(), features.sample_input())
        self.answer([])

    def answer(self, waiting: list[tuple]) -> list[tuple]:
        """Each of `waiting` is the model's input and its state; the batch is
        filled up with empty requests to its one size."""
        count, size = len(waiting), self.batch
        obs = {key: np.zeros((size, *shape), dtype=kind) for key, (shape, kind) in self.shapes.items()}
        carried = [np.zeros((size, features.N_RNN_CHANNELS), dtype=np.float32) for _ in range(2)]
        for row, (shown, rstate) in enumerate(waiting):
            for key in obs:
                obs[key][row] = shown[key]
            for part, kept in zip(rstate, carried, strict=True):
                kept[row] = np.asarray(part).reshape(-1)
        jnp = self.jnp
        (c, h), probs, value = self.run(self.params, tuple(jnp.array(part) for part in carried), {key: jnp.array(part) for key, part in obs.items()})
        c, h, probs, value = np.asarray(c), np.asarray(h), np.asarray(probs), np.asarray(value).reshape(-1)
        return [(probs[row].tolist(), float(value[row]), (c[row : row + 1], h[row : row + 1])) for row in range(count)]


class Asker:
    """A front's line to the model: requests go out with a ticket, and the
    answers that come back are handed to whoever holds it."""

    def __init__(self, front: int, requests, answers) -> None:
        self.front, self.requests, self.answers = front, requests, answers
        self.waiting: dict[int, list] = {}
        self.lock = threading.Lock()
        self.ticket = 0
        threading.Thread(target=self.receive, daemon=True).start()

    def receive(self) -> None:
        while True:
            ticket, result = self.answers.get()
            with self.lock:
                slot = self.waiting.pop(ticket, None)
            if slot is not None:
                slot[1] = result
                slot[0].set()

    def predict(self, rstate, shown):
        slot = [threading.Event(), None]
        with self.lock:
            self.ticket += 1
            ticket = self.ticket
            self.waiting[ticket] = slot
        self.requests.put((self.front, ticket, shown, tuple(np.asarray(part) for part in rstate)))
        if not slot[0].wait(120):
            with self.lock:
                self.waiting.pop(ticket, None)
            raise TimeoutError("the model did not answer")
        probs, value, rstate = slot[1]
        return rstate, probs, value


def front(number: int, args: argparse.Namespace, requests, answers, ready, parent: int) -> None:
    """One of the processes that take the requests: it reads them, builds the
    model's input, has the model asked and writes the answer."""
    features.init_code_list(args.code_list)
    model = Asker(number, requests, answers)
    duels: dict[str, Duel] = {}
    lock = threading.Lock()
    log = open(args.log, "a", encoding="utf8") if args.log and number == 0 else None  # noqa: SIM115
    options = {"card_ids": args.card_ids == "on", "history_cancel": args.history_cancel == "record"}

    def session(name: str, request: dict) -> dict:
        """A request of `/v0`: the duel's memory is kept here.  One request
        of a duel at a time, as their server takes them."""
        if args.fronts > 1:
            raise ValueError("duels kept by the server need --fronts 1: use /v1/predict")
        with lock:
            duel = duels.get(name)
        if duel is None:
            raise ValueError(f"duel {name} not found")
        return predict(model, duel, request, **options)

    def carried(request: dict) -> dict:
        """A request of `/v1`: the duel's memory comes with it and goes back
        with the answer, also where the request could not be answered."""
        duel = Duel.loads(request.get("state"))
        try:
            asked = before(duel, request, **options)
            if asked.single:
                answer = after(duel, asked, None, None)
            else:
                duel.rstate, probs, value = model.predict(duel.rstate, asked.shown)
                answer = after(duel, asked, probs, value)
        except Exception as error:  # noqa: BLE001 - the client answers by a rule of its own
            answer = {"error": f"{type(error).__name__}: {error}"}
        answer["state"] = duel.dumps()
        return answer

    class Handler(BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.1"

        def setup(self) -> None:
            super().setup()
            # A connection that stays open must not wait to send a short answer.
            self.request.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)

        def reply(self, status: int, body: object) -> None:
            """The whole answer in one piece."""
            data = b"" if body is None else body.encode() if isinstance(body, str) else json.dumps(body).encode()
            kind = "text/plain" if isinstance(body, str) else "application/json"
            head = f"HTTP/1.1 {status} {'OK' if status < 300 else 'Error'}\r\nContent-Type: {kind}\r\nContent-Length: {len(data)}\r\n\r\n"
            self.wfile.write(head.encode() + data)

        def do_GET(self) -> None:  # noqa: N802
            self.reply(200, "OK")

        def do_DELETE(self) -> None:  # noqa: N802
            with lock:
                duels.pop(self.path.rstrip("/").rsplit("/", 1)[-1], None)
            self.reply(204, None)

        def do_POST(self) -> None:  # noqa: N802
            parts = self.path.strip("/").split("/")
            body = self.rfile.read(int(self.headers.get("Content-Length") or 0))
            if parts == ["v0", "duels"]:
                name = str(uuid.uuid4())
                with lock:
                    duels[name] = Duel()
                self.reply(200, {"duelId": name, "index": 0})
                return
            try:
                if parts == ["v1", "predict"]:
                    answer = carried(json.loads(body))
                elif len(parts) == 4 and parts[:2] == ["v0", "duels"] and parts[3] == "predict":
                    answer = session(parts[2], json.loads(body))
                else:
                    self.reply(404, {"error": f"no such path: {self.path}"})
                    return
            except Exception as error:  # noqa: BLE001 - the client answers by a rule of its own
                answer = {"error": f"{type(error).__name__}: {error}"}
            if log is not None:
                log.write(json.dumps({"path": self.path, "request": json.loads(body or b"null"), "answer": answer}) + "\n")
                log.flush()
            self.reply(200, answer)

        def log_message(self, *_: object) -> None:
            return

    class Server(ThreadingHTTPServer):
        daemon_threads = True
        request_queue_size = 1024

        def server_bind(self) -> None:
            # Every front listens on the one port; a connection goes to one of them.
            self.socket.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEPORT, 1)
            super().server_bind()

    def orphaned() -> None:
        while os.getppid() == parent:
            time.sleep(1)
        os._exit(0)

    threading.Thread(target=orphaned, daemon=True).start()
    with Server((args.host, args.port), Handler) as server:
        ready.put(number)
        server.serve_forever()


def serve(args: argparse.Namespace) -> None:
    if args.fronts < 1 or args.batch < 1:
        raise SystemExit("--fronts and --batch are at least 1")
    os.environ["JAX_PLATFORMS"] = "cuda" if args.device == "gpu" else "cpu"
    if args.device == "gpu":
        # Leave the card's memory to whatever else uses it.
        os.environ.setdefault("XLA_PYTHON_CLIENT_PREALLOCATE", "false")
    context = multiprocessing.get_context("spawn")
    requests = context.Queue()
    answers = [context.Queue() for _ in range(args.fronts)]
    ready = context.Queue()
    fronts = [
        context.Process(target=front, args=(number, args, requests, answers[number], ready, os.getpid()), daemon=True)
        for number in range(args.fronts)
    ]
    features.init_code_list(args.code_list)
    model = Model(args.checkpoint, args.batch)
    for process in fronts:
        process.start()

    def stop(*_: object) -> None:
        for process in fronts:
            process.terminate()
        sys.exit(0)

    signal.signal(signal.SIGTERM, stop)
    signal.signal(signal.SIGINT, stop)
    for _ in fronts:
        ready.get()
    print(
        f"serving {args.checkpoint} on http://{args.host}:{args.port} "
        f"(card ids {args.card_ids}, history of declined actions: {args.history_cancel}; "
        f"{args.device}, {args.batch} at a time, {args.fronts} front{'s' if args.fronts > 1 else ''})",
        flush=True,
    )
    # The requests are taken off the fronts' line as they come, also while
    # the model is busy: what has gathered by the time it is free is its
    # next batch.
    gathered: queue.SimpleQueue = queue.SimpleQueue()

    def gather() -> None:
        while True:
            gathered.put(requests.get())

    threading.Thread(target=gather, daemon=True).start()
    asked = batches = 0
    reported = time.time()
    while True:
        waiting = [gathered.get()]
        while len(waiting) < args.batch:
            try:
                waiting.append(gathered.get_nowait())
            except queue.Empty:
                break
        results = model.answer([(shown, rstate) for _, _, shown, rstate in waiting])
        for (number, ticket, _, _), result in zip(waiting, results, strict=True):
            answers[number].put((ticket, result))
        asked += len(waiting)
        batches += 1
        if args.report and time.time() - reported >= args.report:
            print(f"{asked} requests put to the model in {batches} batches ({asked / batches:.1f} a batch)", flush=True)
            reported = time.time()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--checkpoint", required=True, help="a .flax_model file")
    parser.add_argument("--code-list", default="code_list.txt")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=3013)
    parser.add_argument("--device", choices=("cpu", "gpu"), default="cpu")
    parser.add_argument("--batch", type=int, default=1, help="requests the model answers together")
    parser.add_argument("--fronts", type=int, default=1, help="processes that take the requests")
    parser.add_argument("--card-ids", choices=("off", "on"), default="off")
    parser.add_argument("--history-cancel", choices=("skip", "record"), default="skip")
    parser.add_argument("--log", help="append every request and answer of the first front to this file, one JSON a line")
    parser.add_argument("--report", type=float, default=0, help="say every so many seconds how full the batches are")
    serve(parser.parse_args())


if __name__ == "__main__":
    main()
