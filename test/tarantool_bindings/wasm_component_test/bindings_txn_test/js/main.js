import {
    id,
    isolation,
    makeSynchronous,
    setIsolation,
    begin,
    commit,
    rollback,
} from 'tarantool:tarantool/transaction@0.2.0';
import { write } from 'tarantool:tarantool/log@0.2.0';

function info(message) {
    write('info', message, undefined)
}

export const run = {
    run: function() {
        info(`transaction id before begin: ${id() ?? 'none'}`)

        begin()
        info('transaction.begin()')
        info(`transaction id first: ${id() ?? 'none'}`)
        info(`transaction isolation: ${isolation() ?? 'none'}`)

        makeSynchronous()
        info('transaction.make_synchronous()')

        setIsolation('best-effort')
        info('transaction.set_isolation()')
        info(`transaction isolation after set: ${isolation() ?? 'none'}`)

        commit()
        info('transaction.commit()')

        begin()
        info('transaction.begin() second')
        info(`transaction id second: ${id() ?? 'none'}`)
        rollback()
        info('transaction.rollback()')
        info(`transaction id after rollback: ${id() ?? 'none'}`)
    },
}
