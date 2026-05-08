#!/usr/bin/env python3
"""Probe DM server with correct startup payload to capture the handshake."""
import socket
import struct
import sys

sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
sock.settimeout(5)
sock.connect(('127.0.0.1', 5236))
print("Connected to DM server")

def make_frame(msg_type, handle, body):
    """Build a DM protocol frame header + body."""
    body_len = len(body)
    header = bytearray(64)
    struct.pack_into('<i', header, 0, handle)    # handle at offset 0
    header[4] = msg_type                           # msg_type at offset 4
    struct.pack_into('<i', header, 6, body_len)    # body_len at offset 6
    # response_code at offset 10 = 0 (client msg)
    # reserved at offset 14 = 0
    header[18] = 0                                 # compress_flag
    # XOR checksum of bytes 0-18
    cs = 0
    for i in range(19):
        cs ^= header[i]
    header[19] = cs
    # rest is zeros (20-63)
    return bytes(header) + body

def put_string(buf, offset, s, encoding='utf-8'):
    """Put a length-prefixed string (i32 LE length + bytes)."""
    data = s.encode(encoding)
    struct.pack_into('<i', buf, offset, len(data))
    buf[offset+4:offset+4+len(data)] = data
    return offset + 4 + len(data)

# From Go driver:
# Dm_build_327 = 20 (payload start offset)
# Dm_build_858 = Dm_build_327 = 20
# Dm_build_859 = Dm_build_858 + ULINT_SIZE(4) = 24  (compress: int32)
# Dm_build_860 = Dm_build_859 + ULINT_SIZE(4) = 28  (login_encrypt: int32)
# Dm_build_861 = Dm_build_860 + BYTE_SIZE(1) = 29   (some flag)
# Dm_build_862 = Dm_build_861 + BYTE_SIZE(1) = 30   (bdta flag)
# Dm_build_863 = Dm_build_862 + BYTE_SIZE(1) = 31   (compressID)
# Dm_build_864 = Dm_build_863 + BYTE_SIZE(1) = 32   (loginCertificate)
# Dm_build_865 = Dm_build_864 + BYTE_SIZE(1) = 33
# Dm_build_866 = Dm_build_865 + BYTE_SIZE(1) = 34
# Dm_build_867 = Dm_build_866 + BYTE_SIZE(1) = 35   (MsgVersion: uint16)
# Then driver version string (Dm_build_282 = "7.6.0.0")
# Dm_build_868 = Dm_build_327 = 20 (separate section for login)
# Dm_build_869 = Dm_build_868 + ULINT_SIZE(4) = 24
# Dm_build_870 = Dm_build_869 + ULINT_SIZE(4) = 28
# Dm_build_871 = Dm_build_870 + ULINT_SIZE(4) = 32
# Dm_build_872 = Dm_build_871 + ULINT_SIZE(4) = 36
# Dm_build_873 = Dm_build_872 + ULINT_SIZE(4) = 40
# Dm_build_874 = Dm_build_873 + BYTE_SIZE(1) = 41
# Dm_build_875 = Dm_build_874 + BYTE_SIZE(1) = 42
# Dm_build_876 = Dm_build_875 + BYTE_SIZE(1) = 43
# Dm_build_877 = Dm_build_876 + BYTE_SIZE(1) = 44
# Dm_build_878 = Dm_build_877 + BYTE_SIZE(1) = 45
# Dm_build_879 = Dm_build_878 + USINT_SIZE(2) = 47
# Dm_build_880 = Dm_build_879 + BYTE_SIZE(1) = 48

# Build startup payload - just send minimal version info
# Looking at Go code line 2740-2782 for startup payload construction
payload = bytearray(256)

# Offset 20: int32 = 0 (some flag)
struct.pack_into('<i', payload, 20, 0)
# Offset 24: int32 = 0 (compress)
struct.pack_into('<i', payload, 24, 0)
# Offset 28: int32 = 0 (login_encrypt flag)
struct.pack_into('<i', payload, 28, 0)
# Offset 29: byte = 0 (Dm_build_861)
payload[29] = 0
# Offset 30: byte = 0 (Dm_build_862, not bdta)
payload[30] = 0
# Offset 31: byte = 0 (Dm_build_863, compressID)
payload[31] = 0
# Offset 32: byte = 0 (Dm_build_864, no certificate)
payload[32] = 0
# Offset 33: byte = 0 (Dm_build_865)
payload[33] = 0
# Offset 34: byte = 1 (Dm_build_866)
payload[34] = 1
# Offset 35: uint16 LE (MsgVersion)
struct.pack_into('<H', payload, 35, 0)

# Now write driver version string at offset 37
# Format: i32 LE length + string bytes
ver = b"7.6.0.0"
struct.pack_into('<i', payload, 37, len(ver))
payload[37+4:37+4+len(ver)] = ver

# After version string: catalog (database name) at offset 37+4+7=48
cat = b"SYSDBA"
struct.pack_into('<i', payload, 48, len(cat))
payload[48+4:48+4+len(cat)] = cat

# After catalog: host name at offset 48+4+6=58
host = b"localhost"
struct.pack_into('<i', payload, 58, len(host))
payload[58+4:58+4+len(host)] = host

total_payload = 58 + 4 + len(host)

# Send startup message (msg_type=200)
frame = make_frame(200, 0, bytes(payload[:total_payload]))
print(f"Sending STARTUP frame ({len(frame)} bytes, payload={total_payload})")
print(f"Frame hex: {frame[:64].hex()}")
print(f"Payload hex: {bytes(payload[:total_payload]).hex()}")
sock.sendall(frame)

import select
read_ready, _, _ = select.select([sock], [], [], 3)
if read_ready:
    resp = sock.recv(4096)
    print(f"\nServer response ({len(resp)} bytes):")
    if len(resp) >= 64:
        h = resp[:64]
        h_handle = struct.unpack_from('<i', h, 0)[0]
        h_msgtype = h[4]
        h_bodylen = struct.unpack_from('<i', h, 6)[0]
        h_respcode = struct.unpack_from('<i', h, 10)[0]
        h_checksum = h[19]
        calc_cs = 0
        for i in range(19):
            calc_cs ^= h[i]
        print(f"  Handle: {h_handle}, MsgType: {h_msgtype}, BodyLen: {h_bodylen}")
        print(f"  ResponseCode: {h_respcode}")
        print(f"  Checksum: {h_checksum}, computed: {calc_cs}, match: {h_checksum == calc_cs}")
        if len(resp) > 64:
            body = resp[64:]
            print(f"  Payload ({len(body)} bytes): {body[:200].hex()}")
            # Try to decode version string
            if len(body) > 4:
                ver_len = struct.unpack_from('<i', body, 0)[0]
                if ver_len > 0 and len(body) > 4 + ver_len:
                    print(f"  Server version: {body[4:4+ver_len]}")
                    # Challenge bytes after version
                    if len(body) > 4 + ver_len + 8:
                        print(f"  Challenge/Key bytes: {body[4+ver_len:4+ver_len+32].hex()}")
    else:
        print(f"  Raw: {resp.hex()}")
else:
    print("No response from server")

sock.close()
