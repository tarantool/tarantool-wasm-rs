import socket
import sys
from wit_world import exports

# Ensure idna encoding is pre-loaded for WASM
"".encode('idna')

class WitWorld:
    def foo(self, host: str, port: int) -> bool:
        try:
            with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
                s.connect((host, port))
                return True
        except:
            return False

class Run(exports.Run):
     def run(self) -> None:
        try:
            host = sys.argv[0]
            port = sys.argv[1]

            with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
                print(f"Start server {host}:{port}")
                s.settimeout(10)
                s.bind((host, int(port)))
                s.listen()
                conn, addr = s.accept()
                with conn:
                    print(f"Connected by {addr}")

        except Exception as e:
            print(e)
