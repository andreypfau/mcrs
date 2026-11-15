#!/usr/bin/env python3
"""A small offline protocol client for give experiments; consumes JSON lines on stdin.

Run with: python3 tools/mcrs_minecraft_probe/give_client.py --name GiveProbeA
Send {"command":"give @s stone"} and read JSON events on stdout. The server must
use the corpus version, offline mode, and network-compression-threshold=-1.
Packet numbers come from assets/mcrs/reports/packets.json. No game code is read.
"""
import argparse
import json
import socket
import struct
import sys
import threading
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PACKETS = json.loads((ROOT / "assets/mcrs/reports/packets.json").read_text())
VERSION = json.loads((ROOT / "assets/minecraft/version.json").read_text())
OUTPUT = threading.Lock()


def emit(**event):
    with OUTPUT:
        print(json.dumps(event, ensure_ascii=False, separators=(",", ":")), flush=True)


def varint(number):
    number &= 0xffffffff
    result = bytearray()
    while number > 0x7f:
        result.append((number & 0x7f) | 0x80)
        number >>= 7
    result.append(number)
    return bytes(result)


def read_varint(data, offset=0):
    number = 0
    for index in range(5):
        byte = data[offset + index]
        number |= (byte & 0x7f) << (7 * index)
        if byte < 0x80:
            return number, offset + index + 1
    raise ValueError("invalid VarInt")


def string(value):
    raw = value.encode("utf-8")
    return varint(len(raw)) + raw


class NbtReader:
    def __init__(self, data):
        self.data = data
        self.offset = 0

    def take(self, count):
        result = self.data[self.offset:self.offset + count]
        if len(result) != count:
            raise ValueError("truncated NBT")
        self.offset += count
        return result

    def number(self, fmt):
        return struct.unpack(">" + fmt, self.take(struct.calcsize(fmt)))[0]

    def text(self):
        return self.take(self.number("H")).decode("utf-8", errors="replace")

    def payload(self, kind):
        if kind == 1: return self.number("b")
        if kind == 2: return self.number("h")
        if kind == 3: return self.number("i")
        if kind == 4: return self.number("q")
        if kind == 5: return self.number("f")
        if kind == 6: return self.number("d")
        if kind == 7: return list(self.take(self.number("i")))
        if kind == 8: return self.text()
        if kind == 9:
            element = self.number("B")
            return [self.payload(element) for _ in range(self.number("i"))]
        if kind == 10:
            result = {}
            while True:
                child = self.number("B")
                if child == 0: return result
                name = self.text()
                result[name] = self.payload(child)
        if kind == 11: return [self.number("i") for _ in range(self.number("i"))]
        if kind == 12: return [self.number("q") for _ in range(self.number("i"))]
        raise ValueError(f"unsupported NBT tag {kind}")

    def root(self):
        return self.payload(self.number("B"))


