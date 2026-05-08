#!/usr/bin/env python3
"""Proxy server that captures wire protocol traffic between client and DM server."""
import socket
import threading
import sys
import os

SERVER_HOST = '127.0.0.1'
SERVER_PORT = 5236
PROXY_PORT = 5237

class Proxy:
    def __init__(self, client, server):
        self.client = client
        self.server = server
    
    def dump(self, label, data):
        print(f'[{label}] ({len(data)} bytes)')
        for i in range(0, min(len(data), 512), 16):
            hex_str = ' '.join(f'{b:02x}' for b in data[i:i+16])
            ascii_str = ''.join(chr(b) if 32<=b<127 else '.' for b in data[i:i+16])
            print(f'  {i:04x}: {hex_str:<48s}  {ascii_str}')
    
    def forward_c2s(self):
        try:
            while True:
                data = self.client.recv(65536)
                if not data:
                    break
                self.dump('C->S', data)
                self.server.sendall(data)
        except Exception as e:
            print(f'[C->S] Error: {e}')
    
    def forward_s2c(self):
        try:
            while True:
                data = self.server.recv(65536)
                if not data:
                    break
                self.dump('S->C', data)
                self.client.sendall(data)
        except Exception as e:
            print(f'[S->C] Error: {e}')

def handle_client(client_sock):
    server_sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    server_sock.settimeout(5)
    server_sock.connect((SERVER_HOST, SERVER_PORT))
    proxy = Proxy(client_sock, server_sock)
    t1 = threading.Thread(target=proxy.forward_c2s, daemon=True)
    t2 = threading.Thread(target=proxy.forward_s2c, daemon=True)
    t1.start()
    t2.start()
    t1.join()
    t2.join()
    client_sock.close()
    server_sock.close()
    print('Connection closed')

if __name__ == '__main__':
    server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    server.bind(('127.0.0.1', PROXY_PORT))
    server.listen(1)
    print(f'Proxy listening on 127.0.0.1:{PROXY_PORT}')
    sys.stdout.flush()
    
    client_sock, addr = server.accept()
    print(f'Client connected from {addr}')
    sys.stdout.flush()
    handle_client(client_sock)
    server.close()
