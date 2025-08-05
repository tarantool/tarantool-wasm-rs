import sys
import os

class WitWorld:

    def t_stdout(self, msg: str) -> None:
        print(msg)

    def t_stdin(self) -> None:
        value = input()
        print(f"read: {value}")

    def t_stderr(self) -> None:
        print("error!", file=sys.stderr)

    def t_env(self) -> str:
        return os.getenv("TEST_VAR", "TEST_VAR IS NONE")

    def t_inherit_env(self) -> str:
        return os.getenv("HOME", "HOME IS NONE")

    def t_preopen(self) -> None:
        try:
            with open("./ro/f1.txt") as f:
                print("./ro/f1.txt RO:", f.read())
        except Exception as e:
            print("RO FAIL-1:", e)

        try:
            with open("./ro/f1.txt", "w") as f:
                print("./ro/f1.txt RW:", f.read())
        except Exception as e:
            print("RO FAIL-2:", e)

        try:
            with open("./rw/f2.txt", "w") as f:
                f.write("./rw/f2.txt OK")
        except Exception as e:
            print("RW FAIL-3:", e)

    def t_args(self) -> None:
        print(sys.argv)
