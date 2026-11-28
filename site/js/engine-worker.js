// The engine in a worker: scenes are made and drawn here, off the page's
// thread, straight onto the canvases the page handed over.
import { Core, createHost } from './engine-core.js';

let host = null;
const early = [];

self.onmessage = (e) => {
  const m = e.data;
  if (m.type === 'init') {
    Core.load(m.url).then(
      (core) => {
        host = createHost(core, (msg, transfer) => self.postMessage(msg, transfer || []));
        self.postMessage({ type: 'ready' });
        for (const q of early.splice(0)) host(q);
      },
      (err) => self.postMessage({ type: 'failed', message: String(err.message || err) }),
    );
  } else if (host) {
    host(m);
  } else {
    early.push(m);
  }
};
