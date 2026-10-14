#!/usr/bin/env python3
"""Run the fixed root-world warmup host ablation using the frozen plan beside this file."""
from pathlib import Path
import contextlib
import hashlib
import importlib.util
import io
import json
import subprocess
import os
import signal
from datetime import datetime, timezone

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def save(name, value):
    with (HERE / name).open('x') as stream:
        json.dump(value, stream, indent=2, sort_keys=True)
        stream.write('\n')


def main():
    plan = json.loads((HERE / 'plan.json').read_text())
    assert not (HERE / 'manifest.json').exists(), 'No overwriting or resuming partial runs'
    def verify():
        for value in plan['artifacts'].values():
            assert Path(value['path']).stat().st_size == value['bytes']
            assert sha(value['path']) == value['sha256']
    verify()
    assert sha(ROOT/'benchmarks/generic-policy/analyze.py') == plan['analyzer_sha256']
    sources = {name: sha(HERE / name) for name in ('plan.json', 'run.py')}
    save('manifest.json', {'plan': plan, 'source_sha256': sources,
         'started_utc': datetime.now(timezone.utc).isoformat(),
         'launch_checkout': subprocess.check_output(['git','rev-parse','HEAD'], cwd=ROOT, text=True).strip()})
    # Serial arms: the sum of active match workers never exceeds 16.
    for job in plan['jobs']:
        with (HERE / (job['name'] + '.log')).open('x') as log:
            child = subprocess.Popen(job['command'], cwd=ROOT, stdout=log, stderr=subprocess.STDOUT,
                                     start_new_session=True)
            try:
                returncode = child.wait()
            except BaseException:
                os.killpg(child.pid, signal.SIGTERM)
                child.wait()
                raise
        save(job['name'] + '-exit.json', {'returncode': returncode,
             'finished_utc': datetime.now(timezone.utc).isoformat()})
        assert returncode == 0, 'Failed stage; retain partial data without scoring'
        verify()
    assert sources == {name: sha(HERE / name) for name in sources}
    spec = importlib.util.spec_from_file_location('roster_analysis', ROOT/'benchmarks/generic-policy/analyze.py')
    analysis = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(analysis)
    quality = {}
    coverage = {}
    for job in plan['jobs']:
        rows, summary, metadata = analysis.load(HERE / (job['name']+'.jsonl'))
        assert len(rows) == plan['pairs'] == 128
        assert set(metadata['policies']) == set(plan['policies'])
        assert metadata['search']['evaluation_weights'] == plan['evaluation_weights']
        for deck in plan['policies']:
            assert [sum(k[0] == deck and k[2] == seat for k in rows) for seat in (0, 1)] == [16, 16]
        assert metadata['arguments'] == dict(zip(job['command'][5::2],job['command'][6::2]))
        quality[job['name']] = {key: summary['overall'][key] for key in (
            'failures','caps','failed_searches','failed_playouts','mcts_cutoffs',
            'unresolved_playouts','incomplete_worlds','budget_exhausted')}
        assert not any(row['game'].get('search_errors') for row in rows.values())
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
