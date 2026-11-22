import './forms.js';
export const escapeHTML = (value) =>
  String(value).replace(
    /[&<>"']/g,
    (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]
  );
// Keep the catalog's intentional line breaks intact when the CSS preserves whitespace.
export function gameWordmark(game) {
  const title = escapeHTML(game.wordmark || game.title);
  return game.subtitle ? title + `<small>${escapeHTML(game.subtitle)}</small>` : title;
}
const paths = {
  store: 'M3 9h18l-2-6H5L3 9Zm1 0v12h16V9M9 21v-8h6v8M3 9c0 4 6 4 6 0 0 4 6 4 6 0 0 4 6 4 6 0',
  library: 'M4 4h5v16H4zM12 4h3v16h-3zM18 4l3 1-1 15-3-1z',
  download: 'M12 3v12m-5-5 5 5 5-5M4 16v5h16v-5',
  heart:
    'M20.8 4.6a5.5 5.5 0 0 0-7.8 0L12 5.7l-1.1-1.1a5.5 5.5 0 0 0-7.8 7.8L12 21l8.8-8.6a5.5 5.5 0 0 0 0-7.8Z',
  settings:
    'M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8Zm-2-5h4l1 3 3-1 2 3-2 3 2 3-2 3-3-1-1 3h-4l-1-3-3 1-2-3 2-3-2-3 2-3 3 1 1-3Z',
  cube: 'm12 2 9 5v10l-9 5-9-5V7l9-5Zm0 10v10M3 7l9 5 9-5M7.5 4.5l9 5',
  search: 'm21 21-5-5M10.5 3a7.5 7.5 0 1 0 0 15 7.5 7.5 0 0 0 0-15Z',
  bell: 'M18 8a6 6 0 0 0-12 0c0 7-3 7-3 9h18c0-2-3-2-3-9M10 21h4',
  arrow: 'M4 12h16m-6-6 6 6-6 6',
  chevron: 'm9 5 7 7-7 7',
  plus: 'M12 5v14M5 12h14',
  check: 'm5 12 4 4L20 5',
  play: 'm8 4 13 8-13 8V4Z',
  pause: 'M8 5v14M16 5v14',
  close: 'm6 6 12 12M6 18 18 6',
  channels: 'm12 3 9 5-9 5-9-5 9-5Zm-9 9 9 5 9-5M3 16l9 5 9-5',
  grid: 'M3 3h7v7H3zM14 3h7v7h-7zM3 14h7v7H3zM14 14h7v7h-7z',
  bolt: 'm13 2-9 12h7l-1 8 10-13h-7l1-7Z',
  code: 'm8 5-7 7 7 7m8-14 7 7-7 7M14 2l-4 20',
};
export const icon = (name, size = 19) =>
  `<svg width="${size}" height="${size}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="${paths[name] || paths.cube}"/></svg>`;
class RBButton extends HTMLElement {
  static observedAttributes = ['disabled'];
  attributeChangedCallback() {
    const button = this.querySelector(':scope > button');
    if (button) button.disabled = this.hasAttribute('disabled');
  }
  connectedCallback() {
    if (this.querySelector(':scope > button')) return;
    const button = document.createElement('button');
    button.type = 'button';
    while (this.firstChild) button.append(this.firstChild);
    if (this.hasAttribute('disabled')) button.disabled = true;
    if (this.hasAttribute('label')) button.setAttribute('aria-label', this.getAttribute('label'));
    this.append(button);
  }
}
class RBToggle extends HTMLElement {
  static observedAttributes = ['checked'];
  attributeChangedCallback() {
    this.sync();
  }
  connectedCallback() {
    if (this.firstChild) return;
    this.innerHTML = '<button type="button" role="switch"></button>';
    this.firstChild.setAttribute('aria-label', this.getAttribute('label') || 'Toggle');
    this.sync();
    this.firstChild.addEventListener('click', () => {
      this.toggleAttribute('checked');
      this.sync();
      this.dispatchEvent(
        new CustomEvent('change', { bubbles: true, detail: this.hasAttribute('checked') })
      );
    });
  }
  sync() {
    this.firstChild?.setAttribute('aria-checked', String(this.hasAttribute('checked')));
  }
}
class RBGameCard extends HTMLElement {
  set game(game) {
    this._game = game;
    this.render();
  }
  get game() {
    return this._game;
  }
  render() {
    if (!this._game) return;
    const g = this._game;
    const title = escapeHTML(g.title);
    this.innerHTML = /* HTML */ `<article class="game-card" data-game="${escapeHTML(g.id)}">
      <div class="game-cover">
        <img src="${escapeHTML(g.cover || g.art)}" alt="" loading="lazy" />
        <div class="cover-wordmark" aria-hidden="true">${gameWordmark(g)}</div>
        <button class="cover-open" aria-label="Explore ${title}"></button>
        <button
          class="wishlist-button"
          aria-label="${g.wishlisted ? 'Remove from' : 'Add to'} wishlist: ${title}"
          aria-pressed="${Boolean(g.wishlisted)}"
        >
          ${icon(g.wishlisted ? 'check' : 'plus', 16)}
        </button>
      </div>
      <div class="game-meta">
        <div class="game-title">${title}</div>
        <div class="game-genre">${escapeHTML(g.genre || '')}</div>
        <div class="game-price">${g.owned ? 'In library' : 'Free demo'}</div>
      </div>
    </article>`;
    const open = () =>
      this.dispatchEvent(new CustomEvent('game-open', { detail: g.id, bubbles: true }));
    this.querySelector('.cover-open').addEventListener('click', open);
    this.querySelector('.wishlist-button').addEventListener('click', () =>
      this.dispatchEvent(new CustomEvent('game-wishlist', { detail: g.id, bubbles: true }))
    );
  }
}
class RBNativeView extends HTMLElement {
  get viewport() {
    return Number(this.getAttribute('viewport') ?? 0);
  }
  connectedCallback() {
    this._scroll = () => this.sync();
    this._observer = new ResizeObserver(this._scroll);
    this._observer.observe(this);
    window.addEventListener('resize', this._scroll);
    document.addEventListener('scroll', this._scroll, true);
    this.sync();
  }
  sync() {
    if (!this.client || !this.isConnected || this.hasAttribute('paused')) return;
    // Coalesce scroll and resize events while the native view acknowledges its last bounds.
    this._dirty = true;
    return (this._pending ??= this._syncLayout().finally(() => {
      this._pending = null;
    }));
  }
  async _syncLayout() {
    while (this._dirty && this.isConnected && !this.hasAttribute('paused')) {
      this._dirty = false;
      const r = this.getBoundingClientRect();
      if (r.width < 1 || r.height < 1) return;
      if (r.top < 0 || r.left < 0 || r.bottom > innerHeight || r.right > innerWidth) {
        await this.client.invoke('graphics.detach', { viewport: this.viewport }).catch(() => {});
        continue;
      }
      try {
        const info = await this.client.invoke('graphics.attach', {
          viewport: this.viewport,
          x: r.x,
          y: r.y,
          width: r.width,
          height: r.height,
          viewportWidth: innerWidth,
          viewportHeight: innerHeight,
        });
        if (info.layoutPending) {
          await new Promise((resolve) => setTimeout(resolve, 16));
          this._dirty = true;
        } else if (this.isConnected && !this._dirty) {
          this.dispatchEvent(new CustomEvent('native-ready', { detail: info, bubbles: true }));
        }
      } catch (error) {
        if (this.isConnected && !this._dirty) {
          this.dispatchEvent(new CustomEvent('native-error', { detail: error, bubbles: true }));
        }
      }
    }
  }
  disconnectedCallback() {
    this._dirty = false;
    this._observer?.disconnect();
    window.removeEventListener('resize', this._scroll);
    document.removeEventListener('scroll', this._scroll, true);
    this.client?.invoke('graphics.detach', { viewport: this.viewport }).catch(() => {});
  }
}
for (const [name, component] of [
  ['rb-button', RBButton],
  ['rb-toggle', RBToggle],
  ['rb-game-card', RBGameCard],
  ['rb-native-view', RBNativeView],
]) {
  if (!customElements.get(name)) customElements.define(name, component);
}
