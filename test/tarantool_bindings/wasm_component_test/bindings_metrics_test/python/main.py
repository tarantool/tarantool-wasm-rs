from wit_world import exports
from wit_world.imports.metrics import uptime
from time import sleep

class Run(exports.Run):
    def run(self) -> None:
        print(uptime())
        sleep(1)
        print(uptime())
