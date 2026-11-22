import { rustybox } from './sdk/rustybox.js';
document.querySelector('form').onsubmit = async (event) => {
  event.preventDefault();
  const output = document.querySelector('output');
  try {
    const result = await rustybox.invoke('host.greet', {
      name: document.querySelector('#name').value,
    });
    output.textContent = result.text;
  } catch (error) {
    output.textContent = error.message;
  }
};
