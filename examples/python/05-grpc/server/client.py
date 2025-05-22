import asyncio
from grpclib.client import Channel

from helloworld_pb2 import HelloRequest
from helloworld_grpc import GreeterStub

async def main():
    async with Channel('127.0.0.1', 50051) as channel:
        stub = GreeterStub(channel)
        reply = await stub.SayHello(HelloRequest(name='Alice'))
        print('Received:', reply.message)

if __name__ == '__main__':
    asyncio.run(main())

