from typing import Optional

from wit_world.imports import session as session_api, types


class WitWorld:
    def t_current(self) -> types.Session:
        return session_api.current()

    def t_broadcast(self, key: str, value: str) -> None:
        session_api.broadcast(key, value)

    def t_iproto_send(
        self,
        session: types.Session,
        header: bytes,
        body: Optional[bytes],
    ) -> None:
        session_api.iproto_send(session, header, body)
