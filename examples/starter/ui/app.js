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
