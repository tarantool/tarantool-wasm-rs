import { addBase, labelBase } from 'demo:cross-call/coremath@0.1.0';
import { sayInfo } from 'tarantool:tarantool/say@0.1.2';

export const math = {
    add(left, right) {
        sayInfo(`js provider: add(${left}, ${right})`, null);
        return addBase(left, right);
    },

    label(left, right) {
        sayInfo(`js provider: label(${left}, ${right})`, null);
        return `js wrapped [${labelBase(left, right)}]`;
    },
};
