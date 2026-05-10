#!/usr/bin/env python3
"""
dump_row.py - Raw DM protocol row-dump utility.

Connects to a Dameng database via raw TCP sockets, authenticates,
executes "SELECT ID, NAME, AGE FROM RUST_TEST WHERE OBJECTID IS NOT NULL"
using the OPE(91) optimized path, captures the ACK payload bytes, and
prints a detailed hex dump with field-level annotations.

Protocol reference:
  Frame header (64 bytes):
    0  i32 LE  handle
    4  u8      msg_type
    5  u8      reserved (0)
    6  i32 LE  body_len
    10 i32 LE  response_code (filled by server)
    14 i32 LE  reserved (0)
    18 u8      compress_flag (0)
    19 u8      checksum (XOR of bytes 0..19)
    20 [44x0]  padding

  Message types: 200=STARTUP, 228=STARTUP_RESPONSE, 1=LOGIN,
    163=LOGIN_RESPONSE, 3=READY, 187=ACK, 91=OPE
"""

import socket
import struct
import sys

HOST = "127.0.0.1"
PORT = 5236
USERNAME = "SYSDBA"
PASSWORD = "SYSDBA"
SQL = "SELECT ID, NAME, AGE FROM RUST_TEST WHERE OBJECTID IS NOT NULL"

FRAME_SIZE = 64


# ---------------------------------------------------------------------------
# Frame helpers
# ---------------------------------------------------------------------------

def build_frame(msg_type: int, handle: int, body: bytes) -> bytes:
    """Build a 64-byte frame header + payload."""
    frame = bytearray(FRAME_SIZE)
    struct.pack_into('<i', frame, 0, handle)
    frame[4] = msg_type
    # 5 = reserved 0
    struct.pack_into('<i', frame, 6, len(body))
    # 10 = response_code 0 (client)
    # 14 = reserved 0
    frame[18] = 0  # compress_flag
    # checksum at 19
    cs = 0
    for i in range(19):
        cs ^= frame[i]
    frame[19] = cs
    return bytes(frame) + body


def verify_checksum(header: bytes) -> bool:
    cs = 0
    for i in range(19):
        cs ^= header[i]
    return cs == header[19]


def parse_frame(header: bytes) -> dict:
    handle = struct.unpack_from('<i', header, 0)[0]
    msg_type = header[4]
    body_len = struct.unpack_from('<i', header, 6)[0]
    resp_code = struct.unpack_from('<i', header, 10)[0]
    compress = header[18]
    checksum = header[19]
    ok = verify_checksum(header)
    return {
        'handle': handle,
        'msg_type': msg_type,
        'body_len': body_len,
        'resp_code': resp_code,
        'compress': compress,
        'checksum': checksum,
        'checksum_ok': ok,
    }


MSG_NAMES = {
    0: 'EXEC_RESPONSE', 1: 'LOGIN', 3: 'READY', 5: 'EXEC',
    7: 'FETCH', 8: 'COMMIT', 9: 'ROLLBACK', 13: 'BIND',
    20: 'CLOSE', 27: 'SET_CURSOR', 44: 'FETCH_RESULT',
    52: 'SET_ISOLATION', 91: 'OPE(91)', 160: 'FETCH_RESP',
    163: 'LOGIN_RESP', 187: 'ACK', 200: 'STARTUP', 228: 'STARTUP_RESP',
}
def msg_name(t):
    return MSG_NAMES.get(t, f'UNKNOWN({t})')


# ---------------------------------------------------------------------------
# I/O helpers
# ---------------------------------------------------------------------------

def send(sock: socket.socket, data: bytes):
    total = len(data)
    sent = 0
    while sent < total:
        n = sock.send(data[sent:])
        if n == 0:
            raise RuntimeError("Connection closed while sending")
        sent += n
    sys.stderr.write(f"  >> sent {sent} bytes\n")


def recv_exact(sock: socket.socket, n: int) -> bytes:
    buf = bytearray()
    while len(buf) < n:
        chunk = sock.recv(n - len(buf))
        if not chunk:
            raise RuntimeError(f"Connection closed, got {len(buf)}/{n} bytes")
        buf.extend(chunk)
    return bytes(buf)


