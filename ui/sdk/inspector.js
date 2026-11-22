// Loaded by the development host. Record timings and outcomes without collecting command payloads.
const entries = [];
let panel, list;
function render() {
  if (!panel?.open) return;
  list.replaceChildren(
    ...entries
      .slice()
      .reverse()
      .map(({ method, elapsed, error }) => {
        const line = document.createElement('li');
        line.textContent =
          method + ' · ' + elapsed.toFixed(1) + ' ms' + (error ? ' · ' + error : '');
        if (error) line.className = 'failed';
        return line;
      })
  );
}
globalThis[Symbol.for('rustybox.dev.inspect')] = (method, elapsed, error) => {
  entries.push({ method, elapsed, error });
  if (entries.length > 120) entries.shift();
  render();
};
document.addEventListener('keydown', (event) => {
  if (!(event.ctrlKey && event.shiftKey && event.key === 'F2')) return;
  event.preventDefault();
  if (panel?.open) {
    panel.close();
    return;
  }
  if (!panel) {
    const style = Object.assign(document.createElement('link'), {
      rel: 'stylesheet',
      href: new URL('./inspector.css', import.meta.url).href,
    });
    document.head.append(style);
    panel = document.createElement('dialog');
    panel.className = 'rb-inspector';
    panel.setAttribute('aria-label', 'Native command inspector');
    const title = document.createElement('h2');
    title.textContent = 'Native commands';
    const close = document.createElement('button');
    close.textContent = 'Close';
    close.onclick = () => panel.close();
    list = document.createElement('ol');
    panel.append(title, close, list);
    document.body.append(panel);
  }
  panel.showModal();
  render();
});
