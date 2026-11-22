import { icon as sdkIcon, escapeHTML as esc } from './sdk/components.js';
import { games } from './catalog.js';
import { gameById, profile, playStats, achievements } from './profile.js';

const shapes = {
  home: 'm3 11 9-8 9 8M5 10v11h5v-7h4v7h5V10',
  installed: 'M12 2v12m-5-5 5 5 5-5M4 15v5h16v-5',
  star: 'm12 3 2.7 5.7 6.3.9-4.5 4.4 1.1 6.2-5.6-3-5.6 3 1.1-6.2L3 9.6l6.3-.9L12 3Z',
  book: 'M3 4h6c2 0 3 1 3 3 0-2 1-3 3-3h6v15h-6c-2 0-3 1-3 2 0-1-1-2-3-2H3V4Zm9 3v14',
  info: 'M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18Zm0 7v7m0-11v1',
  radio:
    'm12 9-4 11m4-11 4 11m-7-5h6M7.5 5.5a7 7 0 0 0 0 9M16.5 5.5a7 7 0 0 1 0 9M4.5 3a11 11 0 0 0 0 14M19.5 3a11 11 0 0 1 0 14',
  wave: 'M2 12h4l2-7 3 14 3-16 3 15 2-6h3',
  trophy: 'M7 3h10v5c0 4-2 6-5 6s-5-2-5-6V3Zm0 2H3v2c0 3 2 5 5 5m9-7h4v2c0 3-2 5-5 5m-4 2v5m-4 2h8',
  time: 'M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18Zm0 4v5l4 2',
  image: 'M3 4h18v16H3zM4 17l5-6 4 4 3-3 5 6',
  pad: 'M7 7h10c2 0 3 2 4 6 1 4 0 6-2 5l-4-3H9l-4 3c-2 1-3-1-2-5 1-4 2-6 4-6Zm1 2v4m-2-2h4m5-1h1m1 2h1',
  folder: 'M3 5h6l2 2h10v12H3V5Z',
  user: 'M12 3a4 4 0 1 0 0 8 4 4 0 0 0 0-8ZM4 21v-2c0-4 3-6 8-6s8 2 8 6v2',
};
function glyph(name, size = 18) {
  if (!shapes[name]) return sdkIcon(name, size);
  return `<svg width="${size}" height="${size}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.65" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="${shapes[name]}"/></svg>`;
}
const guides = {
  signal: 'Tuning the emergency relay',
  afterlight: 'Contacting the watchtower',
  coast: 'Calling the lighthouse',
  drift: 'Setting the pit radio',
  everwild: 'Checking the trail frequency',
  orbit: 'Hailing the flight deck',
  hollow: 'Calling back to camp',
};
function logo(g) {
  if (g.id === 'signal') return '<small>The Last</small><strong>Signal</strong>';
  return /* HTML */ `<strong>${esc(g.wordmark)}</strong>${g.subtitle
      ? `<small>${esc(g.subtitle)}</small>`
      : ''}`;
}
function dateLabel(ms) {
  if (!ms) return 'Never';
  const d = new Date(ms);
  if (d.toDateString() === new Date().toDateString()) return 'Today';
  return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
}
function playTime(seconds) {
  if (seconds && seconds < 60) return '<1 min';
  return seconds >= 3600
    ? `${(seconds / 3600).toFixed(1)} hours`
    : `${Math.floor(seconds / 60)} min`;
}
function shelf(list, label) {
  return /* HTML */ `<div class="shelf" aria-label="${esc(label)}">
    ${list
      .map(
        (g) =>
          /* HTML */ `<button
            class="capsule"
            data-go="game/${g.id}"
            data-title="${g.id}"
            aria-label="View ${esc(g.title)}"
          >
            <img src="${g.cover}" alt="" /><span class="cover-logo">${logo(g)}</span>
          </button>`
      )
      .join('')}
  </div>`;
}
function sidebar(visible, collapsed, selected) {
  const group = (key, title, ids) => {
    if (!ids.length) return '';
    return /* HTML */ `<section class="list-group">
      <button class="list-heading" data-group="${key}" aria-expanded="${!collapsed.has(key)}">
        <span class="twisty"></span>${title}<span>${ids.length}</span>
      </button>
      <div ${collapsed.has(key) ? 'hidden' : ''}>
        ${ids
          .map((id) => {
            const g = gameById.get(id);
            return /* HTML */ `<button
              class="list-game${profile.packs[id] ? ' installed' : ''}"
              data-go="game/${id}"
              ${id === selected ? 'aria-current="page"' : ''}
            >
              <img src="${g.cover}" alt="" /><span>${esc(g.title)}</span>
            </button>`;
          })
          .join('')}
      </div>
    </section>`;
  };
  return (
    group(
      'fav',
      'Favorites',
      visible.filter((id) => profile.favorites.includes(id))
    ) +
      group(
        'other',
        'Uncategorized',
        visible.filter((id) => !profile.favorites.includes(id))
      ) || '<p class="list-empty">No matching games.</p>'
  );
}
function guideCard(g) {
  return /* HTML */ `<button class="guide-card" data-guide="${g.id}">
    <img src="media/goldstone.webp" alt="" /><span class="guide-copy"
      ><small>${glyph('book', 15)} Field guide</small><strong>${esc(guides[g.id])}</strong
      ><span>Use signal strength to find and lock each of the three channels.</span
      ><b>Read guide ${glyph('arrow', 15)}</b></span
    >
  </button>`;
}
function awards(g) {
  const list = achievements(g.id),
    earned = list.filter((a) => a.earned).length;
  return /* HTML */ `<section class="awards panel">
    <header>
      <h2>Achievements</h2>
      <span>${earned} / ${list.length}</span>
    </header>
    <progress value="${earned}" max="${list.length}" aria-label="Achievements unlocked"></progress>
    ${list
      .map(
        (a, i) =>
          /* HTML */ `<div class="award ${a.earned ? 'earned' : ''}">
            <span class="award-emblem grade-${i}">${glyph(a.icon, 25)}</span>
            <div>
              <h3>
                ${a.name}<span class="visually-hidden"
                  >${a.earned ? ', unlocked' : ', locked'}</span
                >
              </h3>
              <p>${a.detail}</p>
            </div>
            ${a.earned ? glyph('check', 15) : ''}
          </div>`
      )
      .join('')}
  </section>`;
}
function activity(g) {
  const events = profile.events.filter((e) => e.id === g.id).slice(0, 5);
  const labels = {
    play: 'Started the radio demo',
    finish: 'Completed all three channels',
    install: 'Installed the demo pack',
    add: 'Added to library',
  };
  return /* HTML */ `<div class="detail-columns">
    <section class="activity">
      <h2>Activity</h2>
      ${guideCard(g)}
      ${events.length
        ? `<div class="timeline">${events.map((e) => /* HTML */ `<p>${glyph(e.kind === 'finish' ? 'trophy' : e.kind === 'install' ? 'download' : 'pad', 15)} ${labels[e.kind]} <time>${dateLabel(e.at)}</time></p>`).join('')}</div>`
        : ''}
    </section>
    ${awards(g)}
  </div>`;
}
function guide(g) {
  return /* HTML */ `<article class="guide-article">
    <div>
      <h2>${esc(guides[g.id])}</h2>
      <p>
        Three stations share the receiver. Start the demo, then move the tuning dial until the
        strength meter peaks. Use the arrow keys for fine adjustments.
      </p>
      <p>
        Choose <strong>Lock channel</strong> when the meter reaches 96% or higher. A clean sweep
        means finding all three stations without a missed lock.
      </p>
    </div>
    <figure>
      <img src="media/goldstone.webp" alt="A Deep Space Network antenna at sunset" />
      <figcaption>Goldstone Solar System Radar. NASA/JPL-Caltech.</figcaption>
    </figure>
  </article>`;
}
function detail(g, tab) {
  const owned = profile.owned.includes(g.id),
    installed = Boolean(profile.packs[g.id]),
    fav = profile.favorites.includes(g.id);
  const s = playStats(profile.stats[g.id]),
    action = installed ? 'play' : owned ? 'install' : 'own';
  return /* HTML */ `<article class="game-page" data-title="${g.id}">
    <div class="game-banner">
      <img class="banner-art" src="${g.art}" fetchpriority="high" alt="" />
      <h1 class="game-logo" aria-label="${esc(g.title)}">${logo(g)}</h1>
    </div>
    <div class="launchbar">
      <button
        class="play-button ${installed ? '' : 'install-button'}"
        data-action="${action}"
        data-id="${g.id}"
      >
        ${glyph(installed ? 'play' : owned ? 'download' : 'plus', 23)}
        ${installed ? 'Play' : owned ? 'Install demo' : 'Add to library'}
      </button>
      <div class="metric">
        ${glyph('time', 20)}<span><small>Last played</small><b>${dateLabel(s.last)}</b></span>
      </div>
      <div class="metric">
        ${glyph('pad', 21)}<span><small>Play time</small><b>${esc(playTime(s.seconds))}</b></span>
      </div>
      <div class="game-tools">
        ${owned
          ? `<button class="square favorite" aria-pressed="${fav}" data-action="favorite" data-id="${g.id}" aria-label="${fav ? 'Remove from favorites' : 'Add to favorites'}" title="${fav ? 'Remove from favorites' : 'Add to favorites'}">${glyph('star', 20)}</button>`
          : ''}<button
          class="square"
          data-action="info"
          data-id="${g.id}"
          aria-label="Game information"
          title="Game information"
        >
          ${glyph('info', 20)}
        </button>
      </div>
    </div>
    <nav class="game-tabs" aria-label="Game sections">
      ${[
        ['activity', 'Activity'],
        ['guide', 'Game guide'],
        ['art', 'Artwork'],
      ]
        .map(
          ([key, label]) =>
            /* HTML */ `<button data-tab="${key}" ${tab === key ? 'aria-current="page"' : ''}>
              ${label}
            </button>`
        )
        .join('')}
    </nav>
    <div class="detail-body">
      ${tab === 'guide'
        ? guide(g)
        : tab === 'art'
          ? `<section><h2>Artwork</h2><button class="gallery" data-action="art" data-id="${g.id}" aria-label="Enlarge ${esc(g.title)} artwork"><img src="${g.art}" alt="${esc(g.title)} key art"></button><p class="credit">${esc(g.credit)}</p></section>`
          : activity(g)}
    </div>
  </article>`;
}
function home() {
  const owned = profile.owned.map((id) => gameById.get(id));
  const recent = owned
    .filter((g) => profile.stats[g.id]?.last)
    .sort((a, b) => profile.stats[b.id].last - profile.stats[a.id].last);
  return /* HTML */ `<div class="page home-page">
    <h1>Library home</h1>
    <div class="shelf-heading"><h2>Game guides</h2></div>
    <div class="news-grid">
      ${['signal', 'drift', 'coast']
        .map((id) => {
          const g = gameById.get(id);
          return /* HTML */ `<button class="news-card" data-guide="${id}">
            <img src="${id === 'signal' ? 'media/goldstone.webp' : g.art}" alt="" /><span
              ><small>${esc(g.title)}</small><strong>${esc(guides[id])}</strong></span
            >
          </button>`;
        })
        .join('')}
    </div>
    ${recent.length
      ? `<div class="shelf-heading"><h2>Recently played</h2></div>${shelf(recent, 'Recently played')}`
      : ''}
    <div class="shelf-heading">
      <h2>All games <span>${owned.length}</span></h2>
    </div>
    ${shelf(owned, 'All games')}
  </div>`;
}
function collections() {
  const groups = [
    ['all', 'All games', profile.owned],
    ['fav', 'Favorites', profile.favorites],
    ['ready', 'Ready to play', profile.owned.filter((id) => profile.packs[id])],
  ];
  return /* HTML */ `<div class="page">
    <h1>Collections</h1>
    <div class="collection-grid">
      ${groups
        .map(
          ([key, name, ids]) =>
            /* HTML */ `<button class="collection-card" data-go="collection/${key}">
              <span class="collection-covers"
                >${ids
                  .slice(0, 3)
                  .map((id) => /* HTML */ `<img src="${gameById.get(id).cover}" alt="" />`)
                  .join('')}</span
              ><strong>${name}</strong
              ><small>${ids.length} ${ids.length === 1 ? 'game' : 'games'}</small>
            </button>`
        )
        .join('')}
    </div>
  </div>`;
}
function collection(key) {
  const [title, ids] =
    key === 'fav'
      ? ['Favorites', profile.favorites]
      : key === 'ready'
        ? ['Ready to play', profile.owned.filter((id) => profile.packs[id])]
        : ['All games', profile.owned];
  return /* HTML */ `<div class="page">
    <button class="crumb" data-go="collections">Collections ${glyph('chevron', 12)}</button>
    <h1>${title}</h1>
    ${ids.length
      ? shelf(
          ids.map((id) => gameById.get(id)),
          title
        )
      : '<p class="empty">Use the star on a game page to add it to Favorites.</p>'}
  </div>`;
}
function store() {
  return /* HTML */ `<div class="page">
    <h1>Store</h1>
    <div class="store-list">
      ${games
        .map(
          (g) =>
            /* HTML */ `<button class="store-game" data-go="game/${g.id}">
              <img src="${g.art}" alt="" /><span
                ><strong>${esc(g.title)}</strong><small>${esc(g.genre)}</small></span
              ><b>${profile.owned.includes(g.id) ? 'In library' : 'Free demo'}</b>${glyph(
                'chevron',
                15
              )}
            </button>`
        )
        .join('')}
    </div>
  </div>`;
}
function downloads() {
  const installs = profile.events.filter((e) => e.kind === 'install');
  return /* HTML */ `<div class="page">
    <h1>Downloads</h1>
    <div class="shelf-heading">
      <h2>Completed installs <span>${installs.length}</span></h2>
    </div>
    ${installs.length
      ? `<div class="store-list">${installs
          .map((e) => {
            const g = gameById.get(e.id);
            return /* HTML */ `<button class="store-game" data-go="game/${g.id}">
              <img src="${g.art}" alt="" /><span
                ><strong>${esc(g.title)}</strong><small>Bundled radio demo</small></span
              ><b>${Number(e.bytes) || 0} bytes</b><time>${dateLabel(e.at)}</time>
            </button>`;
          })
          .join('')}</div>`
      : `<div class="empty">${glyph('download', 29)}<h2>No recent installs</h2><p>Choose Install demo on a game page to load its bundled radio pack.</p><button class="secondary" data-go="store">Browse games</button></div>`}
  </div>`;
}

export { glyph, sidebar, detail, home, collections, collection, store, downloads };
