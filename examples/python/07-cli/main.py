from wit_world import exports
import wit_world.imports as wit
from wit_world.imports import wlua, txn

import code


class IncomingHandler(exports.IncomingHandler):
    def handle(self, request, response_out):
        pass


class Run(exports.Run):
    def run(self) -> None:
        code.interact(local=globals())
