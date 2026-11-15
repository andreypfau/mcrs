#!/usr/bin/env python3
"""Decode the give subtree from commands packets in a JSONL give_client capture.

Uses only the packet and registry reports and docs/spec/commands-packet.md.
"""
import json
import struct
import sys
from pathlib import Path

sys.dont_write_bytecode = True
from give_client import ROOT, VERSION, read_varint

registry = json.loads((ROOT / 'assets/mcrs/reports/registries.json').read_text())
types = {v['protocol_id']: k for k, v in registry['minecraft:command_argument_type']['entries'].items()}


class Reader:
    def __init__(self, data): self.data, self.pos = data, 0
    def raw(self, size):
        result = self.data[self.pos:self.pos + size]
        if len(result) != size: raise ValueError('truncated data')
        self.pos += size
        return result
    def byte(self): return self.raw(1)[0]
    def var(self):
        value, self.pos = read_varint(self.data, self.pos)
        return value
    def string(self): return self.raw(self.var()).decode('utf-8')
    def number(self, fmt): return struct.unpack('>' + fmt, self.raw(struct.calcsize(fmt)))[0]


def decode(data):
    reader = Reader(data)
    nodes = []
    for _ in range(reader.var()):
        flags = reader.byte()
        children = [reader.var() for _ in range(reader.var())]
        node = {'flags': flags, 'executable': bool(flags & 4), 'children': children}
        if flags & 8: node['redirect'] = reader.var()
        kind = flags & 3
        if kind in (1, 2): node['name'] = reader.string()
        if kind == 2:
            parser = types[reader.var()]
            node['parser'] = parser
            if parser in ('brigadier:float', 'brigadier:double', 'brigadier:integer', 'brigadier:long'):
                mask = reader.byte()
                fmt = {'brigadier:float': 'f', 'brigadier:double': 'd', 'brigadier:integer': 'i', 'brigadier:long': 'q'}[parser]
                node['properties'] = {'flags': mask}
                if mask & 1: node['properties']['min'] = reader.number(fmt)
                if mask & 2: node['properties']['max'] = reader.number(fmt)
            elif parser == 'brigadier:string': node['properties'] = reader.var()
            elif parser in ('minecraft:entity', 'minecraft:score_holder'): node['properties'] = reader.byte()
            elif parser == 'minecraft:time': node['properties'] = reader.number('i')
            elif parser in ('minecraft:resource_or_tag', 'minecraft:resource_or_tag_key', 'minecraft:resource', 'minecraft:resource_key', 'minecraft:resource_selector'):
                node['properties'] = reader.string()
            if flags & 16: node['suggestions'] = reader.string()
        elif kind not in (0, 1): raise ValueError('invalid node kind')
        nodes.append(node)
    root = reader.var()
    if reader.pos != len(data): raise ValueError(f'{len(data)-reader.pos} trailing bytes')
    return nodes, root


def subtree(nodes, index):
    node = dict(nodes[index])
    node['index'] = index
    node['children'] = [subtree(nodes, child) for child in node['children']]
    return node


def main():
    path = Path(sys.argv[1])
    versions = {}
    for line in path.read_text().splitlines():
        event = json.loads(line)
        if event.get('event') == 'joined':
            version = event.get('version')
            protocol = event.get('protocol')
            if version != VERSION['id'] or protocol != VERSION['protocol_version']:
                raise ValueError(f'unexpected game version for {event["client"]}: {version}, {protocol}')
            versions[event['client']] = version
        if event.get('event') != 'commands': continue
        if event['client'] not in versions:
            raise ValueError(f'no joined event for {event["client"]}')
        nodes, root = decode(bytes.fromhex(event['body']))
        matches = [i for i in nodes[root]['children'] if nodes[i].get('name') == 'give']
        print(json.dumps({'client': event['client'], 'version': versions[event['client']], 'node_count': len(nodes), 'give': [subtree(nodes, i) for i in matches]}, ensure_ascii=False))


if __name__ == '__main__': main()
