import asyncio
from grpclib.server import Server
from grpclib.utils import graceful_exit

from helloworld_pb2 import HelloReply
from helloworld_grpc import GreeterBase

class Greeter(GreeterBase):

    async def SayHello(self, stream):
        request = await stream.recv_message()
        await stream.send_message(
            HelloReply(message=f'Hello, {request.name}!')
        )

async def main():
    server = Server([Greeter()])
    with graceful_exit([server]):
        await server.start('127.0.0.1', 50051)
        print('Serving on 127.0.0.1:50051')
        await server.wait_closed()

if __name__ == '__main__':
    asyncio.run(main())
