import { rustybox as client } from './sdk/rustybox.js';
import './sdk/titlebar.js';
import { onCommittedInput } from './sdk/forms.js';
import { icon } from './sdk/components.js';
import { bindCommands } from './commands.js';

const $ = (s) => document.querySelector(s);
const commands = bindCommands(client);
const viewport = $('rb-native-view');
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
  showCatalog();
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
  const a = await commands.selectAsset({ id });
  inspect(a);
  await applyView();
  announce(a.fileName);
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
    if (!catalog.some((item) => item.id === a.id)) catalog.push(a);
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
  if (e.ctrlKey && e.key.toLowerCase() === 'o') {
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
client.addEventListener('graphics-restored', () => applyView().catch(report));

try {
  viewport.client = client;
  catalog = await commands.catalog();
  await viewport.sync();
  await selectAsset(catalog[0].id);
  initialized = true;
} catch (e) {
  report(e);
}
