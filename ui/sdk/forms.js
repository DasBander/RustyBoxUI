// Progressive controls keep native text input, button and dialog semantics.
let sequence = 0;

// A composing input contains the IME's provisional text. Render once its edit is committed.
export function onCommittedInput(input, changed) {
  const controller = new AbortController();
  const options = { signal: controller.signal };
  let original = null;
  let tail;
  input.addEventListener(
    'compositionstart',
    () => {
      original = input.value;
      tail = undefined;
    },
    options
  );
  input.addEventListener(
    'input',
    (event) => {
      if (original !== null || event.isComposing) return;
      const duplicate = tail === input.value;
      tail = undefined;
      if (!duplicate) changed(input.value);
    },
    options
  );
  input.addEventListener(
    'compositionend',
    () => {
      const before = original;
      original = null;
      tail = input.value;
      if (input.value !== before) changed(input.value);
      // Some engines send a trailing input for the same edit.
      setTimeout(() => {
        tail = undefined;
      }, 0);
    },
    options
  );
  return () => controller.abort();
}

export class RBField extends HTMLElement {
  connectedCallback() {
    const input = this.querySelector('input, textarea, select');
    if (!input || this.querySelector('label')) return;
    if (!input.id) input.id = 'rb-field-' + ++sequence;
    const label = document.createElement('label');
    label.htmlFor = input.id;
    label.textContent = this.getAttribute('label') || 'Value';
    this.prepend(label);
  }
}
export class RBMenu extends HTMLElement {
  connectedCallback() {
    this.controller?.abort();
    this.controller = new AbortController();
    const opts = { signal: this.controller.signal };
    this.trigger = this.querySelector(':scope > button');
    this.panel = this.querySelector('[role="menu"]');
    if (!this.trigger || !this.panel) return;
    this.panel.id ||= 'rb-menu-' + ++sequence;
    this.trigger.setAttribute('aria-controls', this.panel.id);
    this.trigger.setAttribute('aria-haspopup', 'menu');
    if (!this.panel.hasAttribute('aria-label') && !this.panel.hasAttribute('aria-labelledby')) {
      this.trigger.id ||= 'rb-menu-trigger-' + ++sequence;
      this.panel.setAttribute('aria-labelledby', this.trigger.id);
    }
    this.close(false);
    this.trigger.addEventListener(
      'click',
      () => (this.panel.hidden ? this.open() : this.close()),
      opts
    );
    this.addEventListener(
      'keydown',
      (e) => {
        if (e.isComposing || e.keyCode === 229) return;
        const items = this.items();
        if (this.panel.hidden) {
          if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
            e.preventDefault();
            this.open(e.key === 'ArrowUp' ? items.at(-1) : items[0]);
          }
          return;
        }
        if (e.key === 'Escape') {
          e.preventDefault();
          this.close();
          return;
        }
        if (e.key === 'Tab') {
          this.close();
          return;
        }
        const index = items.indexOf(document.activeElement);
        const target = {
          ArrowDown: (index + 1) % items.length,
          ArrowUp: (index - 1 + items.length) % items.length,
          Home: 0,
          End: items.length - 1,
        }[e.key];
        if (target !== undefined) {
          e.preventDefault();
          items[target]?.focus();
        }
      },
      opts
    );
    this.panel.addEventListener(
      'click',
      (e) => {
        const item = e.target.closest('[role^="menuitem"]');
        if (!item || item.disabled) return;
        this.close();
        this.dispatchEvent(
          new CustomEvent('rb-select', { bubbles: true, detail: item.dataset.value })
        );
      },
      opts
    );
    document.addEventListener(
      'pointerdown',
      (e) => {
        if (!this.contains(e.target)) this.close(false);
      },
      opts
    );
    this.addEventListener(
      'focusout',
      (e) => {
        if (e.relatedTarget && !this.contains(e.relatedTarget)) this.close(false);
      },
      opts
    );
  }
  disconnectedCallback() {
    this.controller?.abort();
  }
  items() {
    return [...this.panel.querySelectorAll('[role^="menuitem"]:not(:disabled)')];
  }
  open(target) {
    for (const item of this.panel.querySelectorAll('[role^="menuitem"]')) item.tabIndex = -1;
    this.panel.hidden = false;
    this.trigger.setAttribute('aria-expanded', 'true');
    (target || this.panel.querySelector('[aria-checked="true"]') || this.items()[0])?.focus();
  }
  close(focus = true) {
    this.panel.hidden = true;
    this.trigger.setAttribute('aria-expanded', 'false');
    if (focus) this.trigger.focus();
  }
}
export class RBDialog extends HTMLElement {
  connectedCallback() {
    this.controller?.abort();
    this.controller = new AbortController();
    this.dialog = this.querySelector('dialog');
    this.dialog?.addEventListener('close', () => this.opener?.focus(), {
      signal: this.controller.signal,
    });
    this.addEventListener(
      'click',
      (e) => {
        if (e.target.closest('[data-dismiss]')) this.close();
      },
      { signal: this.controller.signal }
    );
  }
  disconnectedCallback() {
    this.controller?.abort();
  }
  open() {
    if (!this.dialog.open) {
      this.opener = document.activeElement;
      this.dialog.showModal();
    }
  }
  close() {
    if (this.dialog.open) this.dialog.close();
  }
}
for (const [name, component] of [
  ['rb-field', RBField],
  ['rb-menu', RBMenu],
  ['rb-dialog', RBDialog],
]) {
  if (!customElements.get(name)) customElements.define(name, component);
}
