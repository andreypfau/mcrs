#!/usr/bin/env python3
"""Run the pinned give experiments on the isolated tools/vanilla-server world.

First provision the server with tools/vanilla-server/run.sh fetch. The runner
creates a marked test world. It refuses an existing unmarked world. The
experiment clears probe inventories and removes item entities in that world.
It does not build mcrs or read Minecraft code.
"""
import argparse
import importlib.util
import json
import os
import re
import struct
import sys
import threading
import time
from pathlib import Path

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(HERE))
import give_client as client  # noqa: E402


def load_console():
    spec = importlib.util.spec_from_file_location('physics_record', ROOT / 'tools/physics-fixtures/record.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.Console


def contains_translation(value, key):
    if isinstance(value, dict):
        return value.get('translate') == key or any(contains_translation(v, key) for v in value.values())
    if isinstance(value, list):
        return any(contains_translation(v, key) for v in value)
    return False


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--out', type=Path, default=ROOT / 'target/give-results.json')
    parser.add_argument('--trace', type=Path, default=ROOT / 'target/give-trace.jsonl')
    args = parser.parse_args()
    if os.environ.get('MINECRAFT_EULA') != 'true':
        raise SystemExit('Set MINECRAFT_EULA=true after accepting https://aka.ms/MinecraftEULA')
    work = ROOT / 'target/vanilla-server' / client.VERSION['id']
    if not (work / 'server.jar').is_file():
        raise SystemExit('Provision the server with tools/vanilla-server/run.sh fetch first.')
    marker = work / '.mcrs-give-test-world'
    if not marker.exists():
        if (work / 'world').exists():
            raise SystemExit('Refusing to use an existing world without the give-test marker.')
        (work / 'eula.txt').write_text('eula=true\n')
        (work / 'server.properties').write_text(
            'online-mode=false\nserver-ip=127.0.0.1\nserver-port=25566\n'
            'network-compression-threshold=-1\nlevel-type=minecraft:flat\n'
            'spawn-protection=0\nview-distance=3\nsimulation-distance=3\n'
        )
        marker.write_text('World reserved for the mcrs give experiment.\n')
    properties = dict(line.split('=', 1) for line in (work / 'server.properties').read_text().splitlines()
                      if '=' in line and not line.startswith('#'))
    required = {'online-mode': 'false', 'server-ip': '127.0.0.1', 'server-port': '25566',
                'network-compression-threshold': '-1'}
    if any(properties.get(k) != v for k, v in required.items()):
        raise SystemExit('The test world must use offline mode, loopback port 25566 and no compression.')
    cases = json.loads((HERE / 'give_cases.json').read_text())
    scenario = json.loads((HERE / 'give_scenario.json').read_text())
    if scenario['version'] != client.VERSION['id']:
        raise SystemExit('Scenario and corpus versions differ.')
    events = []
    lock = threading.Lock()
    def emit(**value):
        with lock:
            events.append(value)
    client.emit = emit
    console = load_console()(str(work))
    clients = {}
    try:
        console.wait_for(r'Done \(', timeout=180)
        console.run(['whitelist off', 'difficulty peaceful', 'deop GiveProbeA', 'deop GiveProbeB',
                     'scoreboard objectives add give_result dummy',
                     'scoreboard objectives add give_success dummy',
                     'scoreboard objectives add picked_stone minecraft.picked_up:minecraft.stone'])
        for name in scenario['players']:
            clients[name] = client.Client('127.0.0.1', 25566, name)
        deadline = time.monotonic() + 30
        while True:
            with lock:
                joined = {x.get('client') for x in events if x.get('event') == 'joined'}
            if joined.issuperset(clients):
                break
            if time.monotonic() > deadline:
                raise TimeoutError('Probe players did not join.')
            time.sleep(.05)
        for connection in clients.values():
            connection.send('play', 'client_command', client.varint(0))
        time.sleep(1)
        console.run(['fill -3 99 -3 3 99 3 stone',
                     'execute in minecraft:overworld run tp GiveProbeA 0 100 0',
                     'execute in minecraft:overworld run tp GiveProbeB 2 100 0',
                     'gamemode survival @a',
                     'advancement revoke @a everything',
                     'op GiveProbeA', 'clear @a', 'kill @e[type=item]'])
        time.sleep(1)
        client_observations = []
        for step in scenario['steps']:
            with lock:
                start = len(events)
            emit(event='sent', step=step['id'], client=step['client'], command=step['command'])
            clients[step['client']].send('play', 'chat_command', client.string(step['command'].removeprefix('/')))
            time.sleep(scenario.get('quiet_seconds', 1))
            with lock:
                replies = [x for x in events[start:] if x.get('client') == step['client'] and x.get('event') == 'system_chat']
            found = any(contains_translation(x.get('text'), step['expect_translation']) for x in replies)
            client_observations.append({'id': step['id'], 'expected': step['expect_translation'], 'found': found})
            print(step['id'] + (': PASS' if found else ': FAIL'), flush=True)
        time.sleep(0.2)
        console.run([])
        observations = []
        for case in cases:
            if case.get('setup'):
                console.run(case['setup'])
                if case.get('delay_seconds'):
                    time.sleep(case['delay_seconds'])
                    console.run([])
            with lock:
                start = len(events)
            emit(event='case_start', case=case['id'])
            lines = console.run(case['commands'])
            time.sleep(.3)
            lines = [re.sub(r'^.*?System chat: ', '', x) for x in lines if 'System chat:' in x]
            joined = '\n'.join(lines)
            missing = [part for part in case['contains'] if part not in joined]
            missing += ['unexpected: ' + part for part in case.get('absent', []) if part in joined]
            if 'score_unchanged' in case:
                score = case['score_unchanged']
                pattern = rf"^{re.escape(score['holder'])} has (-?\d+) \[{re.escape(score['objective'])}\]$"
                values = re.findall(pattern, joined, re.MULTILINE)
                if len(values) != 2 or values[0] != values[1]:
                    missing.append(f"score changed or could not be read twice: {values}")
            sounds = None
            if 'pickup_sounds' in case:
                registry = json.loads((ROOT / 'assets/mcrs/reports/registries.json').read_text())
                sound_id = registry['minecraft:sound_event']['entries']['minecraft:entity.item.pickup']['protocol_id']
                with lock:
                    sounds = sum(1 for x in events[start:] if x.get('client') == 'GiveProbeA'
                                 and x.get('event') == 'sound'
                                 and client.read_varint(bytes.fromhex(x['body']))[0] == sound_id + 1)
                if sounds != case['pickup_sounds']:
                    missing.append(f"pickup sounds: expected {case['pickup_sounds']}, got {sounds}")
                if 'pickup_sound_position' in case:
                    with lock:
                        packets = [x for x in events[start:] if x.get('client') == 'GiveProbeA'
                                   and x.get('event') == 'sound'
                                   and client.read_varint(bytes.fromhex(x['body']))[0] == sound_id + 1]
                    positions = []
                    for packet in packets:
                        body = bytes.fromhex(packet['body'])
                        _, offset = client.read_varint(body)
                        _, offset = client.read_varint(body, offset)
                        positions.append(list(struct.unpack_from('>iii', body, offset)))
                    if positions != [case['pickup_sound_position']]:
                        missing.append(f"pickup sound positions: expected {[case['pickup_sound_position']]}, got {positions}")
            if 'pickup_sounds_b' in case:
                registry = json.loads((ROOT / 'assets/mcrs/reports/registries.json').read_text())
                sound_id = registry['minecraft:sound_event']['entries']['minecraft:entity.item.pickup']['protocol_id']
                with lock:
                    sounds_b = sum(1 for x in events[start:] if x.get('client') == 'GiveProbeB'
                                   and x.get('event') == 'sound'
                                   and client.read_varint(bytes.fromhex(x['body']))[0] == sound_id + 1)
                if sounds_b != case['pickup_sounds_b']:
                    missing.append(f"pickup sounds B: expected {case['pickup_sounds_b']}, got {sounds_b}")
            observations.append({'id': case['id'], 'commands': case['commands'],
                                 'output': lines, 'pickup_sounds': sounds, 'missing': missing})
            print(case['id'] + (': FAIL ' + repr(missing) if missing else ': PASS'), flush=True)
        with lock:
            trace = list(events)
        args.trace.write_text(''.join(json.dumps(e, ensure_ascii=False) + '\n' for e in trace))
        args.out.write_text(json.dumps({'version': client.VERSION['id'],
                                       'protocol': client.VERSION['protocol_version'],
                                       'client_observations': client_observations,
                                       'observations': observations}, ensure_ascii=False, indent=2) + '\n')
        if any(x['missing'] for x in observations) or any(not x['found'] for x in client_observations):
            raise SystemExit('One or more observations did not match. See ' + str(args.out))
        print('Saved', args.out, 'and', args.trace, flush=True)
    finally:
        for connection in clients.values():
            connection.socket.close()
        console.stop()


if __name__ == '__main__':
    main()
