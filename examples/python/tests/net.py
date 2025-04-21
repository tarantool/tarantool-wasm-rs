from adder import exports
from adder.imports import instance_network, stdout, ip_name_lookup, poll, stdin, tcp_create_socket,  network, tcp
from adder.types import Ok, Err
from adder.imports.wasi_http_types import (
    OutgoingRequest,
    RequestOptions,
    Method_Get,
    Fields,
    OutgoingBody,
    FutureIncomingResponse,
    IncomingResponse
)
from adder.imports import outgoing_handler
from adder.imports import say


def resolve_test():
    _stdin = stdin.get_stdin()
    _stdout = stdout.get_stdout()

    _stdout.blocking_write_and_flush("Domain: ".encode())
    name = _stdin.blocking_read(1024)

    net = instance_network.instance_network()
    s = ip_name_lookup.resolve_addresses(net, name.decode().strip())

    pollable = s.subscribe()
    pollables = [pollable]
    while True:
        # Проверяем готовность хотя бы одного объекта из списка
        ready_handles = poll.poll(pollables)  # poll() ожидает завершения

        if ready_handles:
            # Если объект готов, получаем следующий адрес
            address = s.resolve_next_address()
            if address is None:
                break
            print(f"Resolved address: {address}")
        else:
            # Блокируем выполнение до готовности
            pollable.block()


def connect_test():
    net = instance_network.instance_network()
    peer = network.Ipv4SocketAddress(port=12121, address=(127, 0, 0, 1))
    socket = tcp_create_socket.create_tcp_socket(network.IpAddressFamily.IPV4)
    pollable = socket.subscribe()
    pollables = [pollable]
    socket.start_connect(net, network.IpSocketAddress_Ipv4(peer))
    while True:
        # Проверяем готовность хотя бы одного объекта из списка
        ready_handles = poll.poll(pollables)  # poll() ожидает завершения

        if ready_handles:
            in_s, out_s = socket.finish_connect()
            out_s.blocking_write_and_flush("hello from python wasm\n".encode())
            msg = in_s.blocking_read(1024)
            print(f"Msg from server: {msg}")
            socket.shutdown(tcp.ShutdownType.BOTH)
            break
        else:
            # Блокируем выполнение до готовности
            pollable.block()


def http_test():
    opts = RequestOptions()
    opts.set_connect_timeout(5_000_000_000)
    opts.set_first_byte_timeout(5_000_000_000)
    opts.set_between_bytes_timeout(5_000_000_000)

    headers = Fields.from_list([
        ("User-Agent", b"MyWASIClient/1.0"),
    ])

    req = OutgoingRequest(headers)
    req.set_method(Method_Get())
    req.set_path_with_query("/")  
    req.set_authority("google.com")
    
    body = req.body()
    OutgoingBody.finish(body, None)

    future = outgoing_handler.handle(req, opts)
    pollable = future.subscribe()
    poll.poll([pollable])

    opt = future.get()
    if opt is None:
        return
    elif isinstance(opt, Ok):
        resp = opt.value.value
        print("Статус:", resp.status())
        hdrs = resp.headers().entries()
        print("Headers:")
        for name, val in hdrs:
            print(f"  {name}: {val.decode()}")
        
        inc_body = resp.consume()
        stream = inc_body.stream()
        chunks = []
        while True:
            data = stream.blocking_read(1024)
            if not data:
                break
            chunks.append(data)
        body_bytes = b"".join(chunks)
        print("Body (first 1024 bytes):")
        print(body_bytes[:1024].decode(errors="replace"))

    elif isinstance(opt, Err):
        print("Ошибка HTTP‑клиента:", opt.value)
    else:
        return


class IncomingHandler(exports.IncomingHandler):
    def handle(self, request, response_out):
        print("HTTP HANDLE")

class Run(exports.Run):
    def run(self) -> None:
        try:
            resolve_test()
            http_test()
            connect_test()

        except Exception as e:
            say.say_error(str(e), None)
