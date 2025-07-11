export function tBool(x) {
    return x;
}

export function tS8(x) {
    return x;
}

export function tS16(x) {
    return x;
}

export function tS32(x) {
    return x;
}

export function tS64(x) {
    console.log(x);
    return BigInt.asIntN(64, x);
}

export function tU8(x) {
    return x;
}

export function tU16(x) {
    return x;
}

export function tU32(x) {
    console.log(x);
    return x;
}

export function tU64(x) {
    return x;
}

export function tF32(x) {
    return x;
}

export function tF64(x) {
    return x;
}

export function tChar(x) {
    return x;
}

export function tString(x) {
    return x;
}

export function tList(x) {
    return x;
}

export function tOption(x) {
    return x;
}

export function tResult(x) {
    if (!x) {
        throw new Error("Error: result was false");
    }
    return x;
}

export function tTuple(x) {
    return [x[0], x[2]];
}

export function tRecord(x) {
    return x;
}

export function tVariant(x) {
    return x;
}

export function tEnum(x) {
    return x;
}

export function tFlags(x) {
    return x;
}
