from wit_world import exports
from time import sleep
import os

class Run(exports.Run):
    def run(self) -> None:
        inf = bool(os.getenv("INF"))
        while inf:
            print(1)
            sleep(0.1)
        print(2)
