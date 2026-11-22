import { rustybox as client } from './sdk/rustybox.js';
import './sdk/titlebar.js';
import './sdk/desktop.js';
import { bindActions } from './sdk/actions.js';
import { projectBrowser } from './project-browser.js';
import { onCommittedInput } from './sdk/forms.js';
import { icon } from './sdk/components.js';
import { bindCommands } from './commands.js';

const $ = (s) => document.querySelector(s);
const commands = bindCommands(client);
const viewport = $('rb-native-view');
const reference = $('#reference');
const browser = projectBrowser(commands, selectAsset, report);
let selectionRevision = 0;
let selecting = false;
let workspace,
  comparing = false,
  projectMode = false,
  syncing = false,
  refreshPending = false;
const channelMenu = $('#channels');
const channels = {
  rgb: 'RGB channels',
  red: 'Red channel',
  green: 'Green channel',
  blue: 'Blue channel',
  alpha: 'Alpha channel',
};
let catalog = [],
  selected,
  mode = 'fill',
  zoom = 1,
  channel = 'rgb',
  motion = false;
let initialized = false,
  busy = false;
function announce(message, error = false) {
  $('#notice').textContent = message;
  $('#notice').classList.toggle('error', error);
}
function report(error) {
  announce(error.message || String(error), true);
}
function bytes(value) {
  return value >= 1048576
    ? (value / 1048576).toFixed(2) + ' MiB'
    : (value / 1024).toFixed(0) + ' KiB';
}
function showCatalog() {
  const filter = $('#filter').value.trim().toLowerCase();
  if (projectMode) {
    browser.search(filter);
    return;
  }
  const fragment = document.createDocumentFragment();
  for (const a of catalog.filter((a) =>
    (a.title + ' ' + a.fileName).toLowerCase().includes(filter)
  )) {
    const card = document.createElement('button');
    card.className = 'asset';
    card.type = 'button';
    card.dataset.id = a.id;
    card.setAttribute('aria-pressed', String(selected?.id === a.id));
    const thumb = document.createElement(a.thumbnail ? 'img' : 'span');
    thumb.className = 'thumb';
    if (a.thumbnail) {
      thumb.src = a.thumbnail;
      thumb.alt = '';
    } else {
      thumb.textContent = '▧';
    }
    const meta = document.createElement('span'),
      title = document.createElement('strong'),
      location = document.createElement('small');
    title.textContent = a.title;
    location.textContent = a.location;
    meta.append(title, location);
    card.append(thumb, meta);
    fragment.append(card);
  }
  if (!fragment.childElementCount) {
    const empty = document.createElement('p');
    empty.className = 'empty';
    empty.textContent = 'No matching assets.';
    fragment.append(empty);
  }
  $('#assets').replaceChildren(fragment);
  $('#count').textContent = catalog.length + (catalog.length === 1 ? ' asset' : ' assets');
}
function inspect(a) {
  selected = a;
  $('#title').textContent = a.title;
  $('#file-chip').textContent = a.fileName;
  $('#location').textContent = a.location;
  $('#dimensions').textContent = a.width + ' × ' + a.height;
  $('#size').textContent = bytes(a.bytes);
  $('#format').textContent = a.fileName.split('.').at(-1).toUpperCase().replace('JPG', 'JPEG');
  $('#credit').textContent = a.credit;
  viewport.setAttribute('aria-label', 'Texture preview of ' + a.title);
  if (projectMode) browser.select(a.id);
  else showCatalog();
}
function fitZoom() {
  const rect = viewport.getBoundingClientRect();
  const x = rect.width / selected.width,
    y = rect.height / selected.height;
  return Math.min(8, Math.max(x, y) / Math.min(x, y));
}
async function applyView() {
  if (!selected) return;
  if (mode === 'fill') zoom = fitZoom();
  if (mode === 'fit') zoom = 1;
  await client.invoke('graphics.view', { zoom, channel, motion });
  $('#zoom').textContent = Math.round(zoom * 100) + '%';
  for (const m of ['fit', 'fill']) $('#' + m).setAttribute('aria-pressed', String(mode === m));
  $('#channel-name').textContent = channels[channel];
  for (const button of document.querySelectorAll('[data-channel]')) {
    button.setAttribute(
      button.parentElement.id === 'swatches' ? 'aria-pressed' : 'aria-checked',
      String(button.dataset.channel === channel)
    );
  }
  $('#motion').setAttribute('aria-pressed', String(motion));
  $('#motion').firstElementChild.innerHTML = icon(motion ? 'pause' : 'play', 13);
  $('#motion').lastElementChild.textContent = motion ? 'Pause pan' : 'Pan preview';
}
async function selectAsset(id) {
  const revision = ++selectionRevision;
  selecting = true;
  try {
    const a = await commands.selectAsset({ id });
    if (revision !== selectionRevision) return;
    inspect(a);
    await applyView();
    if (comparing && revision === selectionRevision) await paintReference();
    if (revision === selectionRevision) announce(a.fileName);
  } finally {
    if (revision === selectionRevision) selecting = false;
  }
}
async function importAsset() {
  if (busy || !initialized) return;
  busy = true;
  $('#import').disabled = true;
  try {
    const a = await commands.importImage({}, { timeout: 600000 });
    if (!a) {
      announce('Import canceled');
      return;
    }
    await syncWorkspace();
    $('#filter').value = '';
    inspect(a);
    await applyView();
    announce('Imported ' + a.fileName);
  } finally {
    busy = false;
    $('#import').disabled = false;
  }
}
for (const [value, text] of Object.entries(channels)) {
  const menuItem = document.createElement('button');
  menuItem.textContent = text;
  menuItem.dataset.value = value;
  menuItem.dataset.channel = value;
  menuItem.tabIndex = -1;
  menuItem.setAttribute('aria-label', text);
  menuItem.setAttribute('role', 'menuitemradio');
  menuItem.setAttribute('aria-checked', String(value === channel));
  channelMenu.panel.append(menuItem);
  const swatch = document.createElement('button');
  swatch.textContent = value === 'rgb' ? 'RGB' : value[0].toUpperCase();
  swatch.setAttribute('aria-label', text);
  swatch.dataset.channel = value;
  swatch.setAttribute('aria-pressed', String(value === channel));
  $('#swatches').append(swatch);
}
function switchChannel(value) {
  channel = value;
  return applyView();
}
channelMenu.addEventListener('rb-select', (e) => switchChannel(e.detail).catch(report));
$('#swatches').addEventListener('click', (e) => {
  const b = e.target.closest('button');
  if (b) switchChannel(b.dataset.channel).catch(report);
});
$('#assets').addEventListener('click', (e) => {
  const b = e.target.closest('button');
  if (b) selectAsset(b.dataset.id).catch(report);
});
onCommittedInput($('#filter'), showCatalog);
$('#import').addEventListener('click', () => importAsset().catch(report));
$('#details').addEventListener('click', () => {
  if (!selected) return;
  $('#detail-title').textContent = selected.title;
  $('#detail-copy').textContent = [
    selected.fileName,
    selected.width + ' × ' + selected.height + ' pixels / ' + bytes(selected.bytes),
    selected.location,
    selected.credit,
  ].join('\n');
  $('#detail').open();
});
for (const id of ['fill', 'fit'])
  $('#' + id).addEventListener('click', () => {
    mode = id;
    applyView().catch(report);
  });
