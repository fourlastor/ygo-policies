#!/usr/bin/env python3
"""Run only the two fixed, serial iteration-budget arms in the adjacent plan."""
from pathlib import Path
import contextlib
import hashlib
import importlib.util
import io
import json
import os
import signal
import subprocess
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
    assert not (HERE / 'manifest.json').exists(), 'No overwrite or partial-run resume'
    sources = {name: sha(HERE / name) for name in ('plan.json', 'run.py', 'analyzer.py')}
    assert sources['analyzer.py'] == plan['analyzer_sha256']

    def verify():
        assert sources == {name: sha(HERE / name) for name in sources}
        for value in plan['artifacts'].values():
            assert Path(value['path']).stat().st_size == value['bytes']
            assert sha(value['path']) == value['sha256']

    verify()
    save('manifest.json', {'plan': plan, 'source_sha256': sources,
         'started_utc': datetime.now(timezone.utc).isoformat(),
         'launch_checkout': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()})
    # One child at a time; its taskset command confines all16 match workers.
    for job in plan['jobs']:
        with (HERE / (job['name'] + '.log')).open('x') as log:
            child = subprocess.Popen(job['command'], cwd=ROOT, stdout=log, stderr=subprocess.STDOUT,
                                     start_new_session=True)
            try:
                code = child.wait()
            except BaseException:
                os.killpg(child.pid, signal.SIGTERM)
                child.wait()
                raise
        save(job['name'] + '-exit.json', {'returncode': code,
             'finished_utc': datetime.now(timezone.utc).isoformat()})
        assert code == 0, 'Failed arm; preserve partial evidence without scoring'
        verify()

    spec = importlib.util.spec_from_file_location('frozen_analysis', HERE / 'analyzer.py')
    analysis = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(analysis)
    quality = {}
    for job in plan['jobs']:
        rows, summary, meta = analysis.load(HERE / (job['name'] + '.jsonl'))
        assert len(rows) == plan['pairs'] == 64
        assert set(meta['policies']) == set(plan['policies'])
        assert meta['seed'] == plan['base_seed'] == 2495000
        assert meta['games_per_matchup'] == 16 and meta['workers'] == 16
        assert meta['plain'] is False and meta['search']['mcts']['root_warmup'] is False
        assert meta['search']['mcts']['iterations'] == job['iterations']
        assert meta['search']['evaluation_weights'] == plan['evaluation_weights']
        assert meta['arguments'] == dict(zip(job['command'][5::2], job['command'][6::2]))
        for deck in plan['policies']:
            assert [sum(k[0] == deck and k[2] == seat for k in rows) for seat in (0, 1)] == [8, 8]
        for field, artifact in [('executable', 'executable'), ('candidate', 'library'), ('baseline', 'baseline')]:
            assert all(meta[field][key] == plan['artifacts'][artifact][key] for key in ('path', 'bytes'))
        quality[job['name']] = {key: summary['overall'][key] for key in (
            'failures', 'caps', 'failed_searches', 'failed_playouts', 'mcts_cutoffs',
            'unresolved_playouts', 'incomplete_worlds', 'budget_exhausted',
            'soft_budget_overruns', 'plan_mismatches', 'live_plan_mismatches')}
        assert not any(row['game'].get('search_errors') for row in rows.values())
    save('quality.json', quality)
    assert not any(value for arm in quality.values() for value in arm.values())
    capture = io.StringIO()
    with contextlib.redirect_stdout(capture):
        analysis.main([str(HERE / (job['name'] + '.jsonl')) for job in plan['jobs']],
                      allow_metadata_ablation=plan['allowed_metadata_ablation'])
    report = json.loads(capture.getvalue())
    comparison = report['comparisons'][0]
    assert comparison['candidate_revision_changes'] == []
    assert comparison['configuration_changes'] == plan['expected_configuration_changes']
    assert comparison['isolated_metadata_ablation'] is True
    verify()
    save('comparison.json', report)
    print(json.dumps(comparison['overall'], indent=2))


if __name__ == '__main__':
    main()
