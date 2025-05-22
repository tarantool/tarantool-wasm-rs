from wit_world import exports
from wit_world.imports import say

class IncomingHandler(exports.IncomingHandler):
    def handle(self, request, response_out):
        pass
    
import socket

def py_socket_server_test():
    HOST = "127.0.0.1"
    PORT = 65432

    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.bind((HOST, PORT))
        s.listen()
        conn, addr = s.accept()
        with conn:
            print(f"Connected by {addr}")
            while True:
                data = conn.recv(1024)
                if not data:
                    break
                conn.sendall(data)


class Run(exports.Run):
    def run(self) -> None:
        try:
            py_socket_server_test()
        except Exception as e:
            say.say_error(str(e), None)
