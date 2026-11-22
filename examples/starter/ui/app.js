import './sdk/forms.js';
import './sdk/titlebar.js';
import { rustybox } from './sdk/rustybox.js';
import { bindCommands } from './commands.js';
const api = bindCommands(rustybox);
document.querySelector('form').onsubmit = async (event) => {
  event.preventDefault();
  try {
    const { text } = await api.greet({ name: new FormData(event.target).get('name') });
    document.querySelector('output').textContent = text;
  } catch (err) {
    document.querySelector('output').textContent = err.message;
  }
};
document.querySelector('#close').onclick = () => rustybox.invoke('window.close');

let activationRevision = 0;

async function showOpenedItems() {
  const revision = ++activationRevision;
  const items = await api.openedItems();
  if (revision !== activationRevision) return;
  const list = document.querySelector('#items');
  list.replaceChildren(
    ...items.map((item) => {
      const article = document.createElement('article');
      const title = document.createElement('h3');
      const content = document.createElement('p');
      title.textContent = item.title;
      content.textContent = item.url || item.text;
      article.append(title, content);
      return article;
    })
  );
  document.querySelector('#opened').hidden = items.length === 0;
  const last = items.at(-1);
  if (last?.url) {
    try {
      const link = new URL(last.url);
      if (link.hostname === 'greet' && link.searchParams.has('name')) {
        const name = link.searchParams.get('name').slice(0, 100);
        document.querySelector('input[name=name]').value = name;
        if (name.trim()) {
          const message = await api.greet({ name });
          if (revision === activationRevision)
            document.querySelector('output').textContent = message.text;
        }
      }
    } catch {
      /* Other registered app links are displayed as text. */
    }
  }
}
if (rustybox.isNative) {
  const refresh = () =>
    showOpenedItems().catch((error) => {
      document.querySelector('output').textContent = error.message;
    });
  rustybox.addEventListener('activation-changed', refresh);
  refresh();
}
