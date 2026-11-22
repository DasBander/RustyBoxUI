import { rustybox, createPreviewClient } from './sdk/rustybox.js';
import './sdk/titlebar.js';
import { games } from './catalog.js';
import { onCommittedInput } from './sdk/forms.js';
import { icon, escapeHTML as esc, gameWordmark } from './sdk/components.js';
import { heading, featuredHero, empty, gameDetails, nativeLabPage } from './storefront/views.js';
import { defaultProfile, validState, writeProfile } from './storefront/profile.js';

let state = structuredClone(defaultProfile);
const client = rustybox.isNative ? rustybox : createPreviewClient();
const $ = (selector) => document.querySelector(selector);
const byId = (id) => games.find((g) => g.id === id);
let route = 'discover',
  heroIndex = 0,
  filter = 'all',
  sort = 'featured',
  search = '',
  nativeTimer,
  runtime;
const downloads = [];
const featured = ['afterlight', 'drift', 'everwild', 'orbit', 'signal'].map(byId);
function save() {
  return writeProfile(client, state).catch((error) =>
    toast('Could not save: ' + error.message, true)
  );
}
function toast(message, error = false) {
  const element = document.createElement('div');
  element.className = `toast${error ? ' error' : ''}`;
  const symbol = document.createElement('span');
  symbol.className = 'toast-icon';
  symbol.innerHTML = icon(error ? 'close' : 'check', 15);
  element.append(symbol, document.createTextNode(message));
  $('#toasts').append(element);
  setTimeout(() => element.remove(), 4500);
}
function navigation() {
  function item(id, name, glyph, count = 0) {
    const current = route === id || (id === 'discover' && ['browse', 'wishlist'].includes(route));
    return /* HTML */ `<button
      class="nav-item${current ? ' active' : ''}"
      data-route="${id}"
      aria-label="${name}"
      title="${name}"
      ${current ? 'aria-current="page"' : ''}
    >
      ${icon(glyph, 21)}<span>${name}</span>${count
        ? `<span class="nav-count">${count}</span>`
        : ''}
    </button>`;
  }
  $('#navigation').innerHTML =
    item('discover', 'Store', 'store') + item('library', 'Library', 'library');
  $('#utility-navigation').innerHTML =
    item('downloads', 'Downloads', 'download', downloads.filter((d) => d.progress < 100).length) +
    item('native', 'Native Lab', 'cube') +
    item('settings', 'Settings', 'settings');
  const searchBox = $('#search');
  searchBox.placeholder =
    route === 'library'
      ? 'Search library'
      : route === 'wishlist'
        ? 'Search wishlist'
        : 'Search store';
  searchBox.setAttribute('aria-label', searchBox.placeholder);
  const quick = $('#quick-launch');
  const installed = state.installed.map(byId).filter(Boolean);
  quick.hidden = !installed.length;
  quick.innerHTML =
    '<h2>Quick launch</h2>' +
    installed
      .map(
        (g) =>
          /* HTML */ `<button
            class="quick-game"
            data-play="${g.id}"
            aria-label="Play ${esc(g.title)} preview"
            title="Play ${esc(g.title)} preview"
          >
            <img src="${g.cover}" alt="" /><span>${esc(g.title)}</span>${icon('play', 13)}
          </button>`
      )
      .join('');
  document.querySelectorAll('.top-link').forEach((link) => {
    const current = link.dataset.route === route;
    link.classList.toggle('active', current);
    if (current) link.setAttribute('aria-current', 'page');
    else link.removeAttribute('aria-current');
  });
  const count = $('#wishlist-count');
  count.hidden = !state.wishlist.length;
  count.textContent = state.wishlist.length;
}
function cards(list, container) {
  const target = typeof container === 'string' ? $(container) : container;
  list.forEach((game) => {
    const card = document.createElement('rb-game-card');
    card.game = {
      ...game,
      owned: state.library.includes(game.id),
      wishlisted: state.wishlist.includes(game.id),
    };
    target.append(card);
  });
}
function discover() {
  const g = featured[heroIndex];
  $('#main').innerHTML = /* HTML */ `<div class="store-page">
    <h1 class="sr-only">Discover</h1>
    <section class="hero-layout" aria-label="Featured games">
      ${featuredHero(g, state.wishlist.includes(g.id))}
      <div class="featured-rail" role="group" aria-label="Choose a featured game">
        ${featured
          .map(
            (game, i) =>
              /* HTML */ `<button
                class="featured-item${i === heroIndex ? ' active' : ''}"
                data-feature="${i}"
                aria-pressed="${i === heroIndex}"
              >
                <img src="${game.cover}" alt="" /><span>${esc(game.title)}</span>
              </button>`
          )
          .join('')}
      </div>
    </section>
    <div class="section-heading">
      <h2>Free demos</h2>
      <button class="text-button" data-route="browse">Browse all ${icon('chevron', 14)}</button>
    </div>
    <section class="game-grid shelf" id="picks" aria-label="Free demos"></section>
  </div>`;
  cards(['drift', 'everwild', 'orbit', 'signal', 'coast'].map(byId), '#picks');
}
function selectFeatured(index) {
  if (!featured[index] || index === heroIndex) return;
  heroIndex = index;
  $('.hero').outerHTML = featuredHero(featured[index], state.wishlist.includes(featured[index].id));
  document.querySelectorAll('[data-feature]').forEach((button, i) => {
    button.classList.toggle('active', i === index);
    button.setAttribute('aria-pressed', String(i === index));
  });
}
function listGames(wishlistOnly = false) {
  let list = games.filter(
    (g) =>
      (!wishlistOnly || state.wishlist.includes(g.id)) &&
      (!search || `${g.title} ${g.genre}`.toLowerCase().includes(search)) &&
      (filter === 'all' || g.genre.toLowerCase().includes(filter))
  );
  if (sort === 'name') list.sort((a, b) => a.title.localeCompare(b.title));
  if (sort === 'new') list.sort((a, b) => Number(Boolean(b.new)) - Number(Boolean(a.new)));
  $('#main').innerHTML =
    heading(wishlistOnly ? 'Wishlist' : 'Browse games') +
    /* HTML */ `<div class="toolbar">
        <div class="filters">
          ${['all', 'adventure', 'exploration', 'racing', 'rpg']
            .map(
              (f) =>
                /* HTML */ `<button
                  class="filter ${filter === f ? 'active' : ''}"
                  data-filter="${f}"
                >
                  ${f === 'all'
                    ? 'All games'
                    : f.toUpperCase() === 'RPG'
                      ? 'RPG'
                      : f[0].toUpperCase() + f.slice(1)}
                </button>`
            )
            .join('')}
        </div>
        <select class="sort" id="sort" aria-label="Sort games">
          <option value="featured">Featured</option>
          <option value="name">Name A–Z</option>
          <option value="new">New arrivals</option>
        </select>
      </div>
      <div class="game-grid browse-grid" id="browse-grid"></div>`;
  $('#sort').value = sort;
  if (list.length) cards(list, '#browse-grid');
  else
    $('#browse-grid').outerHTML =
      wishlistOnly && !state.wishlist.length
        ? empty(
            'Your wishlist is empty',
            'Choose Wishlist on a game to save it here.',
            'Browse games'
          )
        : empty('No matching games', 'Try another search or genre.', 'Clear filters', 'filters');
}
function library() {
  const list = state.library
    .map(byId)
    .filter((g) => g && (!search || g.title.toLowerCase().includes(search)));
  $('#main').innerHTML =
    heading(
      'Library',
      `${state.library.length} ${state.library.length === 1 ? 'game' : 'games'} in your collection`
    ) +
    '<section class="library-grid" aria-label="Your games">' +
    list
      .map((g) => {
        const installed = state.installed.includes(g.id);
        const queued = downloads.some((d) => d.id === g.id && d.progress < 100);
        const action = installed
          ? `data-play="${g.id}"`
          : queued
            ? 'data-route="downloads"'
            : `data-install="${g.id}"`;
        return /* HTML */ `<article class="library-card" data-game="${g.id}">
          <div class="library-cover">
            <img src="${g.art}" alt="" />
            <div class="cover-wordmark" aria-hidden="true">${gameWordmark(g)}</div>
            <button
              class="cover-open"
              data-open="${g.id}"
              aria-label="View ${esc(g.title)}"
            ></button>
          </div>
          <div class="library-body">
            <h2>${esc(g.title)}</h2>
            <p>${esc(g.genre)}</p>
            <div class="library-actions">
              <rb-button small variant="${installed ? 'primary' : 'secondary'}" ${action}
                >${icon(installed ? 'play' : 'download', 14)}
                ${installed ? 'Play preview' : queued ? 'View download' : 'Install demo'}</rb-button
              ><span>${installed ? 'Installed' : `${g.size} GB demo`}</span>
            </div>
          </div>
        </article>`;
      })
      .join('') +
    '</section>';
  if (!list.length)
    $('#main').insertAdjacentHTML(
      'beforeend',
      empty(
        search ? 'No matching games' : 'Your library is empty',
        search ? 'Try another game title.' : 'Add a demo from the store.',
        search ? 'Clear search' : 'Browse games',
        search ? 'filters' : 'browse'
      )
    );
}
function renderDownloads() {
  const running = downloads.filter((d) => !d.paused && d.progress < 100).length;
  $('#main').innerHTML =
    heading('Downloads') +
    /* HTML */ `<div class="download-summary">
        ${icon('download', 30)}
        <div>
          <h2>
            ${running
              ? `${running} ${running === 1 ? 'download' : 'downloads'} in progress`
              : 'Download queue'}
          </h2>
          <p>This showcase simulates file transfers.</p>
        </div>
      </div>
      <div id="download-list">
        ${downloads
          .map((d) => {
            const g = byId(d.id);
            return /* HTML */ `<article
              class="download-row ${d.paused ? 'paused' : ''}"
              data-download="${d.id}"
            >
              <img src="${g.art}" alt="" />
              <div class="download-copy">
                <h3>${esc(g.title)}</h3>
                <div
                  class="progress-track"
                  role="progressbar"
                  aria-label="${esc(g.title)} download"
                  aria-valuemin="0"
                  aria-valuemax="100"
                  aria-valuenow="${Math.floor(d.progress)}"
                >
                  <div class="progress-fill" style="width:${d.progress}%"></div>
                </div>
                <div class="download-meta">
                  <span class="download-status">${downloadLabel(d)}</span
                  ><span class="download-percent">${Math.floor(d.progress)}%</span>
                </div>
              </div>
              <button
                class="icon-button"
                data-pause="${d.id}"
                aria-label="${d.paused ? 'Resume' : 'Pause'} ${esc(g.title)}"
                ${d.progress >= 100 ? 'disabled' : ''}
              >
                ${icon(d.paused ? 'play' : 'pause', 17)}</button
              ><button
                class="icon-button"
                data-cancel="${d.id}"
                aria-label="${d.progress >= 100 ? 'Dismiss' : 'Cancel'} ${esc(g.title)}"
              >
                ${icon('close', 17)}
              </button>
            </article>`;
          })
          .join('')}
      </div>`;
  if (!downloads.length)
    $('#download-list').innerHTML = empty(
      'No downloads',
      'Select Install demo on a game to add it to the queue.',
      'Browse games'
    );
}
function downloadLabel(d) {
  const g = byId(d.id);
  return d.progress >= 100
    ? 'Installed'
    : d.paused
      ? 'Paused'
      : `${((g.size * d.progress) / 100).toFixed(1)} of ${g.size} GB`;
}
function install(id) {
  if (!state.library.includes(id)) state.library.push(id);
  if (state.installed.includes(id)) {
    play(id);
    return;
  }
  if (!downloads.some((d) => d.id === id)) downloads.push({ id, progress: 0, paused: false });
  save();
  closeDialog();
  go('downloads');
  toast(`${byId(id).title} added to the demo queue.`);
}
function tickDownloads() {
  let completed = false;
  downloads.forEach((d) => {
    if (d.paused || d.progress >= 100) return;
    d.progress = Math.min(100, d.progress + 1.25);
    if (d.progress === 100) {
      if (!state.installed.includes(d.id)) state.installed.push(d.id);
      completed = true;
      toast(`${byId(d.id).title} is ready to preview.`);
    }
    const row = document.querySelector(`[data-download="${d.id}"]`);
    if (row) {
      row.querySelector('.progress-fill').style.width = d.progress + '%';
      row
        .querySelector('[role="progressbar"]')
        .setAttribute('aria-valuenow', String(Math.floor(d.progress)));
      row.querySelector('.download-status').textContent = downloadLabel(d);
      row.querySelector('.download-percent').textContent = Math.floor(d.progress) + '%';
    }
  });
  if (completed) {
    save();
    navigation();
    if (route === 'downloads') renderDownloads();
  }
}
function applySettings() {
  const colors = {
    gold: ['#e9b870', '233,184,112'],
    mint: ['#91c7ad', '145,199,173'],
    blue: ['#5bc9ff', '91,201,255'],
  };
  const color = colors[state.accent] || colors.blue;
  document.documentElement.style.setProperty('--accent', color[0]);
  document.documentElement.style.setProperty('--accent-rgb', color[1]);
  document.body.classList.toggle('reduce-motion', state.reducedMotion);
  document.body.classList.toggle('compact', state.compact);
}
function settings() {
  $('#main').innerHTML =
    heading('Settings') +
    /* HTML */ `<div class="settings-layout">
      <section class="settings-panel">
        <h2>Appearance</h2>
        <div class="setting-row">
          <div><strong>Accent color</strong></div>
          <select class="sort" id="color-choice" aria-label="Accent color">
            <option value="blue">Blue</option>
            <option value="gold">Gold</option>
            <option value="mint">Mint</option>
          </select>
        </div>
        <div class="setting-row">
          <div><strong>Reduced motion</strong><small>Turn off interface animations.</small></div>
          <rb-toggle
            label="Reduced motion"
            data-setting="reducedMotion"
            ${state.reducedMotion ? 'checked' : ''}
          ></rb-toggle>
        </div>
        <div class="setting-row">
          <div><strong>Compact cards</strong><small>Fit more games in each row.</small></div>
          <rb-toggle
            label="Compact cards"
            data-setting="compact"
            ${state.compact ? 'checked' : ''}
          ></rb-toggle>
        </div>
      </section>
      <section class="runtime-card">
        <h2>About RustyBox</h2>
        <div class="runtime-row">
          <span>Version</span><strong>${esc(runtime?.version || '0.6.0')}</strong>
        </div>
        <div class="runtime-row">
          <span>Runtime</span><strong>${client.isNative ? 'WebView2' : 'Browser preview'}</strong>
        </div>
        <p>
          This launcher uses a fictional game catalog. Transfers are simulated. Your library and
          preferences are stored on this device.
        </p>
      </section>
    </div>`;
  $('#color-choice').value = state.accent;
}
async function nativeLab() {
  $('#main').innerHTML = nativeLabPage(client.isNative);
  const view = $('rb-native-view');
  view.client = client;
  view.addEventListener('native-ready', (event) => {
    const g = event.detail;
    if ($('#native-backend')) {
      $('#native-backend').textContent = g.backend;
      $('#native-device').textContent = g.hardware ? 'Hardware GPU' : 'Software / WARP';
    }
  });
  view.addEventListener('native-error', (event) => {
    if ($('#native-message')) $('#native-message').textContent = event.detail.message;
  });
  if (client.isNative) await view.sync();
  if (route !== 'native') return;
  nativeTimer = setInterval(async () => {
    try {
      const info = await client.invoke('graphics.info');
      if ($('#native-frames'))
        $('#native-frames').textContent = (info.frames || 0).toLocaleString();
    } catch {}
  }, 1000);
}
function render() {
  clearInterval(nativeTimer);
  navigation();
  if (route === 'discover') discover();
  else if (route === 'browse') listGames();
  else if (route === 'wishlist') listGames(true);
  else if (route === 'library') library();
  else if (route === 'downloads') renderDownloads();
  else if (route === 'settings') settings();
  else if (route === 'native') nativeLab();
}
function go(next) {
  if (
    !['discover', 'browse', 'wishlist', 'library', 'downloads', 'settings', 'native'].includes(next)
  )
    next = 'discover';
  route = next;
  filter = 'all';
  search = '';
  $('#search').value = '';
  history.replaceState(null, '', `#${next}`);
  render();
  $('#main').scrollTop = 0;
}
function wishlist(id) {
  if (state.wishlist.includes(id)) state.wishlist = state.wishlist.filter((x) => x !== id);
  else state.wishlist.push(id);
  save();
  if (route === 'discover') {
    const wished = state.wishlist.includes(id);
    const count = $('#wishlist-count');
    count.hidden = !state.wishlist.length;
    count.textContent = state.wishlist.length;
    const trigger = $('.hero-wishlist');
    if (trigger.dataset.wish === id) {
      trigger.setAttribute('aria-pressed', String(wished));
      trigger.innerHTML = icon(wished ? 'check' : 'plus', 17) + ' Wishlist';
    }
    const card = Array.from(document.querySelectorAll('rb-game-card')).find(
      (el) => el.game.id === id
    );
    if (card) {
      const focused = card.contains(document.activeElement);
      card.game = { ...card.game, wishlisted: wished };
      if (focused) card.querySelector('.wishlist-button').focus({ preventScroll: true });
    }
  } else render();
  toast(
    state.wishlist.includes(id)
      ? `${byId(id).title} saved to your wishlist.`
      : 'Removed from your wishlist.'
  );
}
function closeDialog() {
  if ($('#detail-dialog').open) $('#detail-dialog').close();
}
function details(id) {
  const g = byId(id);
  if (!g) return;
  const owned = state.library.includes(id),
    installed = state.installed.includes(id);
  $('#detail-dialog').innerHTML = gameDetails(g, owned, installed, state.wishlist.includes(id));
  $('#detail-dialog').showModal();
}
function play(id) {
  closeDialog();
  go('native');
  toast(`Opening the graphics preview for ${byId(id).title}.`);
}
document.addEventListener('click', async (event) => {
  const target = event.target.closest(
    '[data-route],[data-feature],[data-open],[data-wish],[data-install],[data-play],[data-pause],[data-cancel],[data-filter],[data-reset],[data-close-dialog],[data-dialog-wish],[data-window]'
  );
  if (!target) return;
  try {
    const d = target.dataset;
    if (d.route) {
      event.preventDefault();
      go(d.route);
    } else if (d.feature !== undefined) selectFeatured(Number(d.feature));
    else if (d.open) details(d.open);
    else if (d.wish) wishlist(d.wish);
    else if (d.install) install(d.install);
    else if (d.play) play(d.play);
    else if (d.pause) {
      const item = downloads.find((x) => x.id === d.pause);
      if (item) {
        item.paused = !item.paused;
        renderDownloads();
      }
    } else if (d.cancel) {
      const index = downloads.findIndex((x) => x.id === d.cancel);
      if (index >= 0) downloads.splice(index, 1);
      renderDownloads();
      navigation();
    } else if (d.filter) {
      filter = d.filter;
      listGames(route === 'wishlist');
    } else if (d.reset) {
      go(d.reset === 'filters' ? route : 'browse');
    } else if ('closeDialog' in d) closeDialog();
    else if (d.dialogWish) {
      wishlist(d.dialogWish);
      details(d.dialogWish);
      $('#detail-dialog [data-dialog-wish] button').focus({ preventScroll: true });
    } else if (d.window) {
      await client.invoke('window.' + d.window);
    }
  } catch (error) {
    toast(error.message, true);
  }
});
document.addEventListener('game-open', (e) => details(e.detail));
document.addEventListener('game-wishlist', (e) => wishlist(e.detail));
document.addEventListener('change', (event) => {
  if (event.target.id === 'color-choice') {
    state.accent = event.target.value;
    applySettings();
    save();
  }
  if (event.target.id === 'sort') {
    sort = event.target.value;
    listGames(route === 'wishlist');
  }
  if (event.target.dataset.setting) {
    state[event.target.dataset.setting] = event.detail;
    applySettings();
    save();
  }
});
onCommittedInput($('#search'), (value) => {
  search = value.trim().toLowerCase();
  if (!['browse', 'wishlist', 'library'].includes(route)) {
    route = 'browse';
    history.replaceState(null, '', '#browse');
    navigation();
  }
  if (route === 'library') library();
  else listGames(route === 'wishlist');
});
document.addEventListener('keydown', (event) => {
  if (event.isComposing || event.keyCode === 229 || $('#detail-dialog').open) return;
  if (event.key === '/' && !['INPUT', 'TEXTAREA'].includes(document.activeElement.tagName)) {
    event.preventDefault();
    $('#search').focus();
  }
  const choice = event.target.closest('[data-feature]');
  if (!choice) return;
  const index = Number(choice.dataset.feature);
  const last = featured.length - 1;
  const forward = (index + 1) % featured.length;
  const backward = (index + last) % featured.length;
  const next = {
    ArrowDown: forward,
    ArrowRight: forward,
    ArrowUp: backward,
    ArrowLeft: backward,
    Home: 0,
    End: last,
  }[event.key];
  if (next === undefined) return;
  event.preventDefault();
  selectFeatured(next);
  document.querySelector(`[data-feature="${next}"]`).focus();
});
$('#profile-button').addEventListener('click', () => go('settings'));
$('#notifications').innerHTML = icon('bell', 18);
$('#search-icon').innerHTML = icon('search', 18);
$('#notifications').addEventListener('click', () =>
  toast(
    downloads.some((d) => d.progress < 100)
      ? 'Your demo transfers are in Downloads.'
      : 'No new notifications.'
  )
);
$('#detail-dialog').addEventListener('click', (event) => {
  if (event.target === $('#detail-dialog')) closeDialog();
});
document.addEventListener('click', async (event) => {
  if (event.target.closest('#ping')) {
    const start = performance.now();
    try {
      const reply = await client.invoke('diagnostics.echo', {
        message: 'Hello from the web',
        sentAt: Date.now(),
      });
      $('#bridge-output').textContent =
        `${JSON.stringify(reply)}\nRound trip: ${(performance.now() - start).toFixed(2)} ms / ${client.isNative ? 'Rust ↔ WebView2' : 'browser preview'}`;
    } catch (error) {
      toast(error.message, true);
    }
  }
  if (event.target.closest('#native-pause')) {
    const view = $('rb-native-view');
    view.toggleAttribute('paused');
    if (view.hasAttribute('paused')) {
      await client.invoke('graphics.detach');
      $('#native-message').textContent = 'Renderer paused.';
    } else await view.sync();
    $('#native-pause button').textContent = view.hasAttribute('paused')
      ? 'Resume renderer'
      : 'Pause renderer';
  }
});
try {
  runtime = await client.invoke('runtime.info');
  state = {
    ...state,
    ...validState(await client.invoke('storage.get', { key: 'showcase.state' }), byId),
  };
} catch (error) {
  toast(error.message, true);
}
applySettings();
go(location.hash.slice(1) || 'discover');
setInterval(tickDownloads, 500);
