from wit_world import exports
import asyncio


async def task1():
    print("Starting task 1...")
    await asyncio.sleep(2)  # Simulates an I/O-bound task
    print("Task 1 completed after 2 seconds")


async def task2():
    print("Starting task 2...")
    await asyncio.sleep(1)
    print("Task 2 completed after 1 second")


async def main():
    await task1()
    await task2()


class IncomingHandler(exports.IncomingHandler):
    def handle(self, request, response_out):
        pass


class Run(exports.Run):
    def run(self) -> None:
        asyncio.run(main())
