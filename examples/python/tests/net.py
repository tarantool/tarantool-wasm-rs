from adder import exports
from adder.imports import instance_network, stdout, ip_name_lookup, poll, stdin
from adder.imports import say


def resolve_test():
    _stdin = stdin.get_stdin()
    _stdout = stdout.get_stdout()

    _stdout.blocking_write_and_flush("Domain: ".encode())
    name = _stdin.blocking_read(1024)

    net = instance_network.instance_network()
    s = ip_name_lookup.resolve_addresses(net, name.decode().strip())

    pollable = s.subscribe()
    pollables = [pollable]
    while True:
        # Проверяем готовность хотя бы одного объекта из списка
        ready_handles = poll.poll(pollables)  # poll() ожидает завершения

        if ready_handles:
            # Если объект готов, получаем следующий адрес
            address = s.resolve_next_address()
            if address is None:
                break
            print(f"Resolved address: {address}")
        else:
            # Блокируем выполнение до готовности
            pollable.block()


class Run(exports.Run):
    def run(self) -> None:
        try:
            resolve_test()

        except Exception as e:
            say.say_error(str(e))
