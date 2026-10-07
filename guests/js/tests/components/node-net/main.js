import {createConnection} from 'node:net';
export function exchange(port) {
  return new Promise((resolve, reject) => {
    const socket = createConnection(
        {host: '127.0.0.1', port}, () => socket.write('ping\n'));
    socket.once('error', reject);
    socket.once('data', (data) => {
      socket.destroy();
      resolve(data.toString());
    });
  });
}
