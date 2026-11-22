// Publish opaque game data. The app ships client.json; only the publisher holds signing.jwk.
import { createReadStream } from 'node:fs';
import {
  mkdir,
  readdir,
  rmdir,
  readFile,
  writeFile,
  rename,
  open,
  unlink,
  mkdtemp,
  realpath,
} from 'node:fs/promises';
import { dirname, join, relative, resolve, sep } from 'node:path';
import {
  base64,
  digest,
  encrypt,
  importKeys,
  openIndex,
  packId,
  sealIndex,
} from '../ui/sdk/content-format.js';

async function keygen(directory) {
  const dest = resolve(directory);
  await mkdir(dirname(dest), { recursive: true });

  const pair = await crypto.subtle.generateKey({ name: 'ECDSA', namedCurve: 'P-256' }, true, [
    'sign',
    'verify',
  ]);
  const publisher = await crypto.subtle.exportKey('jwk', pair.publicKey);
  const secret = await crypto.subtle.exportKey('jwk', pair.privateKey);
  const client = { publisher, contentKey: base64(crypto.getRandomValues(new Uint8Array(32))) };
  const stage = await mkdtemp(join(dirname(dest), '.rustybox-keys-'));
  try {
    await writeFile(join(stage, 'client.json'), JSON.stringify(client, null, 2));
    await writeFile(join(stage, 'signing.jwk'), JSON.stringify(secret), { mode: 0o600 });
    await rename(stage, dest);
  } catch (error) {
    // A failed rename (for example an existing publisher) must not leave a second signing key.
    for (const file of ['client.json', 'signing.jwk']) {
      await unlink(join(stage, file)).catch((e) => {
        if (e.code !== 'ENOENT') throw e;
      });
    }
    await rmdir(stage);
    throw error;
  }
  console.log('Client configuration and publisher signing key created in', dest);
}
async function* walk(root, directory = root) {
  const entries = (await readdir(directory, { withFileTypes: true })).sort((a, b) =>
    a.name.localeCompare(b.name, 'en')
  );
  for (const entry of entries) {
    if (entry.name.startsWith('.') || entry.name === 'node_modules') continue;
    const path = join(directory, entry.name);
    if (entry.isDirectory()) yield* walk(root, path);
    else if (entry.isFile()) yield { name: relative(root, path).split(sep).join('/'), path };
  }
}
async function existing(path) {
  try {
    return await readFile(path);
  } catch (error) {
    if (error.code === 'ENOENT') return null;
    throw error;
  }
}
async function pack(source, output, id, versionText, directory) {
  const version = Number(versionText);
  if (!packId.test(id) || !Number.isSafeInteger(version) || version < 1)
    throw new Error('Choose a pack ID and a positive integer version.');
  const root = await realpath(source),
    keysDir = await realpath(directory);
  await mkdir(output, { recursive: true });
  const dest = await realpath(output);
  const inside = (path, base) => path === base || path.startsWith(base + sep);
  if (
    [dest, keysDir].some((path) => inside(path, root) || inside(root, path)) ||
    inside(keysDir, dest) ||
    inside(dest, keysDir)
  )
    throw new Error('Source, output and publisher keys must be separate directories.');
  const keys = await importKeys(JSON.parse(await readFile(join(keysDir, 'client.json'), 'utf8')));
  const signer = await crypto.subtle.importKey(
    'jwk',
    JSON.parse(await readFile(join(keysDir, 'signing.jwk'), 'utf8')),
    { name: 'ECDSA', namedCurve: 'P-256' },
    false,
    ['sign']
  );
  const latest = join(dest, id + '.rbcontent'),
    blocks = join(dest, 'blocks');
  await mkdir(blocks, { recursive: true });
  const lockPath = join(dest, '.' + id + '.lock');
  const lock = await open(lockPath, 'wx');
  try {
    const previous = await existing(latest),
      cache = new Map();
    if (previous) {
      const index = await openIndex(JSON.parse(previous), keys, id);
      if (version <= index.version)
        throw new Error('Choose a version newer than the published pack.');
      for (const file of index.files)
        for (const block of file.blocks) cache.set(block.plain, block);
    }
    const files = [];
    for await (const file of walk(root)) {
      const record = { name: file.name, size: 0, blocks: [] };
      for await (const bytes of createReadStream(file.path, { highWaterMark: 1024 * 1024 })) {
        const plain = await digest(bytes);
        let block = cache.get(plain);
        const cached = block && (await existing(join(blocks, block.hash + '.rbc')));
        if (!cached || (await digest(cached)) !== block.hash) {
          const cipher = await encrypt(keys.content, bytes);
          block = { plain, hash: await digest(cipher), size: bytes.length };
          const temp = join(blocks, '.' + crypto.randomUUID());
          await writeFile(temp, cipher);
          await rename(temp, join(blocks, block.hash + '.rbc'));
          cache.set(plain, block);
        }
        record.blocks.push(block);
        record.size += bytes.length;
      }
      files.push(record);
    }
    const envelope = await sealIndex(
      { format: 'rustybox.content.1', id, version, files },
      keys,
      signer
    );
    await openIndex(envelope, keys, id);
    const temp = join(dest, '.' + crypto.randomUUID());
    await writeFile(temp, JSON.stringify(envelope));
    await rename(temp, latest);
    console.log(id, 'version', version, 'published to', latest);
  } finally {
    await lock.close();
    await unlink(lockPath);
  }
}
const [action, ...args] = process.argv.slice(2);
try {
  if (action === 'keygen' && args.length === 1) await keygen(...args);
  else if (action === 'pack' && args.length === 5) await pack(...args);
  else
    throw new Error(
      'Usage: cargo rustybox content keygen <new-key-directory> or content pack <source> <output> <id> <version> <key-directory>'
    );
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}
