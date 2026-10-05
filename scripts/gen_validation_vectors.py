# generate validation vectors: {input, expect_codes, schema_invalid}
# <META - FILE SUMMARY - Tooling script; see docstring below.>
import json, copy, os

base = json.load(open('fixture/example_project.json', encoding='utf-8'))
out_dir = 'fixture/validation'
os.makedirs(out_dir, exist_ok=True)

def put(name, input_obj, codes, schema_invalid):
    vec = {"input": input_obj, "expect_codes": codes, "schema_invalid": schema_invalid}
    with open(os.path.join(out_dir, name), 'w', encoding='utf-8', newline='\n') as f:
        json.dump(vec, f, indent=2, ensure_ascii=False)
        f.write('\n')

def mutate(fn):
    d = copy.deepcopy(base)
    fn(d)
    return d

i = 0
def emit(codes, schema_invalid, mutator):
    global i
    i += 1
    put(f'v{i:02d}.json', mutate(mutator), codes, schema_invalid)

# structural (schema-detectable)
emit(['E_UNKNOWN_FIELD'], True, lambda d: d.update({'unknown_field': 1}))
emit(['E_UNKNOWN_FIELD'], True, lambda d: d['lanes'][0].update({'foo': 1}))
emit(['E_UNKNOWN_FIELD'], True, lambda d: d['tempo'][0].update({'foo': 1}))
emit(['E_VERSION_TOO_NEW'], True, lambda d: d.update({'format_version': 2}))
emit(['E_FORMAT'], True, lambda d: d.pop('title'))
emit(['E_FORMAT'], True, lambda d: d.pop('ppq'))
emit(['E_FORMAT'], True, lambda d: d['tempo'].pop(0) or d.update(tempo=[]))
emit(['E_FORMAT'], True, lambda d: d.update({'ppq': 240}))
emit(['E_RANGE'], True, lambda d: d['tempo'][0].update({'bpm': 10}))
emit(['E_RANGE'], True, lambda d: d['tempo'][0].update({'bpm': 500}))
emit(['E_RANGE'], True, lambda d: d['lanes'][0].update({'program': 128}))
emit(['E_RANGE'], True, lambda d: d['lanes'][0].update({'volume': 200}))
emit(['E_FORMAT'], True, lambda d: d['meter'][0].update({'den': 3}))
emit(['E_RANGE'], True, lambda d: d['meter'][0].update({'num': 0}))
emit(['E_RANGE'], True, lambda d: d['lanes'][0].update({'id': 0}))
emit(['E_FORMAT'], True, lambda d: d['lanes'][0].update({'kind': 'melodic2'}))
emit(['E_RANGE'], True, lambda d: d['notes'][0].__setitem__(3, 200))
emit(['E_RANGE'], True, lambda d: d['notes'][0].__setitem__(4, 0))
emit(['E_FORMAT'], True, lambda d: d['notes'].__setitem__(0, [1, 0, 120, 60]))
emit(['E_RANGE'], True, lambda d: d.update({'length_ticks': 0}))
emit(['E_RANGE'], True, lambda d: d['loop'].update({'start': 5, 'end': 0}))
emit(['E_RANGE'], True, lambda d: d['lanes'].__setitem__(0, dict(d['lanes'][0], program=300)))

# semantic (schema may pass)
emit(['E_LANE_DUP'], False, lambda d: d['lanes'].append(copy.deepcopy(d['lanes'][0])))
emit(['E_LANE_MISSING'], False, lambda d: d['notes'].__setitem__(0, [9, 0, 480, 72, 100]))
emit(['E_OVERLAP'], False, lambda d: d['notes'].append([1, 100, 480, 72, 100]))
emit(['E_OVERLAP'], False, lambda d: d['notes'].append([2, 1000, 500, 48, 100]))
emit(['E_TEMPO'], False, lambda d: d['tempo'].insert(0, {'tick': 100, 'bpm': 120.0}))
emit(['E_TEMPO'], False, lambda d: d['tempo'][0].update({'tick': 100}))
emit(['E_TEMPO'], False, lambda d: d['tempo'].append({'tick': 0, 'bpm': 120.0}))
emit(['E_METER'], False, lambda d: d['meter'].append({'tick': 100, 'num': 3, 'den': 4}))
emit(['E_METER'], False, lambda d: d['meter'][0].update({'tick': 1}))
emit(['E_METER'], False, lambda d: d['meter'].append({'tick': 0, 'num': 4, 'den': 4}))
emit(['E_LOOP'], False, lambda d: d['loop'].update({'start': 7680, 'end': 100}))
emit(['E_LOOP'], False, lambda d: d['loop'].update({'start': 0, 'end': 99999}))
emit(['W_NOTE_PAST_LENGTH'], False, lambda d: d.update({'length_ticks': 10}))
emit(['W_DRUM_KEY_RANGE'], False, lambda d: d['notes'].append([4, 960, 120, 100, 100]))
emit(['W_UNUSED_LANE'], False, lambda d: d['lanes'].append({'id': 7, 'name': 'X', 'kind': 'melodic', 'program': 5}))
emit(['W_BPM_ROUNDED'], False, lambda d: d['tempo'][0].update({'bpm': 396.8}))
emit(['E_RANGE'], False, lambda d: d['notes'].append([1, 4294967295, 4294967295, 60, 100]))
emit(['E_FORMAT'], True, lambda d: d.update({'format_version': 'one'}))
emit(['E_FORMAT'], True, lambda d: d.update({'title': ''}))
emit(['E_FORMAT'], True, lambda d: d['notes'].__setitem__(0, 'oops'))
emit(['E_FORMAT'], True, lambda d: d.update({'lanes': 'nope'}))
emit(['E_FORMAT'], True, lambda d: d.update({'tempo': [{'tick': 0}]}))
emit(['E_FORMAT'], True, lambda d: d.pop('format_version'))
emit(['E_RANGE'], True, lambda d: d['notes'].__setitem__(0, [1, 0, 0, 60, 100]))
print('wrote', i, 'vectors')