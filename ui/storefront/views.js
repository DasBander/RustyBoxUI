import { icon, escapeHTML as esc, gameWordmark } from '../sdk/components.js';

export function heading(title, subtitle = '', right = '') {
  return /* HTML */ `<div class="page-heading">
    <div>
      <h1>${title}</h1>
      ${subtitle ? `<p>${subtitle}</p>` : ''}
    </div>
    ${right}
  </div>`;
}
export function featuredHero(game, wishlisted) {
  return /* HTML */ `<article class="hero" data-game="${game.id}" aria-labelledby="hero-title">
    <img class="hero-art" src="${game.art}" fetchpriority="high" alt="" />
    <div class="hero-copy">
      <h2 class="feature-logo" id="hero-title" aria-label="${esc(game.title)}">${gameWordmark(game)}</h2>
      <div class="hero-info">
        <p>${esc(game.description)}</p>
        <div class="hero-buttons">
          <rb-button data-open="${game.id}" label="View ${esc(game.title)}">View game</rb-button>
          <button class="hero-wishlist" data-wish="${game.id}" aria-pressed="${wishlisted}">
            ${icon(wishlisted ? 'check' : 'plus', 17)} Wishlist
          </button>
        </div>
      </div>
    </div>
  </article>`;
}
export function empty(title, copy, button, reset = 'browse') {
  return /* HTML */ `<div class="empty-state">
    ${icon('search', 35)}
    <h2>${title}</h2>
    <p>${copy}</p>
    <rb-button variant="secondary" data-reset="${reset}">${button}</rb-button>
  </div>`;
}
export function gameDetails(game, owned, installed, wishlisted) {
  const id = game.id;
  return /* HTML */ `<div class="dialog-art">
      <img src="${game.art}" alt="" />
      <button class="dialog-close" data-close-dialog aria-label="Close details">
        ${icon('close', 18)}
      </button>
    </div>
    <div class="dialog-body">
      <h2 id="dialog-title">${esc(game.title)}</h2>
      <div class="dialog-tags">
        <rb-badge tone="accent">Free demo</rb-badge>
        <rb-badge>${esc(game.genre)}</rb-badge>
      </div>
      <p>${esc(game.description)}</p>
      <div class="dialog-actions">
        <rb-button variant="accent" ${installed ? `data-play="${id}"` : `data-install="${id}"`}
          >${icon(installed ? 'play' : 'download', 15)}
          ${installed ? 'Play preview' : owned ? 'Install demo' : 'Add to library'}</rb-button
        >
        <rb-button variant="secondary" data-dialog-wish="${id}"
          >${icon(wishlisted ? 'check' : 'plus', 15)}
          ${wishlisted ? 'Wishlisted' : 'Wishlist'}</rb-button
        >
      </div>
      <p class="demo-note">Fictional game with a simulated download.</p>
      <p class="art-credit">${esc(game.credit)}</p>
    </div>`;
}

export function nativeLabPage(isNative) {
  return (
    heading('Native Lab') +
    /* HTML */ `<section class="native-panel">
        <div class="native-toolbar">
          <div class="native-title">
            ${icon('cube', 17)} Orbital <span class="native-caption">Procedural shader scene</span>
          </div>
          <rb-button small variant="secondary" id="native-pause">Pause renderer</rb-button>
        </div>
        <div class="native-stage">
          <div class="native-placeholder">
            ${icon('cube', 48)}
            <p id="native-message">
              ${isNative
                ? 'Initializing the native renderer…'
                : 'Open the Windows application to run Direct3D 11.'}
            </p>
          </div>
          <rb-native-view></rb-native-view>
        </div>
        <div class="native-metrics">
          <div class="metric"><small>Backend</small><strong id="native-backend">—</strong></div>
          <div class="metric"><small>Device</small><strong id="native-device">—</strong></div>
          <div class="metric"><small>Frames drawn</small><strong id="native-frames">0</strong></div>
          <div class="metric">
            <small>Bridge</small><strong>${isNative ? 'Rust DLL' : 'Browser preview'}</strong>
          </div>
        </div>
      </section>
      <p class="native-note">
        The launcher renders this scene in a native D3D11 child window. Open Atlas to try a host
        texture with HTML controls drawn over it.
      </p>
      <section class="bridge-console">
        <div>
          <h3>Bridge diagnostics</h3>
          <p>Send an echo request and measure the round trip.</p>
          <div class="bridge-output" id="bridge-output" aria-live="polite"></div>
        </div>
        <rb-button variant="secondary" small id="ping">Send request ${icon('arrow', 13)}</rb-button>
      </section>`
  );
}
