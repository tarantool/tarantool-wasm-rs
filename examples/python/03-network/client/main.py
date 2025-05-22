from wit_world import exports
from wit_world.imports import say

class IncomingHandler(exports.IncomingHandler):
    def handle(self, request, response_out):
        pass

import socket

def py_socket_client_test():
    HOST = "127.0.0.1"
    PORT = 65432

    messages = [b"Hello, server!", b"Another message", b""]

    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.connect((HOST, PORT))
        print(f"Connected to {HOST}:{PORT}")

        for msg in messages:
            if not msg:
                break
            print(f"Sending: {msg}")
            s.sendall(msg)
            data = s.recv(1024)
            print(f"Received: {data}")


class Run(exports.Run):
    def run(self) -> None:
        try:
            py_socket_client_test()
        except Exception as e:
            say.say_error(str(e), None)
