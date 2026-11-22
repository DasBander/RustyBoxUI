// Desktop controls share ordinary CSS variables and expose keyboard focus through ARIA.
let nextId = 0;
function element(tag, role, text) {
  const node = document.createElement(tag);
  if (role) node.setAttribute('role', role);
  if (text !== undefined) node.textContent = text;
  return node;
}
function selected(target, value) {
  target.dispatchEvent(new CustomEvent('selection-change', { bubbles: true, detail: value }));
}
export class RBTable extends HTMLElement {
  #rows = [];
  #columns = [];
  #row = 0;
  #col = 0;
  #offset = -1;
  #serial = ++nextId;
  #height = 36;
  #body;
  #header;
  #observer;
  connectedCallback() {
    if (!this.#body) {
      this.setAttribute('role', 'grid');
      if (!this.hasAttribute('tabindex')) this.tabIndex = 0;
      this.#header = element('div', 'row');
      this.#header.className = 'rb-table-head';
      this.#body = element('div', 'rowgroup');
      this.#body.className = 'rb-table-body';
      this.replaceChildren(this.#header, this.#body);
      this.addEventListener('scroll', () => this.#draw());
      this.addEventListener('keydown', (e) => this.#key(e));
      this.addEventListener('click', (e) => {
        const cell = e.target.closest('[data-rb-row]');
        if (cell) {
          this.focus({ preventScroll: true });
          this.#select(+cell.dataset.rbRow, +cell.dataset.rbCol, true);
        }
      });
    }
    this.#observer = new ResizeObserver(() => {
      const height = this.#header.getBoundingClientRect().height;
      if (height > 0 && this.#height !== height) {
        this.#height = height;
        this.#body.style.height = this.#rows.length * height + 'px';
      }
      this.#draw(true);
    });
    this.#observer.observe(this.#header);
    this.#observer.observe(this);
    this.#render();
  }
  disconnectedCallback() {
    this.#observer?.disconnect();
  }
  set columns(value) {
    this.#columns = Array.from(value);
    this.#col = Math.max(0, Math.min(this.#col, this.#columns.length - 1));
    this.#render();
  }
  get columns() {
    return this.#columns;
  }
  set rows(value) {
    const id = this.#rows[this.#row]?.id;
    this.#rows = Array.from(value);
    this.#row = Math.max(
      0,
      this.#rows.findIndex((r) => r.id === id)
    );
    this.#render();
  }
  get rows() {
    return this.#rows;
  }
  selectId(id) {
    const i = this.#rows.findIndex((r) => r.id === id);
    if (i >= 0 && i !== this.#row) this.#select(i, this.#col, false);
  }
  #render() {
    if (!this.#body) return;
    this.setAttribute('aria-rowcount', String(this.#rows.length + 1));
    this.setAttribute('aria-colcount', String(this.#columns.length));
    this.style.setProperty('--rb-columns', this.#columns.map(() => 'minmax(0,1fr)').join(' '));
    this.#header.replaceChildren(
      ...this.#columns.map((c) => element('span', 'columnheader', c.label))
    );
    this.#header.setAttribute('aria-rowindex', '1');
    this.#body.style.height = this.#rows.length * this.#height + 'px';
    this.#draw(true);
  }
  #select(row, col, emit) {
    if (!this.#rows.length || !this.#columns.length) return;
    this.#row = Math.max(0, Math.min(row, this.#rows.length - 1));
    this.#col = Math.max(0, Math.min(col, this.#columns.length - 1));
    const top = this.#row * this.#height,
      height = this.clientHeight - this.#header.offsetHeight;
    if (top < this.scrollTop) this.scrollTop = top;
    if (top + this.#height > this.scrollTop + height)
      this.scrollTop = Math.max(0, top + this.#height - height);
    this.#draw(true);
    if (emit) selected(this, this.#rows[this.#row]);
  }
  #key(e) {
    if (e.isComposing || e.altKey || e.metaKey || e.shiftKey || e.target !== this) return;
    const keys = {
      ArrowDown: [1, 0],
      ArrowUp: [-1, 0],
      ArrowLeft: [0, -1],
      ArrowRight: [0, 1],
      PageDown: [Math.max(1, Math.floor(this.clientHeight / this.#height) - 1), 0],
      PageUp: [-Math.max(1, Math.floor(this.clientHeight / this.#height) - 1), 0],
    };
    if (e.key === 'Home' || e.key === 'End') {
      e.preventDefault();
      const last = e.key === 'End';
      this.#select(
        e.ctrlKey ? (last ? this.#rows.length - 1 : 0) : this.#row,
        last ? this.#columns.length - 1 : 0,
        e.ctrlKey
      );
    } else if (!e.ctrlKey && keys[e.key]) {
      e.preventDefault();
      const [r, c] = keys[e.key];
      this.#select(this.#row + r, this.#col + c, r !== 0);
    } else if (e.key === 'Enter' && this.#rows.length) {
      e.preventDefault();
      selected(this, this.#rows[this.#row]);
    }
  }
  #draw(force = false) {
    const first = Math.max(0, Math.floor(this.scrollTop / this.#height) - 3);
    if (first === this.#offset && !force) return;
    this.#offset = first;
    const last = Math.min(
      this.#rows.length,
      first + Math.ceil(this.clientHeight / this.#height) + 7
    );
    const fragment = document.createDocumentFragment();
    // Keep the selected cell in the accessibility tree when the user scrolls it outside the visible rows.
    const indices = Array.from({ length: Math.max(0, last - first) }, (_, index) => first + index);
    if (this.#rows.length && this.#row < first) indices.unshift(this.#row);
    else if (this.#rows.length && this.#row >= last) indices.push(this.#row);
    let active;
    for (const i of indices) {
      const row = element('div', 'row');
      row.className = 'rb-table-row';
      row.style.top = i * this.#height + 'px';
      row.setAttribute('aria-rowindex', String(i + 2));
      row.setAttribute('aria-selected', String(i === this.#row));
      this.#columns.forEach((col, j) => {
        const cell = element('span', 'gridcell');
        cell.id = 'rb-cell-' + this.#serial + '-' + i + '-' + j;
        cell.dataset.rbRow = i;
        cell.dataset.rbCol = j;
        cell.setAttribute('aria-colindex', String(j + 1));
        const value = this.#rows[i][col.key];
        const rendered = col.render?.(value, this.#rows[i]) ?? String(value ?? '');
        cell.append(rendered);
        row.append(cell);
        if (i === this.#row && j === this.#col) {
          active = cell.id;
          cell.toggleAttribute('data-active', true);
        }
      });
      fragment.append(row);
    }
    this.#body.replaceChildren(fragment);
    if (active) this.setAttribute('aria-activedescendant', active);
    else this.removeAttribute('aria-activedescendant');
  }
}
export class RBTree extends HTMLElement {
  #items = [];
  #initialized = false;
  #search = '';
  #lastKey = 0;
  #open = new Set();
  #selected;
  #visible = [];
  #serial = ++nextId;
  connectedCallback() {
    if (!this.#initialized) {
      this.#initialized = true;
      this.setAttribute('role', 'tree');
      if (!this.hasAttribute('tabindex')) this.tabIndex = 0;
      this.addEventListener('keydown', (e) => this.#key(e));
      this.addEventListener('click', (e) => {
        const node = e.target.closest('[role=treeitem]');
        if (!node) return;
        this.focus();
        this.#choose(+node.dataset.index, true);
        if (this.#visible[+node.dataset.index].item.children?.length) this.#toggle();
      });
    }
    this.#draw();
  }
  set items(value) {
    this.#items = Array.from(value);
    this.#draw();
  }
  get items() {
    return this.#items;
  }
  #choose(i, emit) {
    const entry = this.#visible[i];
    if (!entry) return;
    this.#selected = entry.item.id;
    this.#draw();
    this.children[i]?.scrollIntoView({ block: 'nearest' });
    if (emit) selected(this, entry.item);
  }
  #toggle() {
    if (this.#open.has(this.#selected)) this.#open.delete(this.#selected);
    else this.#open.add(this.#selected);
    this.#draw();
  }
  #key(e) {
    const i = this.#visible.findIndex((x) => x.item.id === this.#selected),
      node = this.#visible[i];
    if (!node || e.isComposing || e.ctrlKey || e.metaKey || e.altKey) return;
    const last = this.#visible.length - 1;
    switch (e.key) {
      case 'ArrowDown':
        this.#choose(Math.min(last, i + 1), true);
        break;
      case 'ArrowUp':
        this.#choose(Math.max(0, i - 1), true);
        break;
      case 'Home':
        this.#choose(0, true);
        break;
      case 'End':
        this.#choose(last, true);
        break;
      case 'ArrowRight':
        if (node.item.children?.length) {
          if (this.#open.has(node.item.id)) this.#choose(i + 1, true);
          else this.#toggle();
        }
        break;
      case 'ArrowLeft':
        if (this.#open.has(node.item.id)) this.#toggle();
        else if (node.parent >= 0) this.#choose(node.parent, true);
        break;
      case 'Enter':
      case ' ':
        selected(this, node.item);
        break;
      default: {
        if (e.key.length !== 1) return;
        const now = performance.now();
        const letter = e.key.toLocaleLowerCase();
        this.#search = now - this.#lastKey < 700 ? this.#search + letter : letter;
        this.#lastKey = now;
        const prefix = [...this.#search].every((c) => c === letter) ? letter : this.#search;
        for (let offset = 1; offset <= this.#visible.length; offset++) {
          const next = (i + offset) % this.#visible.length;
          if (this.#visible[next].item.label.toLocaleLowerCase().startsWith(prefix)) {
            this.#choose(next, true);
            break;
          }
        }
      }
    }
    e.preventDefault();
  }
  #draw() {
    if (!this.isConnected) return;
    this.#visible = [];
    const fragment = document.createDocumentFragment();
    const walk = (items, depth, parent) => {
      items.forEach((item, index) => {
        const i = this.#visible.push({ item, parent }) - 1;
        const line = element('div', 'treeitem');
        line.id = 'rb-tree-' + this.#serial + '-' + i;
        line.dataset.index = i;
        line.style.paddingInlineStart = (depth - 1) * 16 + 6 + 'px';
        line.setAttribute('aria-level', depth);
        line.setAttribute('aria-posinset', index + 1);
        line.setAttribute('aria-setsize', items.length);
        if (item.children?.length)
          line.setAttribute('aria-expanded', String(this.#open.has(item.id)));
        line.setAttribute('aria-label', item.label);
        line.textContent = item.label;
        fragment.append(line);
        if (item.children?.length && this.#open.has(item.id)) walk(item.children, depth + 1, i);
      });
    };
    walk(this.#items, 1, -1);
    let i = this.#visible.findIndex((x) => x.item.id === this.#selected);
    if (i < 0) i = 0;
    this.#selected = this.#visible[i]?.item.id;
    Array.from(fragment.children).forEach((n, j) =>
      n.setAttribute('aria-selected', String(i === j))
    );
    this.replaceChildren(fragment);
    if (this.children[i]) this.setAttribute('aria-activedescendant', this.children[i].id);
    else this.removeAttribute('aria-activedescendant');
  }
}
export class RBSplit extends HTMLElement {
  connectedCallback() {
    this.setAttribute('role', 'separator');
    if (!this.hasAttribute('tabindex')) this.tabIndex = 0;
    const vertical = this.getAttribute('orientation') !== 'horizontal';
    this.setAttribute('aria-orientation', vertical ? 'vertical' : 'horizontal');
    const min = Number(this.getAttribute('min') ?? 15),
      max = Number(this.getAttribute('max') ?? 85);
    this.setAttribute('aria-valuemin', min);
    this.setAttribute('aria-valuemax', max);
    const set = (v) => {
      const n = Math.max(min, Math.min(max, v));
      this.setAttribute('aria-valuenow', Math.round(n));
      this.setAttribute('aria-valuetext', Math.round(n) + '%');
      this.parentElement.style.setProperty('--rb-split', n + '%');
    };
    set(Number(this.getAttribute('aria-valuenow') ?? this.getAttribute('value') ?? 50));
    let expanded = Number(this.getAttribute('aria-valuenow'));
    const move = (e) => {
      const b = this.parentElement.getBoundingClientRect();
      set(
        vertical ? ((e.clientX - b.left) * 100) / b.width : ((e.clientY - b.top) * 100) / b.height
      );
    };
    this.onpointerdown = (e) => {
      if (e.button !== 0) return;
      this.focus({ preventScroll: true });
      this.setPointerCapture(e.pointerId);
      move(e);
    };
    this.onpointermove = (e) => {
      if (this.hasPointerCapture(e.pointerId)) move(e);
    };
    this.onkeydown = (e) => {
      const dec = vertical ? 'ArrowLeft' : 'ArrowUp',
        inc = vertical ? 'ArrowRight' : 'ArrowDown';
      if (e.isComposing || e.altKey || e.ctrlKey || e.metaKey) return;
      if (e.key === 'Enter') {
        e.preventDefault();
        const current = Number(this.getAttribute('aria-valuenow'));
        if (current > min) {
          expanded = current;
          set(min);
        } else set(expanded > min ? expanded : max);
        return;
      }
      if (![dec, inc, 'Home', 'End'].includes(e.key)) return;
      e.preventDefault();
      set(
        e.key === 'Home'
          ? min
          : e.key === 'End'
            ? max
            : Number(this.getAttribute('aria-valuenow')) + (e.key === inc ? 2 : -2)
      );
    };
  }
}
for (const [name, type] of [
  ['rb-table', RBTable],
  ['rb-tree', RBTree],
  ['rb-split', RBSplit],
])
  if (!customElements.get(name)) customElements.define(name, type);