for (const [id, factor] of [
  ['zoom-in', 1.25],
  ['zoom-out', 0.8],
])
  $('#' + id).addEventListener('click', () => {
    mode = 'custom';
    zoom = Math.max(0.25, Math.min(8, zoom * factor));
    applyView().catch(report);
  });
$('#motion').addEventListener('click', () => {
  motion = !motion;
  applyView().catch(report);
});
document.addEventListener('keydown', (e) => {
  if (e.isComposing || e.keyCode === 229) return;
  if (e.ctrlKey && !e.shiftKey && !e.altKey && !e.metaKey && e.key.toLowerCase() === 'o') {
    e.preventDefault();
    importAsset().catch(report);
  }
  if (e.key === '/' && e.target.tagName !== 'INPUT' && !$('#detail').dialog.open) {
    e.preventDefault();
    $('#filter').focus();
  }
});
for (const el of document.querySelectorAll('[data-icon]')) el.innerHTML = icon(el.dataset.icon, 15);
for (const b of document.querySelectorAll('[data-window]'))
  b.onclick = () => client.invoke('window.' + b.dataset.window).catch(report);

viewport.addEventListener('native-error', (e) => report(e.detail));
viewport.addEventListener('native-ready', (e) => {
  $('#device').textContent = e.detail.hardware ? 'Hardware GPU' : 'WARP software device';
  $('#render-status').textContent = e.detail.composed
    ? 'Direct3D 11 / DirectComposition'
    : 'Direct3D 11';
  if (initialized && selected) {
    const update = e.detail.source === 'hostTexture' ? applyView() : selectAsset(selected.id);
    update.catch(report);
  }
});
// The reference has its own device and view state. Channel and zoom changes affect the primary only.
async function paintReference() {
  if (!comparing || !selected) return;
  await commands.renderAsset({ id: selected.id, viewport: 1 });
  await client.invoke('graphics.view', { viewport: 1, zoom: 1, channel: 'rgb', motion: false });
}
reference.addEventListener('native-error', (e) => report(e.detail));
reference.addEventListener('native-ready', (e) => {
  if (e.detail.source !== 'hostTexture') paintReference().catch(report);
});
$('#compare').addEventListener('click', async () => {
  try {
    comparing = !comparing;
    $('.stage').classList.toggle('comparing', comparing);
    $('#compare').setAttribute('aria-pressed', String(comparing));
    reference.hidden = !comparing;
    reference.toggleAttribute('paused', !comparing);
    $('#compare-split').hidden = !comparing;
    if (comparing) {
      await reference.sync();
      await paintReference();
    } else {
      await client.invoke('graphics.detach', { viewport: 1 });
    }
    await viewport.sync();
  } catch (e) {
    report(e);
  }
});
client.addEventListener('graphics-restored', (e) => {
  const slot = e.detail?.viewport ?? 0;
  if (slot === 1) paintReference().catch(report);
  else if (selected)
    commands.renderAsset({ id: selected.id, viewport: 0 }).then(applyView).catch(report);
});
async function syncWorkspace() {
  refreshPending = true;
  if (syncing) return;
  syncing = true;
  try {
    while (refreshPending) {
      refreshPending = false;
      const revision = selectionRevision;
      const next = await commands.workspace();
      const reset =
        !workspace || workspace.generation !== next.generation || catalog.length > next.count;
      const changed = reset || catalog.length !== next.count;
      if (reset) catalog = [];
      if (next.role !== 'preview') {
        while (catalog.length < next.count) {
          const page = await commands.page({ offset: catalog.length, limit: 128 });
          if (!page.length) break;
          catalog.push(...page);
        }
      }
      workspace = next;
      projectMode = !!next.folder || next.count > 3;
      document.body.classList.toggle('detached', next.role === 'preview');
      $('.workspace-label').textContent =
        next.role === 'preview' ? 'Detached preview' : 'Texture workspace';
      $('h1').textContent = next.folder || 'Earth observations';
      $('#assets').hidden = projectMode;
      $('#project-browser').hidden = !projectMode;
      $('#cancel').hidden = !next.running;
      if (changed && next.role !== 'preview') {
        if (projectMode) browser.update(catalog, reset);
        showCatalog();
      }
      $('#count').textContent =
        next.count +
        ' assets' +
        (next.running ? ' · Scanning…' : '') +
        (next.skipped ? ' · ' + next.skipped + ' skipped' : '');
      if (!selecting && revision === selectionRevision && (!selected || selected.id !== next.selected)) {
        const a = await commands.renderAsset({ id: next.selected, viewport: 0 });
        if (selecting || revision !== selectionRevision) continue;
        inspect(a);
        await applyView();
        await paintReference();
        announce(a.fileName);
      }
    }
  } finally {
    syncing = false;
  }
}
client.addEventListener('project-updated', (e) => {
  if (initialized)
    syncWorkspace()
      .then(() => {
        if (e.detail?.message) announce(e.detail.message);
      })
      .catch(report);
});
document.addEventListener('action-error', (e) => report(e.detail));
try {
  viewport.client = reference.client = client;
  await viewport.sync();
  await bindActions(client);
  initialized = true;
  await syncWorkspace();
  performance.mark('atlas-workspace-ready');
} catch (e) {
  report(e);
}
