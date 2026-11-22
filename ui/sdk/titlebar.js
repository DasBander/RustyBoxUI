import { rustybox } from './rustybox.js';

// Current WebView2 handles CSS caption regions before they reach the DOM.
// Older runtimes deliver ordinary pointer events to the same markup.
if (rustybox.isNative) {
  const controls =
    '[data-rb-no-drag], a, button, input, label, select, textarea, [contenteditable], [role="button"]';
  const isCaption = (event) =>
    event.target instanceof Element &&
    event.target.closest('[data-rb-drag]') &&
    !event.target.closest(controls);
  let press;
  const clear = () => {
    press = undefined;
  };

  document.addEventListener('pointerdown', (event) => {
    clear();
    if (event.button === 0 && event.pointerType === 'mouse' && isCaption(event)) {
      press = { x: event.screenX, y: event.screenY };
    }
  });
  document.addEventListener('pointermove', (event) => {
    if (!press || event.pointerType !== 'mouse') return;
    if (event.buttons !== 1) {
      clear();
      return;
    }
    if (Math.max(Math.abs(event.screenX - press.x), Math.abs(event.screenY - press.y)) < 4) return;
    clear();
    rustybox.invoke('window.drag').catch(console.error);
  });
  document.addEventListener('pointerup', clear);
  document.addEventListener('pointercancel', clear);
  window.addEventListener('blur', clear);
  document.addEventListener('dblclick', (event) => {
    if (event.button === 0 && isCaption(event))
      rustybox.invoke('window.maximize').catch(console.error);
  });
}
