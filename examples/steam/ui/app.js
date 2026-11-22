import './sdk/titlebar.js';
import { onCommittedInput } from './sdk/forms.js';
import { escapeHTML as esc } from './sdk/components.js';
import { client, host, gameById, profile, initialize, change, record, getPack } from './profile.js';
import {
  glyph,
  sidebar,
  detail,
  home,
  store,
  collections,
  collection,
  downloads,
} from './views.js';
import { createRadio } from './radio.js';

const $ = (selector) => document.querySelector(selector);
const dialog = $('#overlay'),
  main = $('#content');
const collapsed = new Set();
let route = 'game/signal',
  tab = 'activity',
  search = '',
  installedOnly = false;
let trail = [],
  cursor = 0,
  noticeTimer;
const pages = { home, store, collections, downloads };
const radio = createRadio({ showDialog, dialogHead, render });

function notify(text) {
  $('#notice').textContent = text;
  clearTimeout(noticeTimer);
  noticeTimer = setTimeout(() => {
    $('#notice').textContent = '';
  }, 3800);
}
function toggleDrawer(open) {
  document.body.classList.toggle('drawer-open', open);
  $('.drawer-toggle').setAttribute('aria-expanded', String(open));
  $('#drawer-shade').hidden = !open;
  if (open) $('#search').focus();
}
function gameId() {
  return route.startsWith('game/') ? route.slice(5) : null;
}
function list() {
  if (!profile) return;
  const ids = profile.owned
    .filter(
      (id) =>
        (!installedOnly || profile.packs[id]) &&
        gameById.get(id).title.toLowerCase().includes(search)
    )
    .sort((a, b) => gameById.get(a).title.localeCompare(gameById.get(b).title));
  $('#game-list').innerHTML = sidebar(ids, collapsed, gameId());
  $('#ready-filter').setAttribute('aria-pressed', String(installedOnly));
}
function render() {
  const [section, id] = route.split('/');
  const top = { game: 'home', collection: 'collections' }[section] || section;
  for (const b of document.querySelectorAll('.primary-nav button')) {
    if (b.dataset.go === top) b.setAttribute('aria-current', 'page');
    else b.removeAttribute('aria-current');
  }
  $('#back').disabled = cursor <= 0;
  $('#forward').disabled = cursor >= trail.length - 1;
  $('.profile').setAttribute('aria-label', profile.name + ' profile settings');
  const downloadsLink = document.querySelector('footer [data-go="downloads"]');
  if (route === 'downloads') downloadsLink.setAttribute('aria-current', 'page');
  else downloadsLink.removeAttribute('aria-current');
  $('#profile-name').textContent = profile.name;
  $('.avatar').textContent = Array.from(profile.name)[0].toUpperCase();
  $('.library-home').classList.toggle('selected', route === 'home');
  const count = profile.events.filter((e) => e.kind === 'install').length;
  $('#download-count').hidden = !count;
  $('#download-count').textContent = count;
  list();
  if (section === 'game') main.innerHTML = detail(gameById.get(id), tab);
  else if (section === 'collection') main.innerHTML = collection(id);
  else main.innerHTML = pages[section]();
  main.dataset.route = route;
}
function validRoute(next) {
  return (
    Object.hasOwn(pages, next) ||
    ['collection/all', 'collection/fav', 'collection/ready'].includes(next) ||
    (next.startsWith('game/') && gameById.has(next.slice(5)))
  );
}
function go(next, nextTab = 'activity', push = true) {
  if (!validRoute(next)) next = 'home';
  if (next !== route || nextTab !== tab || !trail.length) {
    if (push) {
      trail.splice(cursor + 1);
      trail.push([next, nextTab]);
      cursor = trail.length - 1;
    }
    route = next;
    tab = nextTab;
  }
  history.replaceState(null, '', `#${route}`);
  toggleDrawer(false);
  render();
  main.scrollTop = 0;
}
function historyStep(step) {
  const next = cursor + step;
  if (next < 0 || next >= trail.length) return;
  cursor = next;
  go(...trail[cursor], false);
}
function showDialog(html, kind = 'info') {
  $('#dialog-content').innerHTML = html;
  dialog.dialog.dataset.kind = kind;
  dialog.open();
}
function dialogHead(title) {
  return /* HTML */ `<div class="dialog-head">
    <h2 id="dialog-title">${esc(title)}</h2>
    <button class="square" data-dismiss aria-label="Close dialog">${glyph('close')}</button>
  </div>`;
}
function showProfile() {
  showDialog(
    /* HTML */ `${dialogHead('Local profile')}
      <form id="profile-form">
        <label for="player-name">Display name</label
        ><input
          id="player-name"
          name="name"
          value="${esc(profile.name)}"
          required
          maxlength="24"
          autocomplete="off"
        />
        <p>Favorites, installed demos and play history are saved on this device.</p>
        <button class="secondary" type="submit">Save profile</button>
      </form>`
  );
}
function info(id) {
  const g = gameById.get(id);
  showDialog(
    /* HTML */ `${dialogHead(g.title)}<img class="dialog-image" src="${g.art}" alt="" />
      <p>${esc(g.description)}</p>
      <dl>
        <div>
          <dt>Genre</dt>
          <dd>${esc(g.genre)}</dd>
        </div>
        <div>
          <dt>Included demo</dt>
          <dd>Radio check</dd>
        </div>
      </dl>
      <p class="credit">${esc(g.credit)}</p>`
  );
}
function help() {
  showDialog(
    /* HTML */ `${dialogHead('About this library')}
      <p>
        A Steam-inspired example built with RustyBoxUI. Each fictional game includes a short
        radio-tuning challenge. The store, demo packs and achievements work offline.
      </p>
      <dl>
        <div>
          <dt>Find a game</dt>
          <dd>Ctrl F</dd>
        </div>
        <div>
          <dt>Close a dialog</dt>
          <dd>Escape</dd>
        </div>
        <div>
          <dt>Window engine</dt>
          <dd>${client.isNative ? 'RustyBox / WebView2' : 'Browser preview'}</dd>
        </div>
      </dl>
      <p class="credit">
        Game artwork credits appear under Artwork. Goldstone photograph by NASA/JPL-Caltech.
      </p>`
  );
}
async function favorite(id) {
  await change((next) => {
    if (!next.owned.includes(id)) return;
    next.favorites = next.favorites.includes(id)
      ? next.favorites.filter((k) => k !== id)
      : [...next.favorites, id];
  });
  render();
  $('.favorite')?.focus({ preventScroll: true });
}
async function own(id) {
  await change((next) => {
    if (!next.owned.includes(id)) {
      next.owned.push(id);
      record(next, 'add', id);
    }
  });
  render();
  notify('Added to your library.');
}
async function install(id) {
  await change(async (next) => {
    if (next.packs[id]) return;
    if (!next.owned.includes(id)) throw new Error('Add this game to your library first.');
    const pack = await getPack(id);
    next.packs[id] = pack;
    record(next, 'install', id, { bytes: new TextEncoder().encode(JSON.stringify(pack)).length });
  });
  render();
  notify('Demo installed and ready to play.');
}
function problem(error) {
  notify(error.message || String(error));
}
async function action(which, id) {
  switch (which) {
    case 'profile':
      return showProfile();
    case 'help':
      return help();
    case 'info':
      return info(id);
    case 'art': {
      const g = gameById.get(id);
      return showDialog(
        /* HTML */ `${dialogHead(g.title)}<img
            class="full-art"
            src="${g.art}"
            alt="${esc(g.title)} key art"
          />
          <p class="credit">${esc(g.credit)}</p>`,
        'art'
      );
    }
    case 'favorite':
      return favorite(id);
    case 'own':
      return own(id);
    case 'install':
      return install(id);
    case 'play':
      return radio.start(id);
    case 'lock':
      return radio.lock();
  }
}
for (const node of document.querySelectorAll('[data-icon]'))
  node.innerHTML = glyph(node.dataset.icon);
