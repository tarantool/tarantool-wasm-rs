import { schemaVersion, spaceByName, indexByName, insert, update, replace, upsert, ttDelete } from 'docs:adder/ttbox@0.1.0';
import { encode, decode } from 'docs:adder/msgpack@0.1.0';
import { sayInfo, sayError } from 'docs:adder/say@0.1.0';

async function _encode(obj) {
    try {
        const jsonString = JSON.stringify(obj);
        const jsonBytes = new TextEncoder().encode(jsonString);
        const encodedMsgPack = await encode(jsonBytes);
        return encodedMsgPack;
    } catch (error) {
        sayError(`JS | Error in encode: ${JSON.stringify(err.payload, null, 2)}`);
        throw error;
    }
}

async function _decode(msgPackBytes) {
    try {
        const decodedJsonBytes = await decode(msgPackBytes);
        const decodedJsonString = new TextDecoder().decode(decodedJsonBytes);
        return JSON.parse(decodedJsonString);
    } catch (error) {
        sayError(`JS | Error in decode: ${JSON.stringify(err.payload, null, 2)}`);
        throw error;
    }
}

function sleep(ms) {
    return new Promise(resolve => setTimeout(resolve, ms));
}

export const run = {
    run: async function() {
        try {
            const version = await schemaVersion();
            sayInfo(`JS | Schema version: ${String(version)}`);

            const spaceName = "test_space";
            const spaceResult = await spaceByName(spaceName);
            sayInfo(`JS | Space: ${String(JSON.stringify(spaceResult))}`);

            const indexName = "primary";
            const indexResult = await indexByName(spaceResult, indexName);
            sayInfo(`JS | Index: ${String(JSON.stringify(indexResult))}`);

            const tupleData = await _encode([1, "bar"]);
            const insertResult = await insert(spaceResult, tupleData);
            sayInfo(`JS | Insert successful, tuple_id: ${insertResult}`);

            await sleep(5000); // Workaround: Possible issue with 'tx' - requires further investigation

            const key = await _encode([1]);
            const ops = await _encode([["=", 2, "new_value"]]);
            
            const updateResult = await update(indexResult, key, ops);
            sayInfo(`JS | Update successful, tuple_id: ${updateResult}`);

        } catch (err) {
            console.error("JS | Exception caught:", err);
            if (typeof err === "object") {
                sayError(`JS | Error: ${JSON.stringify(err.payload, null, 2)}`);
            }
        }      
    }
}
