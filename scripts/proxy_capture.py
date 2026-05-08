#!/usr/bin/env python3
"""Capture raw wire protocol from Python dmPython driver using a proxy."""
import socket
import threading
import struct
import sys

# Start a local proxy that forwards to DM server and captures traffic
SERVER_HOST = '127.0.0.1'
SERVER_PORT = 5236
PROXY_PORT = 5237

class Proxy:
    def __init__(self, client_sock, server_sock):
        self.client = client_sock
        self.server = server_sock
    
    def forward_c2s(self):
        """Client -> Server"""
        while True:
            data = self.client.recv(65536)
            if not data:
                break
            print(f"[C->S] ({len(data)} bytes): {data[:200].hex()}")
            self.server.sendall(data)
    
    def forward_s2c(self):
        """Server -> Client"""
        while True:
            data = self.server.recv(65536)
            if not data:
                break
            print(f"[S->C] ({len(data)} bytes): {data[:200].hex()}")
            self.client.sendall(data)

def handle_client(client_sock):
    server_sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    server_sock.settimeout(5)
    server_sock.connect((SERVER_HOST, SERVER_PORT))
    
    proxy = Proxy(client_sock, server_sock)
    
    t1 = threading.Thread(target=proxy.forward_c2s)
    t2 = threading.Thread(target=proxy.forward_s2c)
    t1.start()
    t2.start()
    
    t1.join()
    t2.join()
    
    client_sock.close()
    server_sock.close()

def main():
    server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    server.bind(('127.0.0.1', PROXY_PORT))
    server.listen(1)
    print(f"Proxy listening on port {PROXY_PORT}")
    print("Connect your Python dmPython driver to 127.0.0.1:{0} instead of port 5236".format(PROXY_PORT))
    
    client_sock, addr = server.accept()
    print(f"Client connected from {addr}")
    handle_client(client_sock)
    server.close()

if __name__ == '__main__':
    main()
