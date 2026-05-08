#!/usr/bin/env python3
"""Probe DM server with raw socket to capture startup response bytes."""
import socket
import struct

sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
sock.settimeout(5)
sock.connect(('127.0.0.1', 5236))
print("Connected to DM server")

# Send a minimal 64-byte frame header (msg_type=200 STARTUP, handle=0, body_len=82)
# Layout: handle(4) + msg_type(1) + reserved(1) + body_len(4) + resp_code(4) + reserved(4) + compress(1) + checksum(1) + zeros(44)
header = bytearray(64)
struct.pack_into('<i', header, 0, 0)    # handle
header[4] = 200                           # msg_type STARTUP
struct.pack_into('<i', header, 6, 82)    # body_len
struct.pack_into('<i', header, 10, 0)    # response_code
struct.pack_into('<i', header, 14, 0)    # reserved
header[18] = 0                            # compress_flag
# Compute XOR checksum of bytes 0-18
cs = 0
for i in range(19):
    cs ^= header[i]
header[19] = cs

print(f"Sending startup frame (64 bytes header + 82 bytes payload)")
print(f"Header hex: {bytes(header).hex()}")

# Build minimal startup payload (82 bytes)
payload = bytearray(82)
# From Python driver analysis - startup payload format
payload[0] = 1  # major
payload[1] = 1  # minor
payload[2] = 0  # patch
payload[3] = 0  # reserved
payload[4] = 1  # UTF-8 encoding

# Send header + payload
sock.sendall(bytes(header))
sock.sendall(bytes(payload))
print(f"Sent {64 + 82} bytes total")

# Read response
import select
read_ready, _, _ = select.select([sock], [], [], 3)
if read_ready:
    resp = sock.recv(4096)
    print(f"Server response ({len(resp)} bytes):")
    print(f"  Hex: {resp.hex()}")
    if len(resp) >= 64:
        h = resp[:64]
        h_handle = struct.unpack_from('<i', h, 0)[0]
        h_msgtype = h[4]
        h_bodylen = struct.unpack_from('<i', h, 6)[0]
        h_respcode = struct.unpack_from('<i', h, 10)[0]
        h_compress = h[18]
        h_checksum = h[19]
        # Verify checksum
        calc_cs = 0
        for i in range(19):
            calc_cs ^= h[i]
        print(f"  Handle: {h_handle}")
        print(f"  MsgType: {h_msgtype}")
        print(f"  BodyLen: {h_bodylen}")
        print(f"  ResponseCode: {h_respcode}")
        print(f"  CompressFlag: {h_compress}")
        print(f"  Checksum: {h_checksum}, computed: {calc_cs}, match: {h_checksum == calc_cs}")
        if len(resp) > 64:
            print(f"  Payload ({len(resp)-64} bytes): {resp[64:].hex()}")
else:
    print("No response from server (timeout)")

sock.close()
