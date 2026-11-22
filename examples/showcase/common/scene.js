import { rustybox, showError, ready } from './chrome.js';
import './sdk/components.js';

// Send the newest controls when the preceding GPU draw has completed.
export function bindScene(element, values) {
  let attached = false,
    dirty = false,
    drawing = false;
  async function refresh() {
    dirty = true;
    if (!attached || drawing) return;
    drawing = true;
    try {
      while (dirty && attached) {
        dirty = false;
        const { width, height } = element.getBoundingClientRect();
        if (!width || !height) break;
        const scale = Math.min(devicePixelRatio, 1800 / width, 1152 / height);
        await rustybox.invoke('host.scene', {
          width: Math.max(1, Math.round(width * scale)),
          height: Math.max(1, Math.round(height * scale)),
          ...values(),
        });
        if (document.body.dataset.ready !== 'true') ready();
      }
    } catch (e) {
      showError(e);
    } finally {
      drawing = false;
    }
  }
  element.addEventListener('native-ready', () => {
    attached = true;
    refresh();
  });
  element.addEventListener('native-error', (e) => {
    attached = false;
    showError(e.detail);
  });
  element.client = rustybox;
  element.sync();
  return refresh;
}
