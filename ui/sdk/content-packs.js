import { decrypt, digest, importKeys, openIndex, packId } from './content-format.js';

const result = (request) =>
  new Promise((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
const committed = (tx) =>
  new Promise((resolve, reject) => {
    tx.oncomplete = resolve;
    tx.onabort = () => reject(tx.error || new Error('Content store transaction aborted.'));
  });
function safeURL(value) {
  const url = new URL(value, document.baseURI);
  if (
    url.protocol !== 'https:' &&
    !(url.protocol === 'http:' && ['127.0.0.1', 'localhost', '[::1]'].includes(url.hostname))
  )
    throw new Error('Use HTTPS for a content feed.');
  return url;
}
async function download(url, signal) {
  const response = await fetch(url, {
    signal,
    redirect: 'error',
    credentials: 'omit',
    cache: 'no-store',
  });
  if (!response.ok) throw new Error('Could not fetch content (' + response.status + ').');
  return response;
}
export class ContentPacks {
  constructor(namespace, clientConfig) {
    this.name = 'rustybox.packs.' + namespace;
    this.keys = importKeys(clientConfig);
    const request = indexedDB.open(this.name, 1);
    request.onupgradeneeded = () => {
      request.result.createObjectStore('active');
      request.result.createObjectStore('blocks');
    };
    this.db = result(request);
  }
  locked(id, kind, mode, operation) {
    if (!packId.test(id)) throw new Error('Invalid content pack ID.');
    return navigator.locks.request(this.name + ':' + id + ':' + kind, { mode }, operation);
  }
  async get(store, key) {
    return result((await this.db).transaction(store).objectStore(store).get(key));
  }
  async put(store, key, value) {
    const tx = (await this.db).transaction(store, 'readwrite');
    tx.objectStore(store).put(value, key);
    await committed(tx);
  }
  async installed(id) {
    const envelope = await this.get('active', id);
    return envelope ? (await openIndex(envelope, await this.keys, id)).version : null;
  }
  // Only verified ciphertext reaches the cache. Publication and obsolete-block cleanup share a transaction.
  async activate(id, envelope, keep) {
    const tx = (await this.db).transaction(['active', 'blocks'], 'readwrite');
    const active = tx.objectStore('active');
    if (envelope) active.put(envelope, id);
    else active.delete(id);
    const blocks = tx.objectStore('blocks');
    const cursor = blocks.openKeyCursor(IDBKeyRange.bound([id, ''], [id, '\uffff']));
    cursor.onsuccess = () => {
      const row = cursor.result;
      if (row) {
        if (!keep.has(row.key[1])) blocks.delete(row.key);
        row.continue();
      }
    };
    await committed(tx);
  }
  async install(id, manifestURL, { signal, onProgress } = {}) {
    return this.locked(id, 'writer', 'exclusive', async () => {
      const url = safeURL(manifestURL),
        keys = await this.keys;
      const envelope = await (await download(url, signal)).json();
      const index = await openIndex(envelope, keys, id);
      const active = await this.get('active', id);
      if (active) {
        const prior = await openIndex(active, keys, id);
        if (
          prior.version > index.version ||
          (prior.version === index.version && active.body !== envelope.body)
        )
          throw new Error('Content version conflicts with the installed pack.');
      }
      let verified = 0;
      const total = index.files.reduce((sum, file) => sum + file.size, 0),
        keep = new Set();
      onProgress?.({ verified, total });
      for (const file of index.files)
        for (const block of file.blocks) {
          signal?.throwIfAborted();
          if (!keep.has(block.hash)) {
            const key = [id, block.hash],
              cached = await this.get('blocks', key);
            if (!cached || (await digest(cached)) !== block.hash) {
              const bytes = await (
                await download(new URL('blocks/' + block.hash + '.rbc', url), signal)
              ).arrayBuffer();
              if (bytes.byteLength !== block.size + 28 || (await digest(bytes)) !== block.hash)
                throw new Error('Content block verification failed.');
              const plain = await decrypt(keys.content, new Uint8Array(bytes));
              if ((await digest(plain)) !== block.plain)
                throw new Error('Content block identity mismatch.');
              await this.put('blocks', key, bytes);
            }
            keep.add(block.hash);
          }
          verified += block.size;
          onProgress?.({ verified, total });
        }
      signal?.throwIfAborted();
      await this.locked(id, 'active', 'exclusive', () => {
        signal?.throwIfAborted();
        return this.activate(id, envelope, keep);
      });
      return index.version;
    });
  }
  async read(id, name, type = 'application/octet-stream') {
    return this.locked(id, 'active', 'shared', async () => {
      const envelope = await this.get('active', id);
      if (!envelope) throw new Error('Content pack is not installed.');
      const keys = await this.keys;
      const index = await openIndex(envelope, keys, id),
        file = index.files.find((item) => item.name === name);
      if (!file) throw new Error('Content file was not found.');
      const parts = [];
      for (const block of file.blocks) {
        const bytes = await this.get('blocks', [id, block.hash]);
        if (!bytes || (await digest(bytes)) !== block.hash)
          throw new Error('Reinstall the pack to repair this block.');
        parts.push(await decrypt(keys.content, new Uint8Array(bytes)));
      }
      return new Blob(parts, { type });
    });
  }
  remove(id) {
    return this.locked(id, 'writer', 'exclusive', () =>
      this.locked(id, 'active', 'exclusive', () => this.activate(id, null, new Set()))
    );
  }
}
