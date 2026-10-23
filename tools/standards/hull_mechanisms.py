"""Cross references in hull-authored mechanism states; no geometry qualification."""
import math


def errors(hull):
    found = []
    mechanisms, exits = hull.get('mechanisms', []), hull.get('exits', [])
    if not isinstance(mechanisms, list) or not isinstance(exits, list):
        return found  # malformed collections are schema errors
    if mechanisms or exits:
        if not isinstance(hull.get('model'), str) or not hull['model'].strip():
            found.append('mechanisms/exits require a model path')
        if not isinstance(hull.get('model_sha256'), str) or not hull['model_sha256']:
            found.append('mechanisms/exits require model_sha256')

    def unique_strings(values, where):
        strings = [x for x in values if isinstance(x, str)]
        if len(strings) != len(set(strings)):
            found.append(f'{where}: duplicate names')
        return set(strings)

    mech_keys = unique_strings([m.get('key') for m in mechanisms if isinstance(m, dict)], 'mechanisms')
    state_keys, node_owners = {}, {}
    for i, mechanism in enumerate(mechanisms):
        if not isinstance(mechanism, dict):
            continue
        where = f'mechanisms[{i}]'
        key = mechanism.get('key')
        nodes, states = mechanism.get('nodes'), mechanism.get('states')
        if not isinstance(nodes, list) or not isinstance(states, list):
            continue
        declared = unique_strings(nodes, where + '.nodes')
        for node in declared:
            if node in node_owners:
                found.append(f'{where}: node {node!r} already owned by {node_owners[node]}')
            node_owners[node] = where
        keys = unique_strings([s.get('key') for s in states if isinstance(s, dict)], where + '.states')
        if isinstance(key, str):
            state_keys[key] = keys
        if not isinstance(mechanism.get('default'), str) or mechanism['default'] not in keys:
            found.append(f'{where}.default: no such state')
        if mechanism.get('deploys_when') in ('landed', 'always') or 'contact' in mechanism or mechanism.get('kind') == 'gear':
            if 'deployed' not in keys:
                found.append(f'{where}: automatic/contact/gear mechanism requires deployed state')
        if mechanism.get('kind') == 'gear' and 'stowed' not in keys:
            found.append(f'{where}: gear requires stowed state')
        for j, state in enumerate(states):
            if not isinstance(state, dict) or not isinstance(state.get('poses'), list):
                continue
            poses = state['poses']
            sw = f'{where}.states[{j}].poses'
            posed = unique_strings([p.get('node') for p in poses if isinstance(p, dict)], sw)
            if posed != declared:
                found.append(f'{sw}: must cover exactly the declared nodes; missing {sorted(declared - posed)}, extra {sorted(posed - declared)}')
            for pose in poses:
                if not isinstance(pose, dict) or pose.get('kind') != 'rigid':
                    continue
                rotation = pose.get('rotation_xyzw')
                if isinstance(rotation, list) and len(rotation) == 4 and all(isinstance(x, (int, float)) and not isinstance(x, bool) for x in rotation):
                    norm = math.hypot(*rotation)
                    if not math.isfinite(norm) or abs(norm - 1.0) > 1e-6:
                        found.append(f'{sw}: rotation_xyzw must be a unit quaternion (norm tolerance 1e-6)')
    unique_strings([e.get('key') for e in exits if isinstance(e, dict)], 'exits')
    for i, exit in enumerate(exits):
        if not isinstance(exit, dict):
            continue
        where = f'exits[{i}]'
        if exit.get('kind') in ('climb', 'step') and not exit.get('foot'):
            found.append(f'{where}: climb/step requires foot marker')
        if exit.get('foot') and exit.get('top') == exit['foot']:
            found.append(f'{where}: top and foot must be distinct markers')
        requirements = exit.get('requires')
        if not isinstance(requirements, list):
            continue
        unique_strings([r.get('mechanism') for r in requirements if isinstance(r, dict)], where + '.requires')
        for requirement in requirements:
            if not isinstance(requirement, dict):
                continue
            mechanism, state = requirement.get('mechanism'), requirement.get('state')
            if not isinstance(mechanism, str) or mechanism not in mech_keys:
                found.append(f'{where}.requires: no mechanism {mechanism!r}')
            elif not isinstance(state, str) or state not in state_keys.get(mechanism, set()):
                found.append(f'{where}.requires: no state {state!r} in mechanism {mechanism!r}')
    return found


def check_records(records):
    return [(path, error) for key, path, record, _ in records if key.startswith('hull.')
            for error in errors(record)]
