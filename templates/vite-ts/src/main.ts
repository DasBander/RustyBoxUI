import '../../ui/starter.css';
import '../../ui/theme.css';
import '../../ui/sdk/titlebar.js';
import { rustybox } from '../../ui/sdk/rustybox.js';
import { bindCommands } from '../../ui/commands.js';

const commands = bindCommands(rustybox);
const close = document.querySelector<HTMLButtonElement>('#close')!;
close.hidden = !rustybox.isNative;
close.onclick = () => {
  void rustybox.invoke('window.close');
};
document.querySelector('form')!.onsubmit = async (event) => {
  event.preventDefault();
  const name = document.querySelector<HTMLInputElement>('#name')!.value;
  const output = document.querySelector('output')!;
  try {
    output.textContent = rustybox.isNative
      ? (await commands.greet({ name })).text
      : 'Hello, ' + name + '.';
  } catch (error) {
    output.textContent = String(error);
  }
};
