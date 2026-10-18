#!/usr/bin/env python3
"""Freeze the prespecified Psychic Commander plan only after final build records exist. Never runs matches."""
from pathlib import Path
import argparse
import hashlib
import json
import re

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
BASE_SEED = 2493000


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def build(path, source):
    value = json.loads(Path(path).read_text())
    assert re.fullmatch(r'[0-9a-f]{40}', source) and value['source_commit'] == source
    assert value['build_command']
    artifact = Path(value['path'])
    assert artifact.stat().st_size == value['bytes'] and sha(artifact) == value['sha256']
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--host-build', required=True)
    parser.add_argument('--host-source', required=True)
    parser.add_argument('--candidate-build', required=True)
    parser.add_argument('--candidate-source', required=True)
    args = parser.parse_args()
    assert not (HERE/'plan.json').exists() and not (HERE/'manifest.json').exists()
    # A repetition of this base seed in another finalized experiment is not allowed.
    for pattern in ('**/plan.json', '**/*.metadata.json'):
        for path in (ROOT/'benchmarks').glob(pattern):
            if path.parent == HERE:
                continue
            value = json.loads(path.read_text())
            assert value.get('base_seed', value.get('seed')) != BASE_SEED, f'Seed already used: {path}'
    host = build(args.host_build, args.host_source)
    candidate = build(args.candidate_build, args.candidate_source)
    control = build('/tmp/ygo-generic-v24-chain-payoff/library-build.json', 'bc4beeab208ccecbfa4a9ed915606e0f794d21a4')
    assert Path(host['path']).name == 'policy-bench'
    assert Path(candidate['path']).name == 'libygo_policies.so'
    assert candidate['sha256'] != control['sha256']
    template = json.loads((ROOT/'benchmarks/generic-policy-v2/taiyou-v21/plan.json').read_text())
    weights = json.loads((ROOT/'benchmarks/generic-policy-v2/root-warmup-v22/plan.json').read_text())['evaluation_weights']
    plan = {key:value for key,value in template.items() if key not in ('artifacts','executable_build','library_builds','jobs','analyzer_sha256')}
    plan.update(base_seed=BASE_SEED, games_per_deck=128, pairs=128, policies=['psychic'],
        evaluation_weights=weights, root_warmup=False,
        analyzer_sha256=sha(ROOT/'benchmarks/generic-policy/analyze.py'),
        prepared_by={'script':'prepare.py','sha256':sha(HERE/'prepare.py')},
        scope='Fixed fresh 128-pair Psychic mirror development comparison per mode, plain and MCTS32. Four serial arms, 64 games per candidate seat, base2493000, unchanged default weights. Only the loaded policy library changes within each mode: v24 control versus the combined v25 Announce adapter and Psychic Commander transaction fix. The adapter runs inside each loaded FFI through ocgcore::Seat; the control library retains the old hint behavior, while the candidate includes the new hint and Commander logic. This intentionally bundled prerequisite is not an isolated payment-choice or adapter experiment. Both arms use the identical new v25 host, UCT default and explicit root_warmup=false. No Exodia patch, weight change, early stopping, grid or tuning; not qualification or unseen-deck evidence.',
        host_shared_changes=['same frozen host in all four arms','UCT default; --mcts-root-warmup false explicit','face-down banished-card hidden-information guard'],
        library_experiment=['numeric Announce hint adapter inside the loaded FFI; v24 keeps old hint behavior','Psychic Commander transaction/payment logic bundled with the adapter prerequisite','no Exodia patch','no weight changes'],
        seed_nonreuse_verified=True)
    plan['executable_build']=host
    plan['library_builds']={'control':control,'candidate':candidate}
    plan['artifacts']={'baseline':template['artifacts']['baseline'],**{name:{k:value[k] for k in ('path','bytes','sha256')} for name,value in [('executable',host),('control_library',control),('candidate_library',candidate)]}}
    for value in plan['artifacts'].values():
        assert Path(value['path']).stat().st_size == value['bytes'] and sha(value['path']) == value['sha256']
    plan['jobs']=[]
    for old in template['jobs']:
        name=old['name']; mode,arm=name.split('-')
        flags=dict(zip(old['command'][5::2],old['command'][6::2]))
        flags.update({'--library':plan['artifacts'][arm+'_library']['path'], '--policies':'psychic',
                      '--games':'128','--seed':str(BASE_SEED),'--mcts-root-warmup':'false',
                      '--output':str(HERE/(name+'.jsonl'))})
        command=['taskset','-c','8-23',host['path'],'generic']+[item for pair in flags.items() for item in pair]
        plan['jobs'].append({'name':name,'command':command})
    for first,second in plan['comparisons']:
        commands={job['name']:dict(zip(job['command'][5::2],job['command'][6::2])) for job in plan['jobs']}
        assert {key for key in commands[first] if commands[first][key] != commands[second][key]} == {'--library','--output'}
    with (HERE/'plan.json').open('x') as stream:
        json.dump(plan,stream,indent=2,sort_keys=True); stream.write('\n')
    print('Frozen',HERE/'plan.json','; no matches launched.')


if __name__ == '__main__':
    main()
