from wit_world.imports import types, session as ss

class WitWorld:
    def t_current(self) -> types.Session:
        return ss.current()

    def t_broadcast(self, key: str, value: str) -> None:
        ss.broadcast(key, value)

    def t_iproto_send(self, session: types.Session, header: bytes, body: bytes) -> None:
        ss.iproto_send(session, header, body)
