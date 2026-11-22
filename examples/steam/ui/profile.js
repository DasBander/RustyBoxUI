import { games } from './catalog.js';
import { rustybox, createPreviewClient } from './sdk/rustybox.js';
import { bindCommands } from './commands.js';

export const client = rustybox.isNative ? rustybox : createPreviewClient();
export const host = bindCommands(client);
export const gameById = new Map(games.map((game) => [game.id, game]));
const key = 'steam.library.v1';
let previewPacks;
let queue = Promise.resolve();
export let profile;

function validPack(pack, id) {
  return (
    pack?.id === id &&
    Array.isArray(pack.channels) &&
    pack.channels.length === 3 &&
    pack.channels.every(
      (c) =>
        c &&
        typeof c.station === 'string' &&
        Number.isInteger(c.frequency) &&
        c.frequency >= 880 &&
        c.frequency <= 1080
    )
  );
}

export async function getPack(id) {
  if (!gameById.has(id)) throw new Error('Unknown game');
  let pack;
  if (client.isNative) pack = await host.demoPack({ id });
  else {
    previewPacks ??= fetch('./packs.json')
      .then((r) => {
        if (!r.ok) throw new Error('Could not read the bundled demo packs');
        return r.json();
      })
      .catch((error) => {
        previewPacks = undefined;
        throw error;
      });
    pack = (await previewPacks).find((p) => p.id === id);
  }
  if (!validPack(pack, id)) throw new Error('The bundled demo pack could not be loaded.');
  return pack;
}

function ids(value) {
  return [...new Set(Array.isArray(value) ? value.filter((id) => gameById.has(id)) : [])];
}
const count = (n) => (Number.isSafeInteger(n) && n >= 0 ? n : 0);
export function playStats(value = {}) {
  return {
    launches: count(value.launches),
    seconds: count(value.seconds),
    last: count(value.last),
    completions: count(value.completions),
    clean: value.clean === true,
  };
}
function restore(saved) {
  const owned = ids(saved.owned);
  const packs = {},
    stats = {};
  for (const id of owned) {
    if (validPack(saved.packs?.[id], id)) packs[id] = saved.packs[id];
    stats[id] = playStats(saved.stats?.[id] || {});
  }
  return {
    name:
      typeof saved.name === 'string' && saved.name.trim()
        ? saved.name.trim().slice(0, 24)
        : 'Player One',
    owned,
    favorites: ids(saved.favorites).filter((id) => owned.includes(id)),
    packs,
    stats,
    events: (Array.isArray(saved.events) ? saved.events : [])
      .filter(
        (e) =>
          e &&
          owned.includes(e.id) &&
          ['install', 'play', 'finish', 'add'].includes(e.kind) &&
          count(e.at)
      )
      .slice(0, 100),
  };
}
export async function initialize() {
  const saved = await client.invoke('storage.get', { key });
  if (saved && Array.isArray(saved.owned)) {
    profile = restore(saved);
    return;
  }
  const initial = await Promise.all(['signal', 'drift', 'afterlight'].map(getPack));
  profile = restore({
    owned: ['afterlight', 'coast', 'drift', 'everwild', 'orbit', 'signal'],
    favorites: ['drift', 'coast'],
    packs: Object.fromEntries(initial.map((p) => [p.id, p])),
  });
  await client.invoke('storage.set', { key, value: profile });
}

// Commit the pack, ownership and history together. A failed save leaves the visible profile intact.
export function change(update) {
  const job = queue.then(async () => {
    const next = structuredClone(profile);
    await update(next);
    await client.invoke('storage.set', { key, value: next });
    profile = next;
  });
  queue = job.catch(() => {});
  return job;
}
export function record(next, kind, id, extra = {}) {
  next.events.unshift({ kind, id, at: Date.now(), ...extra });
  next.events.length = Math.min(next.events.length, 100);
}
export function achievements(id) {
  const s = playStats(profile.stats[id]);
  return [
    { name: 'On the air', detail: 'Start a radio session.', icon: 'radio', earned: s.launches > 0 },
    {
      name: 'Full spectrum',
      detail: 'Lock all three channels.',
      icon: 'wave',
      earned: s.completions > 0,
    },
    { name: 'Clean sweep', detail: 'Finish without a missed lock.', icon: 'bolt', earned: s.clean },
  ];
}
