import {instanceNetwork} from 'wasi:sockets/instance-network@0.2.12';
import {resolveAddresses} from 'wasi:sockets/ip-name-lookup@0.2.12';
import {createTcpSocket} from 'wasi:sockets/tcp-create-socket@0.2.12';

function using(handle, fn) {
  try {
    return fn(handle);
  } finally {
    handle[Symbol.dispose]();
  }
}
function wait(fn, pollable) {
  for (;;) {
    try {
      return fn();
    } catch (e) {
      if (e.payload !== 'would-block') {
        throw e;
      }
      pollable.block();
    }
  }
}
export function resolve(name) {
  return using(
      instanceNetwork(),
      (network) => using(
          resolveAddresses(network, name),
          (addresses) => using(
              addresses.subscribe(),
              (pollable) => JSON.stringify(
                  wait(() => addresses.resolveNextAddress(), pollable)))));
}
export function exchange(port) {
  try {
    return using(
        instanceNetwork(),
        (network) => using(createTcpSocket('ipv4'), (socket) => {
          socket.startConnect(
              network, {tag: 'ipv4', val: {address: [127, 0, 0, 1], port}});
          return using(socket.subscribe(), (pollable) => {
            const [input, output] =
                wait(() => socket.finishConnect(), pollable);
            return using(input, (reader) => using(output, (writer) => {
                                  writer.blockingWriteAndFlush(
                                      new TextEncoder().encode('ping\n'));
                                  return new TextDecoder().decode(
                                      reader.blockingRead(64n));
                                }));
          });
        }));
  } catch (error) {
    if (error.name === 'ReferenceError' &&
        error.message === 'import_network_0_2_12$Network is not defined') {
      return 'binding-error: ' + error.message;
    }
    throw error;
  }
}
