from wit_world import exports

import asyncio
from grpclib.server import Server

from helloworld_grpc import GreeterBase
from helloworld_pb2 import HelloReply

class Greeter(GreeterBase):
    async def SayHello(self, stream):
        req = await stream.recv_message()
        await stream.send_message(HelloReply(message=f'Hello, {req.name}!'))

async def serve(host: str = '0.0.0.0', port: int = 50051):
    server = Server([Greeter()])
    await server.start(host, port)
    print(f'Serving on {host}:{port}')

    try:
        await asyncio.Future()
    except asyncio.CancelledError:
        pass
    finally:
        print('Shutting down gRPC server...')
        await server.stop(grace=None)
        print('Server stopped.')


class IncomingHandler(exports.IncomingHandler):
    def handle(self, request, response_out):
        pass


class Run(exports.Run):
    def run(self) -> None:
        asyncio.run(serve())


