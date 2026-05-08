#!/usr/bin/env python3
"""Debug script to capture raw wire protocol traffic from dmPython driver."""
import socket
import struct
import sys
import os

# Try to connect directly with raw socket to see what the server sends back
sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
sock.settimeout(5)
sock.connect(('127.0.0.1', 5236))

# Try to import dmPython to see the real protocol
try:
    import dameng
    print(f"dmPython version: {dameng.version}")
    con = dameng.connect('127.0.0.1:5236', 'SYSDBA', 'SYSDBA')
    cur = con.cursor()
    cur.execute("SELECT 1")
    rows = cur.fetchall()
    print(f"Query result: {rows}")
    con.close()
    print("SUCCESS via dmPython!")
except ImportError:
    print("dmPython not available, trying raw protocol capture...")
except Exception as e:
    print(f"dmPython error: {e}")
finally:
    sock.close()