def read_message(sock: socket.socket) -> dict:
    header = recv_exact(sock, FRAME_SIZE)
    info = parse_frame(header)
    body = b''
    if info['body_len'] > 0:
        body = recv_exact(sock, info['body_len'])
    info['body'] = body
    sys.stderr.write(
        f"  << {msg_name(info['msg_type'])} "
        f"handle={info['handle']} body={info['body_len']} "
        f"resp={info['resp_code']} chk={'OK' if info['checksum_ok'] else 'FAIL'}\n"
    )
    return info


# ---------------------------------------------------------------------------
# Startup
# ---------------------------------------------------------------------------

def send_startup(sock: socket.socket):
    version = b'7.6.0.0'
    key = bytes([(i * 7 + 13) & 0xFF for i in range(64)])
    payload = struct.pack('<i', len(version)) + version + b'\x00'
    payload += struct.pack('<i', len(key)) + key
    send(sock, build_frame(200, 0, payload))


def read_startup_response(sock: socket.socket) -> bytes:
    msg = read_message(sock)
    assert msg['msg_type'] in (228, 187), f"Expected 228/187, got {msg['msg_type']}"
    assert msg['resp_code'] >= 0, f"Startup failed: {msg['resp_code']}"
    # Extract challenge: after version string in payload
    data = msg['body']
    ver_len = struct.unpack_from('<I', data, 16)[0]
    ver_start = 20
    after_ver = ver_start + ver_len
    # skip sentinel (4 bytes) then key_len (4 bytes), then key data (64 bytes)
    key_len = struct.unpack_from('<I', data, after_ver + 4)[0]
    challenge = data[after_ver + 8: after_ver + 8 + key_len]
    sys.stderr.write(f"  server version={data[ver_start:ver_start+ver_len]} "
                     f"challenge_len={len(challenge)}\n")
    return challenge


# ---------------------------------------------------------------------------
# Login
# ---------------------------------------------------------------------------

def xor_encrypt(plaintext: bytes, challenge: bytes) -> bytes:
    if not challenge:
        return plaintext
    cl = len(challenge)
    return bytes(p ^ challenge[i % cl] for i, p in enumerate(plaintext))


def send_login(sock: socket.socket, challenge: bytes):
    un = xor_encrypt(USERNAME.encode(), challenge)
    pw = xor_encrypt(PASSWORD.encode(), challenge)
    os_name = f"macOS darwin".encode()
    hostname = HOST.encode() + b'\x00'
    payload = struct.pack('<i', len(un)) + un
    payload += struct.pack('<i', len(pw)) + pw
    payload += b'\x00\x00\x00\x00'  # separator
    payload += struct.pack('<i', len(os_name)) + os_name
    payload += struct.pack('<i', len(hostname)) + hostname
    send(sock, build_frame(1, 0, payload))


def read_login_response(sock: socket.socket):
    msg = read_message(sock)
    assert msg['msg_type'] == 163, f"Expected 163, got {msg['msg_type']}"
    data = msg['body']
    # Server name at offset 0x10
    sn_len = struct.unpack_from('<I', data, 0x10)[0]
    sn = data[0x14: 0x14 + sn_len]
    sys.stderr.write(f"  login ok, server_name={sn}\n")


# ---------------------------------------------------------------------------
# Ready + OPE query
# ---------------------------------------------------------------------------

def send_ready(sock: socket.socket):
    """Send READY with no payload (body_len=0)."""
    send(sock, build_frame(3, 0, b''))


def send_ope_query(sock: socket.socket, sql: str):
    """Send OPE(91) with SQL + null terminator."""
    payload = sql.encode('utf-8') + b'\x00'
    send(sock, build_frame(91, 0, payload))


# ---------------------------------------------------------------------------
# Hex dump
# ---------------------------------------------------------------------------

def hex_dump(data: bytes, prefix: str = ''):
    """Print a full hex dump of the data."""
    lines = []
    for offset in range(0, len(data), 16):
        chunk = data[offset: offset + 16]
        hex_part = ' '.join(f'{b:02x}' for b in chunk)
        ascii_part = ''.join(chr(b) if 0x20 <= b < 0x7f else '.' for b in chunk)
        lines.append(f"{prefix}{offset:04x}  {hex_part:<48s}  |{ascii_part}|")
    return '\n'.join(lines)


