import {
    txn,
    txnId,
    txnIsolation,
    txnMakeSync,
    txnSetIsolation,
    begin,
    commit,
    rollback
} from 'tarantool:tarantool/txn@0.1.2';

import { sayInfo } from 'tarantool:tarantool/say@0.1.2';

const isolationToStr = {
    'txn-isolation-default': 'TXN_ISOLATION_DEFAULT',
    'txn-isolation-best-effort': 'TXN_ISOLATION_BEST_EFFORT',
    'txn-isolation-read-committed': 'TXN_ISOLATION_READ_COMMITTED',
    'txn-isolation-read-confirmed': 'TXN_ISOLATION_READ_CONFIRMED',
    'txn-isolation-linearizable': 'TXN_ISOLATION_LINEARIZABLE',
};

function boolStr(value) {
    return value ? 'True' : 'False';
}

export const run = {
    run: async function() {
        sayInfo(`txn() before begin: ${boolStr(txn())}`);

        await begin();
        sayInfo("txn.begin()");

        const id1 = txnId();
        const iso1 = txnIsolation();
        sayInfo(`txn_id first: ${id1}`);
        sayInfo(`txn_isolation: ${isolationToStr[iso1] ?? `UNKNOWN(${iso1})`}`);

        txnMakeSync();
        sayInfo("txn_make_sync()");

        await txnSetIsolation('txn-isolation-best-effort');
        const iso2 = txnIsolation();
        sayInfo(`txn_isolation after set: ${isolationToStr[iso2] ?? `UNKNOWN(${iso2})`}`);

        await commit();
        sayInfo("txn.commit()");

        await begin();
        sayInfo("txn.begin() second");
        
        const id2 = txnId();
        sayInfo(`txn_id second: ${id2}`);

        await rollback();
        sayInfo("txn.rollback()");

        sayInfo(`txn() after rollback: ${boolStr(txn())}`);
    }
};
