import { rustybox } from './sdk/rustybox.js';
import './sdk/titlebar.js';
export { rustybox };
export const $ = (selector) => document.querySelector(selector);
export function showError(error) {
  const node = $('#app-error');
  node.hidden = false;
  node.textContent = error.message || String(error);
}
const header = $('header[data-rb-drag]');
const brand = document.createElement('strong');
brand.className = 'brand';
brand.textContent = header.dataset.brand;
const context = document.createElement('span');
context.className = 'context';
context.textContent = header.dataset.context;
const controls = document.createElement('div');
controls.className = 'window-buttons';
controls.setAttribute('data-rb-no-drag', '');
for (const name of ['minimize', 'maximize', 'close']) {
  const b = document.createElement('button');
  b.dataset.window = name;
  b.setAttribute('aria-label', name[0].toUpperCase() + name.slice(1));
  b.title = b.getAttribute('aria-label');
  b.onclick = () => rustybox.invoke('window.' + name).catch(showError);
  controls.append(b);
}
header.append(brand, context, controls);
const notice = document.createElement('p');
notice.id = 'app-error';
notice.hidden = true;
notice.setAttribute('role', 'alert');
document.body.append(notice);
export function ready() {
  document.body.dataset.ready = 'true';
}
