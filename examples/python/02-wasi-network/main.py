from wit_world import exports
from wit_world.imports import instance_network, ip_name_lookup, poll, tcp_create_socket,  network, tcp
from wit_world.types import Ok, Err
from wit_world.imports.wasi_http_types import (
    OutgoingRequest,
    RequestOptions,
    Method_Get,
    Fields,
    OutgoingBody,
)
from wit_world.imports import outgoing_handler
from wit_world.imports import say


class IncomingHandler(exports.IncomingHandler):
    def handle(self, request, response_out):
        pass


def resolve_test():
    name = 'vk.com'

    net = instance_network.instance_network()
    s = ip_name_lookup.resolve_addresses(net, name)

    pollable = s.subscribe()
    pollables = [pollable]
    while True:
        ready_handles = poll.poll(pollables)

        if ready_handles:
            address = s.resolve_next_address()
            if address is None:
                break
            print(f"Resolved address: {address}")
        else:
            pollable.block()


def connect_test():
    net = instance_network.instance_network()
    peer = network.Ipv4SocketAddress(port=12121, address=(127, 0, 0, 1))
    socket = tcp_create_socket.create_tcp_socket(network.IpAddressFamily.IPV4)
    pollable = socket.subscribe()
    pollables = [pollable]
    socket.start_connect(net, network.IpSocketAddress_Ipv4(peer))
    while True:
        ready_handles = poll.poll(pollables)

        if ready_handles:
            in_s, out_s = socket.finish_connect()
            out_s.blocking_write_and_flush("hello from python wasm\n".encode())
            msg = in_s.blocking_read(1024)
            print(f"Msg from server: {msg}")
            socket.shutdown(tcp.ShutdownType.BOTH)
            break
        else:
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
    req.set_authority("vk.com")
    
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
        print("HTTP client error:", opt.value)
    else:
        return


class Run(exports.Run):
    def run(self) -> None:
        try:
            resolve_test()
            http_test()
            connect_test()

        except Exception as e:
            say.say_error(str(e), None)
