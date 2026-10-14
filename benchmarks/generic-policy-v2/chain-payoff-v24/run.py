#!/usr/bin/env python3
"""Run the fixed direct chain-payoff library comparisons using the frozen plan beside this file."""
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
    sources = {name: sha(HERE / name) for name in ('plan.json', 'run.py', 'prepare.py')}
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
    for job in plan['jobs']:
        rows, summary, metadata = analysis.load(HERE / (job['name']+'.jsonl'))
        assert len(rows) == plan['pairs'] == 128
        assert metadata['policies'] == ['chain-burn']
        assert metadata['seed'] == plan['base_seed'] == 2492000
        assert metadata['games_per_matchup'] == 128
        assert [sum(key[2] == seat for key in rows) for seat in (0,1)] == [64,64]
        assert metadata['search']['evaluation_weights'] == plan['evaluation_weights']
        assert metadata['search']['mcts']['root_warmup'] is False
        assert metadata['arguments'] == dict(zip(job['command'][5::2],job['command'][6::2]))
        quality[job['name']] = {key: summary['overall'][key] for key in (
            'failures','caps','failed_searches','failed_playouts','mcts_cutoffs',
            'unresolved_playouts','incomplete_worlds','budget_exhausted')}
        assert not any(row['game'].get('search_errors') for row in rows.values())
    save('quality.json', quality)
    assert not any(value for arm in quality.values() for value in arm.values())
    assert sha(ROOT/'benchmarks/generic-policy/analyze.py') == plan['analyzer_sha256']
    for control, candidate in plan['comparisons']:
        capture = io.StringIO()
        with contextlib.redirect_stdout(capture):
            analysis.main([str(HERE/(control+'.jsonl')), str(HERE/(candidate+'.jsonl'))], allow_candidate_change=True)
        report = json.loads(capture.getvalue())
        assert report['comparisons'][0]['candidate_revision_changes'] == ['candidate_library']
        assert not report['comparisons'][0]['configuration_changes']
        save(control.split('-')[0]+'-comparison.json', report)
        print(control, json.dumps(report['comparisons'][0]['overall']), flush=True)



if __name__ == '__main__':
    main()