class Client:
    def __init__(self, host, port, name):
        self.socket = socket.create_connection((host, port), timeout=30)
        self.socket.settimeout(None)
        self.name = name
        self.phase = "login"
        self.position = [0.0, 0.0, 0.0, 0.0, 0.0]
        self.send_lock = threading.Lock()
        self.ids = {
            (phase, direction, name.removeprefix("minecraft:")): spec["protocol_id"]
            for phase, directions in PACKETS.items()
            for direction, packets in directions.items()
            for name, spec in packets.items()
        }
        self.names = {
            (phase, direction, value): name
            for (phase, direction, name), value in self.ids.items()
        }
        self.send("handshake", "intention", varint(VERSION["protocol_version"]) + string(host) + struct.pack(">H", port) + varint(2))
        self.send("login", "hello", string(name) + bytes(16))
        self.thread = threading.Thread(target=self.receive_loop, daemon=True)
        self.thread.start()

    def send(self, phase, name, body=b""):
        frame = varint(self.ids[phase, "serverbound", name]) + body
        with self.send_lock:
            self.socket.sendall(varint(len(frame)) + frame)

    def exact(self, size):
        result = bytearray()
        while len(result) < size:
            chunk = self.socket.recv(size - len(result))
            if not chunk: raise EOFError("server closed")
            result.extend(chunk)
        return bytes(result)

    def length(self):
        value = 0
        for i in range(5):
            byte = self.exact(1)[0]
            value |= (byte & 0x7f) << (7 * i)
            if byte < 0x80: return value
        raise ValueError("invalid frame length")

    def receive_loop(self):
        try:
            while True:
                frame = self.exact(self.length())
                ident, pos = read_varint(frame)
                body = frame[pos:]
                packet = self.names.get((self.phase, "clientbound", ident), f"unknown_{ident}")
                self.handle(packet, body)
        except (OSError, EOFError, ValueError) as error:
            emit(client=self.name, event="closed", reason=str(error))

    def handle(self, packet, body):
        if self.phase == "login":
            if packet == "login_finished":
                if body[-1] != 0: raise ValueError("login requested options; use compression disabled")
                self.send("login", "login_acknowledged")
                self.phase = "configuration"
                info = string("en_us") + bytes([2]) + varint(0) + bytes([1, 127]) + varint(1) + bytes([0, 1]) + varint(0)
                self.send("configuration", "client_information", info)
            else:
                emit(client=self.name, event="login_packet", packet=packet, body=body.hex())
            return
        if self.phase == "configuration":
            if packet == "select_known_packs": self.send("configuration", packet, varint(0))
            elif packet == "keep_alive": self.send("configuration", packet, body)
            elif packet == "ping": self.send("configuration", "pong", body)
            elif packet == "finish_configuration":
                self.send("configuration", packet)
                self.phase = "play"
            elif packet == "disconnect": emit(client=self.name, event="disconnect", body=body.hex())
            return
        if packet == "login":
            emit(client=self.name, event="joined", version=VERSION["id"], protocol=VERSION["protocol_version"])
        elif packet == "keep_alive": self.send("play", packet, body)
        elif packet == "ping": self.send("play", "pong", body)
        elif packet == "player_position":
            ident, pos = read_varint(body)
            x, y, z, _, _, _, yaw, pitch, flags = struct.unpack(">ddddddffi", body[pos:pos + 60])
            values = [x, y, z, yaw, pitch]
            self.position = [old + new if flags & (1 << index) else new
                             for index, (old, new) in enumerate(zip(self.position, values))]
            self.send("play", "accept_teleportation", varint(ident) + struct.pack(">dddff", *self.position))
        elif packet == "system_chat":
            try:
                reader = NbtReader(body)
                text = reader.root()
                emit(client=self.name, event="system_chat", text=text, overlay=bool(body[reader.offset]))
            except (ValueError, IndexError) as error:
                emit(client=self.name, event="decode_error", packet=packet, reason=str(error), body=body.hex())
        elif packet == "container_set_slot":
            try:
                container, pos = read_varint(body)
                state, pos = read_varint(body, pos)
                slot = struct.unpack(">h", body[pos:pos+2])[0]
                count, pos = read_varint(body, pos+2)
                item = None
                if count: item, pos = read_varint(body, pos)
                emit(client=self.name, event=packet, container=container, state=state, slot=slot, count=count, item_id=item, rest=body[pos:].hex())
            except (ValueError, IndexError, struct.error) as error:
                emit(client=self.name, event="decode_error", packet=packet, reason=str(error), body=body.hex())
        elif packet in ("commands", "container_set_content", "add_entity", "set_entity_data", "remove_entities", "take_item_entity", "sound", "sound_entity", "disconnect"):
            emit(client=self.name, event=packet, body=body.hex())


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--name", required=True)
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=25566)
    args = parser.parse_args()
    client = Client(args.host, args.port, args.name)
    try:
        for line in sys.stdin:
            action = json.loads(line)
            if "command" in action:
                if client.phase != "play": raise ValueError("client is not in play state")
                command = action["command"].removeprefix("/")
                emit(client=args.name, event="sent", command=command)
                client.send("play", "chat_command", string(command))
            elif action.get("close"):
                break
    finally:
        client.socket.close()


if __name__ == "__main__": main()