def annotated_dump(data: bytes):
    """Print a detailed annotated hex dump of the ACK payload."""
    width = 16
    print(f"\n{'='*80}")
    print(f"  ACK PAYLOAD HEX DUMP  ({len(data)} bytes)")
    print(f"{'='*80}\n")

    # First pass: plain hex dump
    print("Raw bytes:")
    print(hex_dump(data, '  '))

    # Second pass: field-by-field annotation
    print(f"\n{'='*80}")
    print("  FIELD-LEVEL ANNOTATION")
    print(f"{'='*80}\n")

    off = 0

    # --- Fixed header (16 bytes) ---
    sub_type = struct.unpack_from('<I', data, off)[0]
    flags = struct.unpack_from('<I', data, off + 4)[0]
    reserved = struct.unpack_from('<I', data, off + 8)[0]
    row_count = struct.unpack_from('<I', data, off + 12)[0]
    print(f"  Offset {off:3d}:  sub_type     = {sub_type}  (0x{sub_type:02x})")
    print(f"  Offset {off+4:3d}:  flags        = {flags}  (0x{flags:08x})")
    print(f"  Offset {off+8:3d}:  reserved     = {reserved}")
    print(f"  Offset {off+12:3d}: row_count    = {row_count}")
    off += 16

    # --- First column header (16 bytes) ---
    col_type = struct.unpack_from('<i', data, off)[0]
    nullable = struct.unpack_from('<H', data, off + 4)[0]
    col_count = struct.unpack_from('<H', data, off + 6)[0]
    name_len = struct.unpack_from('<H', data, off + 8)[0]
    type_len = struct.unpack_from('<H', data, off + 10)[0]
    table_len = struct.unpack_from('<H', data, off + 12)[0]
    schema_len = struct.unpack_from('<H', data, off + 14)[0]

    TYPE_NAMES = {1: 'BIT', 2: 'TINYINT', 3: 'VARCHAR', 4: 'INT',
                  5: 'BIGINT', 6: 'SMALLINT', 7: 'FLOAT', 8: 'DOUBLE',
                  9: 'DECIMAL', 10: 'DATE', 11: 'TIME', 12: 'TIMESTAMP',
                  13: 'BLOB', 14: 'CLOB', 16: 'CHAR'}
    type_str = TYPE_NAMES.get(col_type, f'?({col_type})')

    print(f"\n  First Column Header (offset {off}):")
    print(f"    col_type   = {col_type}  ({type_str})")
    print(f"    nullable   = {nullable}")
    print(f"    col_count  = {col_count}")
    print(f"    name_len   = {name_len}")
    print(f"    type_len   = {type_len}")
    print(f"    table_len  = {table_len}")
    print(f"    schema_len = {schema_len}")
    off += 16

    # --- First column strings ---
    col_name = data[off: off + name_len]
    off += name_len
    type_name = data[off: off + type_len]
    off += type_len
    table_name = data[off: off + table_len]
    off += table_len
    schema_name = data[off: off + schema_len]
    off += schema_len
    print(f"\n  First Column strings:")
    print(f"    name   = {col_name}")
    print(f"    type   = {type_name}")
    print(f"    table  = {table_name}")
    print(f"    schema = {schema_name}")

    # Check for null terminator
    if off < len(data) and data[off] == 0:
        print(f"    null_term at offset {off}")
        off += 1

    # --- Subsequent columns ---
    num_cols_parsed = 1
    max_possible = 32

    while num_cols_parsed < max_possible and off + 4 <= len(data):
        saved = off

        # Between-columns metadata (4 bytes)
        between = data[off: off + 4]
        between_val = struct.unpack_from('<I', data, off)[0]
        print(f"\n  Between-columns gap (offset {off}): {between} (value={between_val})")
        off += 4

        # Column header (24 bytes)
        if off + 24 > len(data):
            off = saved
            break

        c_type = struct.unpack_from('<i', data, off)[0]
        c_nullable = struct.unpack_from('<H', data, off + 4)[0]
        c_display = struct.unpack_from('<H', data, off + 6)[0]
        c_reserved = data[off + 8: off + 12]
        c_index = struct.unpack_from('<H', data, off + 12)[0]
        c_name_len = struct.unpack_from('<H', data, off + 14)[0]
        c_type_len = struct.unpack_from('<H', data, off + 16)[0]
        c_table_len = struct.unpack_from('<H', data, off + 18)[0]
        c_schema_len = struct.unpack_from('<H', data, off + 20)[0]

        c_type_str = TYPE_NAMES.get(c_type, f'?({c_type})')

        # Validate
        if c_name_len < 1 or c_name_len > 128 or c_type_len < 1 or c_type_len > 128:
            off = saved
            break

        c_name_bytes = data[off + 24: off + 24 + c_name_len]
        if not all(0x20 <= b <= 0x7e for b in c_name_bytes):
            off = saved
            break

        print(f"\n  Column {num_cols_parsed + 1} header (offset {off}):")
        print(f"    col_type   = {c_type}  ({c_type_str})")
        print(f"    nullable   = {c_nullable}")
        print(f"    display    = {c_display}")
        print(f"    reserved   = {c_reserved}")
        print(f"    col_index  = {c_index}")
        print(f"    name_len   = {c_name_len}")
        print(f"    type_len   = {c_type_len}")
        print(f"    table_len  = {c_table_len}")
        print(f"    schema_len = {c_schema_len}")

        off += 24

        c_name = data[off: off + c_name_len]
        off += c_name_len
        c_type_n = data[off: off + c_type_len]
        off += c_type_len
        c_table = data[off: off + c_table_len]
        off += c_table_len
        c_schema = data[off: off + c_schema_len]
        off += c_schema_len

        print(f"    name   = {c_name}")
        print(f"    type   = {c_type_n}")
        print(f"    table  = {c_table}")
        print(f"    schema = {c_schema}")

        num_cols_parsed += 1

    # --- Row data ---
    print(f"\n{'='*80}")
    print(f"  ROW DATA STARTS AT OFFSET {off} ({len(data) - off} bytes remaining)")
    print(f"{'='*80}\n")

    col_count = num_cols_parsed
    row_num = 0

    while off < len(data):
        row_start = off
        row_size = data[off]
        if row_size == 0 or row_start + row_size > len(data):
            break

        flags = data[off + 1]
        rec_id = struct.unpack_from('<I', data, off + 2)[0]
        padding = struct.unpack_from('<I', data, off + 6)[0]

        print(f"  Row {row_num} (abs_offset={row_start}, row_size={row_size}):")
        print(f"    flags=0x{flags:02x}  rec_id={rec_id}  padding={padding}")

        # Column offsets table
        offsets_table = row_start + 10
        col_offsets = []
        for ci in range(col_count):
            o = struct.unpack_from('<H', data, offsets_table + ci * 2)[0]
            col_offsets.append(o)
        print(f"    col_offsets (relative): {col_offsets}")

        # Column values
        for ci, coff in enumerate(col_offsets):
            val_abs = row_start + coff
            if val_abs + 2 > len(data):
                print(f"    Col {ci}: offset={coff} (abs={val_abs}) - TRUNCATED")
                continue
            val_size = struct.unpack_from('<H', data, val_abs)[0]
            val_data = data[val_abs + 2: val_abs + 2 + val_size] if val_size > 0 else b''
            try:
                val_text = val_data.decode('utf-8')
            except UnicodeDecodeError:
                val_text = f"<{val_size} bytes>"
            if col_count <= 3:
                col_label = ['ID', 'NAME', 'AGE'][ci] if ci < 3 else f'COL{ci}'
            else:
                col_label = f'COL{ci}'
            print(f"    {col_label} (col {ci}): rel_off={coff:3d} abs={val_abs:3d} "
                  f"size={val_size:2d}  value={val_text!r}  "
                  f"hex={' '.join(f'{b:02x}' for b in val_data)}")

        row_raw = data[row_start: row_start + row_size]
        print(f"    Row raw ({row_size} bytes): {' '.join(f'{b:02x}' for b in row_raw)}")
        print()

        off += row_size
        row_num += 1

    # --- Between-column gap analysis ---
    print(f"\n{'='*80}")
    print(f"  BETWEEN-COLUMN GAP ANALYSIS")
    print(f"{'='*80}\n")

    # Walk through again to measure gaps
    gap_off = 0
    # skip fixed header
    gap_off = 16
    # skip first col header
    gap_off += 16
    # skip first col strings
    gap_off += name_len + type_len + table_len + schema_len
    if gap_off < len(data) and data[gap_off] == 0:
        gap_off += 1  # null terminator

    gaps = []
    temp_off = gap_off
    for ci in range(1, num_cols_parsed):
        if temp_off + 4 > len(data):
            break
        gap_bytes = data[temp_off: temp_off + 4]
        gap_val = struct.unpack_from('<I', data, temp_off)[0]
        gaps.append((ci, temp_off, gap_bytes, gap_val))

        # Skip gap + column header (24 bytes) + column strings
        temp_off += 4  # gap
        if temp_off + 24 > len(data):
            break
        cn = struct.unpack_from('<H', data, temp_off + 14)[0]
        ct = struct.unpack_from('<H', data, temp_off + 16)[0]
        cta = struct.unpack_from('<H', data, temp_off + 18)[0]
        cs = struct.unpack_from('<H', data, temp_off + 20)[0]
        temp_off += 24 + cn + ct + cta + cs

    for ci, go, gb, gv in gaps:
        print(f"  Gap before column {ci} (offset {go}): "
              f"{gb} (u32 LE = {gv})")

    print(f"\n  Total parsed columns: {num_cols_parsed}")
    print(f"  Total parsed rows: {row_num}")
    if off < len(data):
        print(f"  Trailing bytes: {data[off:]}")
    else:
        print(f"  No trailing bytes - exact parse!")
    print()


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main():
    print(f"Connecting to DM at {HOST}:{PORT} ...\n")
    sock = socket.create_connection((HOST, PORT), timeout=10)
    print("  Connected.\n")

    # 1. Startup handshake
    print("[1] STARTUP handshake")
    send_startup(sock)
    challenge = read_startup_response(sock)
    print()

    # 2. Login
    print("[2] LOGIN")
    send_login(sock, challenge)
    read_login_response(sock)
    print()

    # 3. Ready
    print("[3] READY")
    send_ready(sock)
    ready_ack = read_message(sock)
    assert ready_ack['msg_type'] == 187, f"Expected ACK for READY, got {msg_name(ready_ack['msg_type'])}"
    print()

    # 4. OPE query
    query_sql = SQL
    print(f"[4] OPE(91) query: {query_sql}")
    send_ope_query(sock, query_sql)
    ope_ack = read_message(sock)

    if ope_ack['resp_code'] < 0:
        err_body = ope_ack['body']
        print(f"  Query ERROR: msg_type={msg_name(ope_ack['msg_type'])} "
              f"resp_code={ope_ack['resp_code']} body_len={len(err_body)}")
        if len(err_body) >= 16:
            msg_len = struct.unpack_from('<I', err_body, 12)[0]
            if msg_len > 0 and len(err_body) >= 16 + msg_len:
                err_text = err_body[16:16+msg_len].decode('utf-8', errors='replace')
                print(f"  Error message: {err_text}")
        print(f"  Error body hex: {' '.join(f'{b:02x}' for b in err_body[:128])}")
        print("\n  Attempting setup (DROP/CREATE/INSERT) then retrying...\n")

        for setup_sql in [
            "DROP TABLE IF EXISTS RUST_TEST",
            "CREATE TABLE RUST_TEST (ID INT PRIMARY KEY, NAME VARCHAR(100), AGE INT)",
            "INSERT INTO RUST_TEST (ID,NAME,AGE) VALUES (1,'Alice',25)",
            "INSERT INTO RUST_TEST (ID,NAME,AGE) VALUES (2,'Bob',30)",
            "INSERT INTO RUST_TEST (ID,NAME,AGE) VALUES (3,'Charlie',35)",
        ]:
            send_ready(sock)
            read_message(sock)
            send_ope_query(sock, setup_sql)
            r = read_message(sock)
            if r['resp_code'] < 0:
                print(f"  Setup ERROR on '{setup_sql[:60]}': code={r['resp_code']}")
            else:
                print(f"  Setup ok: {setup_sql[:60]}")

        query_sql = "SELECT ID, NAME, AGE FROM RUST_TEST"
        print(f"\n  Retrying with: {query_sql}")
        send_ready(sock)
        read_message(sock)
        send_ope_query(sock, query_sql)
        ope_ack = read_message(sock)

        if ope_ack['resp_code'] < 0:
            print(f"  Retry ERROR: code={ope_ack['resp_code']}")
            sys.exit(1)

    payload = ope_ack['body']
    print(f"  msg_type={msg_name(ope_ack['msg_type'])} body={len(payload)} bytes resp={ope_ack['resp_code']}\n")

    # 5. Full hex dump
    annotated_dump(payload)

    sock.close()
    print("Done.")


if __name__ == '__main__':
    main()
