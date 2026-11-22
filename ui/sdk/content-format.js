// Signed envelopes contain an encrypted directory. Each encrypted data block has its own random nonce.
const encoder = new TextEncoder();
export const packId = /^[a-z0-9][a-z0-9-]{0,79}$/;
const digestPattern = /^[a-f0-9]{64}$/;
export function base64(bytes) {
  let binary = '';
  for (let i = 0; i < bytes.length; i += 8192)
    binary += String.fromCharCode(...bytes.subarray(i, i + 8192));
  return btoa(binary);
}
export const unbase64 = (value) => Uint8Array.from(atob(value), (ch) => ch.charCodeAt(0));
export async function digest(bytes) {
  const hash = new Uint8Array(await crypto.subtle.digest('SHA-256', bytes));
  return Array.from(hash, (n) => n.toString(16).padStart(2, '0')).join('');
}
export async function importKeys(config) {
  const verify = await crypto.subtle.importKey(
    'jwk',
    config.publisher,
    { name: 'ECDSA', namedCurve: 'P-256' },
    false,
    ['verify']
  );
  const content = await crypto.subtle.importKey(
    'raw',
    unbase64(config.contentKey),
    'AES-GCM',
    false,
    ['encrypt', 'decrypt']
  );
  return { verify, content };
}
export async function encrypt(key, bytes) {
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const cipher = new Uint8Array(await crypto.subtle.encrypt({ name: 'AES-GCM', iv }, key, bytes));
  const block = new Uint8Array(iv.length + cipher.length);
  block.set(iv);
  block.set(cipher, 12);
  return block;
}
export function decrypt(key, bytes) {
  return crypto.subtle.decrypt(
    { name: 'AES-GCM', iv: bytes.subarray(0, 12) },
    key,
    bytes.subarray(12)
  );
}
export async function sealIndex(index, keys, signer) {
  const bytes = await encrypt(keys.content, encoder.encode(JSON.stringify(index)));
  const signature = await crypto.subtle.sign({ name: 'ECDSA', hash: 'SHA-256' }, signer, bytes);
  return { body: base64(bytes), signature: base64(new Uint8Array(signature)) };
}
export async function openIndex(envelope, keys, expectedId) {
  const bytes = unbase64(envelope.body);
  if (
    !(await crypto.subtle.verify(
      { name: 'ECDSA', hash: 'SHA-256' },
      keys.verify,
      unbase64(envelope.signature),
      bytes
    ))
  )
    throw new Error('Content publisher signature is invalid.');
  const index = JSON.parse(new TextDecoder().decode(await decrypt(keys.content, bytes)));
  if (
    index.format !== 'rustybox.content.1' ||
    !packId.test(expectedId) ||
    index.id !== expectedId ||
    !Number.isSafeInteger(index.version) ||
    index.version < 1 ||
    !Array.isArray(index.files)
  )
    throw new Error('Invalid content directory.');
  const names = new Set();
  for (const file of index.files) {
    if (
      typeof file.name !== 'string' ||
      file.name.includes('\\') ||
      file.name.split('/').some((name) => !name || name === '.' || name === '..') ||
      names.has(file.name) ||
      !Array.isArray(file.blocks)
    )
      throw new Error('Invalid content file.');
    names.add(file.name);
    let size = 0;
    for (const block of file.blocks) {
      if (
        !digestPattern.test(block.hash) ||
        !digestPattern.test(block.plain) ||
        !Number.isSafeInteger(block.size) ||
        block.size <= 0
      )
        throw new Error('Invalid content block.');
      size += block.size;
    }
    if (!Number.isSafeInteger(size) || size !== file.size)
      throw new Error('Content file size mismatch.');
  }
  return index;
}
