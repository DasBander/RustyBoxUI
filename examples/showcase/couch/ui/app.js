import { $, rustybox, ready, showError } from './chrome.js';
import { games } from './catalog.js';
const catalog = ['everwild', 'drift', 'signal', 'orbit', 'coast', 'afterlight', 'hollow'].map(
  (id) => games.find((g) => g.id === id)
);
const tabs = ['All games', 'Favorites', 'Adventure'],
  memory = new Map();
const dialog = $('#game'),
  shelf = $('#tiles');
let active = 0,
  selected = null,
  opened = null,
  favorites = new Set(),
  visible = [];
function feature(g) {
  if (!g) {
    $('.hero').hidden = true;
    return;
  }
  $('.hero').hidden = false;
  $('.hero').dataset.game = g.id;
  $('#backdrop').src = g.art;
  $('#genre').textContent = g.genre;
  $('#title').textContent = g.title;
  $('#synopsis').textContent = g.description;
}
function mark(id, focus) {
  selected = visible.find((g) => g.id === id) || visible[0] || null;
  if (selected) memory.set(active, selected.id);
  for (const b of shelf.children) {
    const current = b.dataset.id === selected?.id;
    b.setAttribute('aria-pressed', String(current));
    b.tabIndex = current ? 0 : -1;
    if (current && focus) {
      b.focus({ preventScroll: true });
      b.scrollIntoView({ block: 'nearest', inline: 'nearest' });
    }
  }
  feature(selected);
}
function collection(tab, focus = true) {
  active = (tab + tabs.length) % tabs.length;
  [...$('#tabs').children].forEach((b, i) => b.setAttribute('aria-pressed', String(i === active)));
  visible = catalog.filter(
    (g) => active === 0 || (active === 1 ? favorites.has(g.id) : g.genre.includes('Adventure'))
  );
  $('#shelf-name').textContent = tabs[active];
  $('#count').textContent = visible.length + (visible.length === 1 ? ' game' : ' games');
  shelf.replaceChildren();
  for (const g of visible) {
    const b = document.createElement('button'),
      im = document.createElement('img'),
      label = document.createElement('span');
    b.dataset.id = g.id;
    im.src = g.art;
    im.alt = '';
    label.textContent = g.title;
    b.append(im, label);
    b.onfocus = () => {
      if (selected?.id !== g.id) mark(g.id, false);
    };
    b.onclick = () => {
      mark(g.id, false);
      open();
    };
    shelf.append(b);
  }
  $('#empty').hidden = visible.length > 0;
  mark(memory.get(active), focus);
  if (focus && !visible.length) $('#tabs').children[active].focus();
}
function favoriteText() {
  $('#favorite').textContent = favorites.has(opened.id) ? 'Remove favorite' : 'Save to favorites';
}
function open() {
  if (!selected || dialog.open) return;
  opened = selected;
  $('#game-title').textContent = opened.title;
  $('#description').textContent = opened.description;
  $('#detail-genre').textContent = opened.genre;
  $('#game-art').src = opened.art;
  $('#credit').textContent = opened.credit;
  $('#play').hidden = opened.id !== 'drift';
  favoriteText();
  dialog.showModal();
  (opened.id === 'drift' ? $('#play') : $('#favorite')).focus();
}
function close() {
  if (!dialog.open) return;
  dialog.close();
  collection(active);
}
function actions(key) {
  if (dialog.open) {
    if (key === 'back') close();
    else if (key === 'open') document.activeElement.click();
    else if (['right', 'left', 'up', 'down'].includes(key)) {
      const buttons = [...dialog.querySelectorAll('button:not([hidden])')];
      const delta = ['right', 'down'].includes(key) ? 1 : -1;
      buttons[
        (buttons.indexOf(document.activeElement) + delta + buttons.length) % buttons.length
      ].focus();
    }
    return;
  }
  if (key === 'next' || key === 'previous') {
    collection(active + (key === 'next' ? 1 : -1));
    return;
  }
  const inTabs = $('#tabs').contains(document.activeElement);
  if (inTabs && (key === 'left' || key === 'right')) {
    collection(active + (key === 'right' ? 1 : -1), false);
    $('#tabs').children[active].focus();
    return;
  }
  if (inTabs && (key === 'down' || key === 'open')) {
    if (selected) mark(selected.id, true);
    return;
  }
  if (key === 'back') {
    $('#tabs').children[active].focus();
    return;
  }
  if (key === 'open') {
    open();
    return;
  }
  if (!selected) return;
  const index = visible.indexOf(selected);
  const columns = getComputedStyle(shelf).gridTemplateColumns.split(' ').length;
  if (key === 'up' && index < columns) {
    $('#tabs').children[active].focus();
    return;
  }
  const offset = { left: -1, right: 1, up: -columns, down: columns }[key] || 0;
  mark(visible[Math.max(0, Math.min(visible.length - 1, index + offset))].id, true);
}
tabs.forEach((name, i) => {
  const b = document.createElement('button');
  b.textContent = name;
  b.onclick = () => collection(i);
  $('#tabs').append(b);
});
$('#details').onclick = open;
$('#back').onclick = close;
dialog.oncancel = (e) => {
  e.preventDefault();
  close();
};
$('#favorite').onclick = async () => {
  const id = opened.id;
  if (favorites.has(id)) favorites.delete(id);
  else favorites.add(id);
  favoriteText();
  try {
    await rustybox.invoke('storage.set', { key: 'favorites', value: [...favorites] });
  } catch (e) {
    showError(e);
  }
};
$('#play').onclick = () => rustybox.invoke('host.launchRace').catch(showError);
const keys = {
  ArrowLeft: 'left',
  ArrowRight: 'right',
  ArrowUp: 'up',
  ArrowDown: 'down',
  Escape: 'back',
  KeyQ: 'previous',
  KeyE: 'next',
};
document.addEventListener('keydown', (e) => {
  const key = keys[e.code];
  if (key) {
    e.preventDefault();
    if (!e.repeat || ['up', 'down', 'left', 'right'].includes(key)) actions(key);
  }
});
const held = new Map();
async function controllers() {
  let delay = 400;
  try {
    if (document.hasFocus()) {
      const pad = await rustybox.invoke('host.gamepad');
      const device = pad.connected ? 'Controller connected' : 'Keyboard';
      if ($('#input-device').textContent !== device) $('#input-device').textContent = device;
      const now = performance.now(),
        current = new Set(pad.keys);
      for (const k of held.keys()) if (!current.has(k)) held.delete(k);
      for (const k of current) {
        if (!held.has(k)) {
          held.set(k, now + 420);
          actions(k);
        } else if (['left', 'right', 'up', 'down'].includes(k) && now >= held.get(k)) {
          held.set(k, now + 150);
          actions(k);
        }
      }
      delay = pad.connected ? 32 : 450;
    } else held.clear();
  } catch (e) {
    showError(e);
    return;
  }
  setTimeout(controllers, delay);
}
async function init() {
  const saved = await rustybox.invoke('storage.get', { key: 'favorites' });
  if (Array.isArray(saved)) favorites = new Set(saved);
  collection(0);
  ready();
  controllers();
}
init().catch(showError);
