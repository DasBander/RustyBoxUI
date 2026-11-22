// The source Blob and its initial edit are committed together. Small subsequent edits
// are journalled synchronously so closing the window immediately after a drag is safe.
export class DocumentDraft {
  constructor(name) {
    this.name = name;
    this.database = new Promise((resolve, reject) => {
      const request = indexedDB.open('rustybox.documents.' + name, 1);
      request.onupgradeneeded = () => request.result.createObjectStore('documents');
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => reject(request.error);
    });
  }
  async read() {
    const db = await this.database;
    const record = await new Promise((resolve, reject) => {
      const request = db.transaction('documents').objectStore('documents').get('active');
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => reject(request.error);
    });
    if (!record) return null;
    const saved = localStorage.getItem(this.key(record.id));
    if (saved !== null) record.edit = JSON.parse(saved);
    this.id = record.id;
    return record;
  }
  key(id) {
    return 'rustybox.edit.' + this.name + '.' + id;
  }
  async replace(name, source, edit) {
    const db = await this.database;
    const id = crypto.randomUUID();
    await new Promise((resolve, reject) => {
      const tx = db.transaction('documents', 'readwrite', { durability: 'strict' });
      tx.objectStore('documents').put({ id, name, source, edit }, 'active');
      tx.oncomplete = resolve;
      tx.onabort = () => reject(tx.error || new Error('Could not store the document.'));
    });
    const previous = this.id;
    this.id = id;
    if (previous) localStorage.removeItem(this.key(previous));
  }
  update(edit) {
    if (!this.id) throw new Error('The source has not been saved for recovery.');
    localStorage.setItem(this.key(this.id), JSON.stringify(edit));
  }
}

// Native saves use a host-owned dialog and a sibling temporary file. Browser previews download a Blob.
export async function saveDocument(client, blob, name) {
  if (!client.isNative) {
    const url = URL.createObjectURL(blob);
    const link = Object.assign(document.createElement('a'), { href: url, download: name });
    link.click();
    setTimeout(() => URL.revokeObjectURL(url), 60000);
    return true;
  }
  const invoke = (args, options) => client.invoke('host.saveDocument', args, options);
  const id = await invoke(
    { operation: 'begin', name, size: blob.size },
    { timeout: 24 * 60 * 60 * 1000 }
  );
  if (id === null) return false;
  try {
    // Leave ample space for the JSON envelope inside one bridge message.
    const chunkSize = 48 * 1024;
    for (let offset = 0; offset < blob.size; offset += chunkSize) {
      const bytes = new Uint8Array(await blob.slice(offset, offset + chunkSize).arrayBuffer());
      const encoded = btoa(Array.from(bytes, (byte) => String.fromCharCode(byte)).join(''));
      await invoke({ operation: 'write', id, offset, bytes: encoded });
    }
    await invoke({ operation: 'finish', id });
    return true;
  } catch (error) {
    await invoke({ operation: 'cancel', id }).catch(() => {});
    throw error;
  }
}