$('#back').addEventListener('click', () => historyStep(-1));
$('#forward').addEventListener('click', () => historyStep(1));
$('.drawer-toggle').addEventListener('click', () =>
  toggleDrawer(!document.body.classList.contains('drawer-open'))
);
$('#drawer-shade').addEventListener('click', () => toggleDrawer(false));
$('#ready-filter').addEventListener('click', () => {
  installedOnly = !installedOnly;
  list();
});
onCommittedInput($('#search'), (value) => {
  search = value.trim().toLowerCase();
  if (search) collapsed.clear();
  list();
});
document.addEventListener('rb-select', (e) => {
  if (profile) go(e.detail === 'ready' ? 'collection/ready' : e.detail);
});
document.addEventListener('click', (e) => {
  const b = e.target.closest('button');
  if (!b) return;
  const d = b.dataset;
  if (d.window) {
    if (client.isNative)
      (async () => {
        if (d.window === 'close') await radio.finish();
        await client.invoke('window.' + d.window);
      })().catch(problem);
    return;
  }
  if (!profile) return;
  if (d.go) go(d.go);
  else if (d.guide) go('game/' + d.guide, 'guide');
  else if (d.tab) {
    go(route, d.tab);
    document.querySelector(`[data-tab="${d.tab}"]`)?.focus({ preventScroll: true });
  } else if (d.group) {
    if (collapsed.has(d.group)) collapsed.delete(d.group);
    else collapsed.add(d.group);
    list();
    document.querySelector(`[data-group="${d.group}"]`)?.focus({ preventScroll: true });
  } else if (d.action) action(d.action, d.id).catch(problem);
});
document.addEventListener('input', (e) => {
  if (e.target.id === 'dial') radio.tune(e.target.value);
});
document.addEventListener('submit', (e) => {
  if (e.target.id !== 'profile-form') return;
  e.preventDefault();
  const input = $('#player-name'),
    name = input.value.trim().slice(0, 24);
  if (!name) {
    input.focus();
    notify('Enter a display name.');
    return;
  }
  change((next) => {
    next.name = name;
  })
    .then(() => {
      render();
      dialog.close();
    })
    .catch(problem);
});
dialog.dialog.addEventListener('close', () => {
  const kind = dialog.dialog.dataset.kind;
  radio
    .finish()
    .then(() => {
      if ((kind === 'radio' || kind === 'result') && !dialog.dialog.open)
        $('.play-button')?.focus({ preventScroll: true });
    })
    .catch(problem);
});
document.addEventListener('keydown', (e) => {
  if (e.isComposing || e.keyCode === 229 || dialog.dialog.open) return;
  const row = e.target.closest('.list-game');
  if (row && ['ArrowUp', 'ArrowDown', 'Home', 'End'].includes(e.key)) {
    const rows = Array.from(
      document.querySelectorAll('.list-group > div:not([hidden]) .list-game')
    );
    const i = rows.indexOf(row);
    const next = {
      ArrowUp: Math.max(0, i - 1),
      ArrowDown: Math.min(rows.length - 1, i + 1),
      Home: 0,
      End: rows.length - 1,
    }[e.key];
    e.preventDefault();
    rows[next]?.focus();
    return;
  }
  if (e.ctrlKey && e.key.toLowerCase() === 'f') {
    e.preventDefault();
    if (matchMedia('(max-width: 730px)').matches) toggleDrawer(true);
    else $('#search').focus();
  }
  if (e.key === 'Escape' && document.body.classList.contains('drawer-open')) {
    toggleDrawer(false);
    $('.drawer-toggle').focus();
  }
});
matchMedia('(max-width: 730px)').addEventListener('change', () => toggleDrawer(false));
window.addEventListener('hashchange', () => {
  if (profile) go(location.hash.slice(1));
});
if (!client.isNative)
  for (const control of document.querySelectorAll('[data-window]')) control.disabled = true;
try {
  await initialize();
  const recent = profile.owned
    .filter((id) => profile.stats[id]?.last)
    .sort((a, b) => profile.stats[b].last - profile.stats[a].last);
  go(location.hash.slice(1) || 'game/' + (recent[0] || 'signal'));
  main.setAttribute('aria-busy', 'false');
} catch (error) {
  main.innerHTML = '<div class="page"><h1>Library unavailable</h1><p id="startup-error"></p></div>';
  $('#startup-error').textContent = error.message;
  main.setAttribute('aria-busy', 'false');
}
