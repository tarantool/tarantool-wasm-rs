import socket

SERVER_IP = "127.0.0.1"
SERVER_PORT = 65432
WELCOME_MESSAGE = "Hello from server!\n"

def main():
    server_socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    server_socket.bind((SERVER_IP, SERVER_PORT))
    server_socket.listen(5)
    print(f"Server listening on {SERVER_IP}:{SERVER_PORT}")

    try:
        while True:
            client_socket, client_address = server_socket.accept()
            print(f"Connection established with {client_address}")

            client_socket.sendall(WELCOME_MESSAGE.encode("utf-8"))

            while True:
                try:
                    data = client_socket.recv(1024)  # Читаем до 1024 байт
                    if not data:
                        print(f"Connection closed by {client_address}")
                        break

                    print(f"Received from {client_address}: {data.decode('utf-8')}")

                    client_socket.sendall(b"Message received\n")
                except ConnectionResetError:
                    print(f"Connection reset by {client_address}")
                    break

            client_socket.close()
    except KeyboardInterrupt:
        print("\nShutting down server...")
    finally:
        server_socket.close()

if __name__ == "__main__":
    main()

