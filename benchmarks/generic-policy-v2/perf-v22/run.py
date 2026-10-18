#!/usr/bin/env python3
"""Run the fixed single-worker v20/v22 configuration timing diagnostic using the frozen plan beside this file."""
from pathlib import Path
import contextlib
import hashlib
import importlib.util
import io
import json
import subprocess
import os
import signal
import time
import math
from datetime import datetime, timezone

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def save(name, value):
    with (HERE / name).open('x') as stream:
        json.dump(value, stream, indent=2, sort_keys=True)
        stream.write('\n')



def assert_exclusive(own_group=None):
    blocked = {'policy-bench','cargo','rustc','cc1','cc1plus','gcc','g++','clang','clang++','ld','ld.lld','lld','make','gmake','ninja','cmake'}
    conflicts = []
    for entry in Path('/proc').iterdir():
        if not entry.name.isdigit():
            continue
        try:
            name = (entry/'comm').read_text().strip()
            if name in blocked and (own_group is None or os.getpgid(int(entry.name)) != own_group):
                conflicts.append({'pid': int(entry.name), 'name': name})
        except (FileNotFoundError, ProcessLookupError):
            continue
    assert not conflicts, f'Concurrent visible build/match processes: {conflicts}'


def latency(rows, field):
    samples = sorted(value for row in rows for value in row['game'][field])
    assert all(type(x) in (int,float) and math.isfinite(x) and x >= 0 for x in samples)
    if not samples:
        return {'samples': 0}
    return {'samples':len(samples),'mean':sum(samples)/len(samples),'total':sum(samples),
            'p50':samples[math.ceil(len(samples)*.5)-1], 'p95':samples[math.ceil(len(samples)*.95)-1],
            'p99':samples[math.ceil(len(samples)*.99)-1], 'max':samples[-1]}


def timing(rows):
    for row in rows:
        game = row['game']
        assert game['decision_ms'] == game['decision_ms_by_seat'][row['seat']]
        assert len(game['search_decision_ms']) == game['searched']
    return {'all_candidate_decisions_ms':latency(rows,'decision_ms'),
            'fresh_search_decisions_ms':latency(rows,'search_decision_ms')}


def main():
    plan = json.loads((HERE / 'plan.json').read_text())
    assert not (HERE / 'manifest.json').exists(), 'No overwriting or resuming partial runs'
    def verify():
        for value in plan['artifacts'].values():
            assert Path(value['path']).stat().st_size == value['bytes']
            assert sha(value['path']) == value['sha256']
    verify()
    assert_exclusive()
    assert sha(ROOT/'benchmarks/generic-policy/analyze.py') == plan['analyzer_sha256']
    sources = {name: sha(HERE / name) for name in ('plan.json', 'run.py')}
    save('manifest.json', {'plan': plan, 'source_sha256': sources,
         'started_utc': datetime.now(timezone.utc).isoformat(),
         'launch_checkout': subprocess.check_output(['git','rev-parse','HEAD'], cwd=ROOT, text=True).strip()})
    # Serial arms: one match worker, no concurrent visible build or match process.
    for job in plan['jobs']:
        assert_exclusive()
        started = datetime.now(timezone.utc).isoformat()
        with (HERE / (job['name'] + '.log')).open('x') as log:
            child = subprocess.Popen(job['command'], cwd=ROOT, stdout=log, stderr=subprocess.STDOUT,
                                     start_new_session=True)
            try:
                while child.poll() is None:
                    assert_exclusive(child.pid)
                    time.sleep(2)
                returncode = child.wait()
            except BaseException:
                os.killpg(child.pid, signal.SIGTERM)
                child.wait()
                raise
        assert_exclusive()
        save(job['name'] + '-exit.json', {'returncode': returncode, 'started_utc': started, 'visible_build_match_exclusivity_checks_passed': True,
             'finished_utc': datetime.now(timezone.utc).isoformat()})
        assert returncode == 0, 'Failed stage; retain partial data without scoring'
        verify()
    assert sources == {name: sha(HERE / name) for name in sources}
    spec = importlib.util.spec_from_file_location('roster_analysis', ROOT/'benchmarks/generic-policy/analyze.py')
    analysis = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(analysis)
    quality = {}
    coverage = {}
    timings = {}
    for job in plan['jobs']:
        rows, summary, metadata = analysis.load(HERE / (job['name']+'.jsonl'))
        assert len(rows) == plan['pairs'] == 16
        assert set(metadata['policies']) == set(plan['policies'])
        assert metadata['search']['evaluation_weights'] == plan['evaluation_weights']
        for deck in plan['policies']:
            assert [sum(k[0] == deck and k[2] == seat for k in rows) for seat in (0, 1)] == [2, 2]
        assert metadata['arguments'] == dict(zip(job['command'][5::2],job['command'][6::2]))
        quality[job['name']] = {key: summary['overall'][key] for key in (
            'failures','caps','failed_searches','failed_playouts','mcts_cutoffs',
            'unresolved_playouts','incomplete_worlds','budget_exhausted')}
        assert not any(row['game'].get('search_errors') for row in rows.values())
        timings[job['name']] = {'overall': timing(list(rows.values())), 'per_deck': {deck:timing([row for key,row in rows.items() if key[0]==deck]) for deck in plan['policies']}}
        fields = ('mcts_root_actions_all_worlds_attempted', 'mcts_root_actions_all_worlds_completed')
        if job['name'] == 'control':
            assert all(not any(field in row['game'] for field in fields) for row in rows.values()), 'Unexpected control telemetry schema'
            coverage['control'] = {'available': False, 'reason': plan['coverage']['control']}
        else:
            totals = {field: 0 for field in ('mcts_root_actions',) + fields}
            for row in rows.values():
                game = row['game']
                assert all(type(game.get(field)) is int and game[field] >= 0 for field in totals), 'Missing or invalid candidate coverage'
                assert game[fields[1]] <= game[fields[0]] <= game['mcts_root_actions']
                for field in totals:
                    totals[field] += game[field]
            denominator = totals['mcts_root_actions']
            coverage['candidate'] = {'available': True, 'totals': totals,
                'attempted_fraction': totals[fields[0]] / denominator if denominator else None,
                'completed_fraction': totals[fields[1]] / denominator if denominator else None}
    save('root-coverage.json', {'definition': plan['coverage'], 'arms': coverage})
    save('latency.json', {'scope': plan['timing_notes'], 'arms': timings})
    save('quality.json', quality)
    assert not any(value for arm in quality.values() for value in arm.values())
    assert sha(ROOT/'benchmarks/generic-policy/analyze.py') == plan['analyzer_sha256']
    capture = io.StringIO()
    with contextlib.redirect_stdout(capture):
        analysis.main([str(HERE/'control.jsonl'), str(HERE/'candidate.jsonl')], allow_candidate_change=True)
    report = json.loads(capture.getvalue())
    assert report['comparisons'][0]['candidate_revision_changes'] == ['candidate_executable']
    assert not report['comparisons'][0]['configuration_changes']
    save('comparison.json', report)
    print(json.dumps(report['comparisons'][0]['overall'], indent=2))


if __name__ == '__main__':
    main()
