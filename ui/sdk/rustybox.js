/** RustyBox bridge v1. Native commands are asynchronous and origin-bound. */
export class RustyBoxClient extends EventTarget {
  #pending = new Map();
  #next = 1;
  #transport;
  // Request IDs restart on reload. The session keeps a late native reply tied to its document.
  #session = crypto.randomUUID();
  constructor(transport = globalThis.chrome?.webview) {
    super();
    this.#transport = transport;
    this.isNative = Boolean(transport);
    transport?.addEventListener('message', ({ data }) => this.#receive(data));
    globalThis.addEventListener?.('pagehide', () => this.dispose());
  }
  #receive(data) {
    if (!data || data.v !== 1) return;
    if (data.event) {
      this.dispatchEvent(new CustomEvent(data.event, { detail: data.data }));
      return;
    }
    if (data.session !== this.#session) return;
    const pending = this.#pending.get(data.id);
    if (!pending) return;
    pending.cleanup();
    this.#pending.delete(data.id);
    if (data.ok) pending.resolve(data.result);
    else
      pending.reject(
        Object.assign(new Error(data.error?.message || 'Native command failed'), {
          code: data.error?.code,
        })
      );
  }
  invoke(method, params = {}, { signal, timeout = 15000 } = {}) {
    if (signal?.aborted) return Promise.reject(new DOMException('Aborted', 'AbortError'));
    if (!this.#transport)
      return Promise.reject(new Error('This operation requires the RustyBox desktop host.'));
    if (this.#pending.size >= 128)
      return Promise.reject(new Error('Too many pending native requests'));
    const id = this.#next++;
    return new Promise((resolve, reject) => {
      let timer;
      const cleanup = () => {
        clearTimeout(timer);
        signal?.removeEventListener('abort', abort);
      };
      const abort = () => {
        cleanup();
        this.#pending.delete(id);
        reject(new DOMException('Aborted', 'AbortError'));
      };
      timer = setTimeout(() => {
        cleanup();
        this.#pending.delete(id);
        reject(new Error(`Native request timed out: ${method}`));
      }, timeout);
      signal?.addEventListener('abort', abort, { once: true });
      this.#pending.set(id, { resolve, reject, cleanup });
      try {
        const request = { v: 1, id, session: this.#session, method, params };
        if (new TextEncoder().encode(JSON.stringify(request)).length > 256 * 1024)
          throw new Error('Message exceeds 256 KiB');
        this.#transport.postMessage(request);
      } catch (error) {
        cleanup();
        this.#pending.delete(id);
        reject(error);
      }
    });
  }
  dispose() {
    for (const pending of this.#pending.values()) {
      pending.cleanup();
      pending.reject(new Error('View closed'));
    }
    this.#pending.clear();
  }
}
export const rustybox = new RustyBoxClient();

// Explicit preview transport: never claims to be native and never renders a fake D3D scene.
export function createPreviewClient() {
  return {
    isNative: false,
    async invoke(method, params = {}) {
      if (method === 'runtime.info')
        return {
          name: 'RustyBoxUI',
          version: '0.7.0',
          engine: 'Browser preview',
          platform: 'browser',
          graphicsAvailable: false,
          abi: 1,
        };
      if (method === 'window.fullscreen') {
        if ('enabled' in params) {
          if (typeof params.enabled !== 'boolean') throw new TypeError('enabled must be a boolean');
          if (params.enabled && !document.fullscreenElement)
            await document.documentElement.requestFullscreen();
          if (!params.enabled && document.fullscreenElement) await document.exitFullscreen();
        }
        return {
          supported: Boolean(document.fullscreenEnabled),
          enabled: Boolean(document.fullscreenElement),
        };
      }
      if (method === 'diagnostics.echo') return params;
      if (method === 'storage.get')
        return JSON.parse(localStorage.getItem(`rustybox.${params.key}`) || 'null');
      if (method === 'storage.set') {
        localStorage.setItem(`rustybox.${params.key}`, JSON.stringify(params.value));
        return params.value;
      }
      if (method === 'graphics.info' || method === 'graphics.detach') return { active: false };
      throw new Error('Open the Windows application to use native features.');
    },
  };
}
