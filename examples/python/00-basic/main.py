from wit_world import exports

class IncomingHandler(exports.IncomingHandler):
    def handle(self, request, response_out):
        pass

class Run(exports.Run):
    def run(self) -> None:
        print("Hello from Python WASM!")
