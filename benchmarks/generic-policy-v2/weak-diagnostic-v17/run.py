#!/usr/bin/env python3
"""Replay only the fixed historical Psychic/Exodia samples and verify exact parity. Requires root launch authorization."""
from pathlib import Path
from datetime import datetime, timezone
import gzip
import hashlib
import json
import os
import signal
import subprocess
import types

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[2]


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def save(name, value):
    with (HERE/name).open('x') as stream:
        json.dump(value,stream,indent=2,sort_keys=True);stream.write('\n')


def main():
    plan=json.loads((HERE/'plan.json').read_text())
    assert not (HERE/'manifest.json').exists(), 'Refusing overwrite or partial-run resume'
    def verify():
        for item in list(plan['artifacts'].values())+list(plan['reference']['files'].values())+[plan['reference']['archive']]:
            path=Path(item['path'])
            assert path.stat().st_size==item['bytes'] and sha(path)==item['sha256']
        raw=gzip.decompress(Path(plan['reference']['archive']['path']).read_bytes())
        assert len(raw)==plan['reference']['archive']['logical_bytes']
        assert hashlib.sha256(raw).hexdigest()==plan['reference']['archive']['logical_sha256']
    verify()
    source=subprocess.check_output(['git','show',plan['analyzer_source_commit']+':benchmarks/generic-policy/analyze.py'],cwd=ROOT)
    assert hashlib.sha256(source).hexdigest()==plan['analyzer_sha256']
    analysis=types.ModuleType('frozen_analysis')
    exec(compile(source,'frozen_analyze.py','exec'),analysis.__dict__)
    old_rows,_,old_meta=analysis.load(Path(plan['reference']['archive']['path']))
    expected={(row['opponent'],row['seed'],row['seat']):row for row in plan['reference']['expected_games']}
    assert len(expected)==plan['pairs']==32
    assert set(expected)=={key for key in old_rows if key[0] in plan['policies']}
    for key,expected_row in expected.items():
        assert all(old_rows[key]['game'][field]==expected_row[field] for field in ('winner','reason','turns','decisions','digest'))
    # Root schedules the sole match pool; also reject visible active benchmark processes.
    for comm in Path('/proc').glob('[0-9]*/comm'):
        try:
            assert comm.read_text().strip()!='policy-bench', f'Another match pool exists: {comm.parent.name}'
        except FileNotFoundError:
            pass
    hashes={name:sha(HERE/name) for name in ('plan.json','run.py')}
    save('manifest.json',{'plan':plan,'source_sha256':hashes,'started_utc':datetime.now(timezone.utc).isoformat(),
        'launch_checkout':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()})
    job=plan['jobs'][0]
    with (HERE/'search32.log').open('x') as log:
        child=subprocess.Popen(job['command'],cwd=ROOT,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
        try:
            code=child.wait()
        except BaseException:
            os.killpg(child.pid,signal.SIGTERM);child.wait();raise
    save('exit.json',{'returncode':code,'finished_utc':datetime.now(timezone.utc).isoformat()})
    assert code==0, 'Incomplete replay is not valid evidence'
    verify()
    assert hashes=={name:sha(HERE/name) for name in hashes}
    rows,summary,meta=analysis.load(HERE/'search32.jsonl')
    assert set(rows)==set(expected)
    assert meta['arguments']==dict(zip(job['command'][5::2],job['command'][6::2]))
    assert meta['search']['evaluation_weights']==plan['evaluation_weights']
    old_context=analysis.pinned_context(old_meta)
    old_context.update(policies=sorted(plan['policies']),planned_jobs=32,
                       decks={key:value for key,value in old_context['decks'].items() if key in plan['policies']})
    assert old_context==analysis.pinned_context(meta)
    assert analysis.recorded_configuration(old_meta)==analysis.recorded_configuration(meta)
    for field in ('candidate','executable'):
        assert analysis.file_identity(old_meta[field],field)==analysis.file_identity(meta[field],field)
    quality={field:summary['overall'][field] for field in ('failures','caps','failed_searches','failed_playouts','mcts_cutoffs','unresolved_playouts','incomplete_worlds','budget_exhausted')}
    save('quality.json',quality)
    assert not any(quality.values()) and not any(row['game']['search_errors'] for row in rows.values())
    checks=[]
    for key,row in sorted(rows.items()):
        assert all(row['game'][field]==expected[key][field] for field in ('winner','reason','turns','decisions','digest')), key
        assert row['game']['trace'], f'Missing recorded trace: {key}'
        checks.append({**expected[key],'exact_match':True,'trace_entries':len(row['game']['trace'])})
    save('replay-checks.json',{'scope':plan['scope'],'all_32_exact_matches':True,'checked_fields':['opponent','seed','seat','winner','reason','turns','decisions','digest'],'games':checks})
    print('All 32 historical games replayed exactly with traces. No new strength inference.')


if __name__=='__main__':
    main()
