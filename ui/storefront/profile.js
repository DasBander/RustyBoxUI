export const defaultProfile = {
  wishlist: [],
  library: ['afterlight', 'drift', 'coast'],
  installed: ['drift'],
  accent: 'blue',
  reducedMotion: false,
  compact: false,
};

export function validState(saved, byId) {
  if (!saved || typeof saved !== 'object') return {};
  const result = {};
  // A saved library can outlive a catalog revision. Keep known game IDs in their saved order.
  for (const key of ['library', 'installed', 'wishlist']) {
    if (Array.isArray(saved[key])) result[key] = [...new Set(saved[key].filter(byId))];
  }
  if (['gold', 'mint', 'blue'].includes(saved.accent)) result.accent = saved.accent;
  for (const key of ['reducedMotion', 'compact']) {
    if (typeof saved[key] === 'boolean') result[key] = saved[key];
  }
  return result;
}

let saving = Promise.resolve();
export function writeProfile(client, state) {
  // Capture this edit now and queue the write behind earlier edits, even after a failed write.
  const snapshot = structuredClone(state);
  saving = saving
    .catch(() => {})
    .then(() => client.invoke('storage.set', { key: 'showcase.state', value: snapshot }));
  return saving;
}
