/** Bind toolbar buttons and menu items to the same native actions as keyboard shortcuts. */
export async function bindActions(client, root = document) {
  let actions = [];
  let disposed = false;
  const refresh = async () => {
    const latest = await client.invoke('host.actions');
    if (disposed) return;
    actions = latest;
    for (const node of root.querySelectorAll('[data-rb-action]')) {
      const action = actions.find((a) => a.id === node.dataset.rbAction);
      node.disabled = !action?.enabled;
      node.setAttribute('aria-disabled', String(node.disabled));
      if (action?.shortcut)
        node.setAttribute('aria-keyshortcuts', action.shortcut.replace('Ctrl', 'Control'));
    }
  };
  const perform = async (id) => {
    await client.invoke('host.performAction', { id }, { timeout: 600000 });
    await refresh();
  };
  const report = (e) => root.dispatchEvent(new CustomEvent('action-error', { detail: e }));
  const click = (e) => {
    const button = e.target.closest?.('[data-rb-action]');
    if (button && !button.disabled) perform(button.dataset.rbAction).catch(report);
  };
  const key = (e) => {
    if (e.isComposing || e.repeat || e.defaultPrevented) return;
    const shortcut = [
      e.ctrlKey ? 'Ctrl' : '',
      e.altKey ? 'Alt' : '',
      e.shiftKey ? 'Shift' : '',
      e.metaKey ? 'Meta' : '',
      e.key,
    ]
      .filter(Boolean)
      .join('+')
      .toLowerCase();
    const action = actions.find((a) => a.enabled && a.shortcut.toLowerCase() === shortcut);
    if (action) {
      e.preventDefault();
      perform(action.id).catch(report);
    }
  };
  const update = () => refresh().catch(report);
  await refresh();
  root.addEventListener('click', click);
  root.addEventListener('keydown', key);
  client.addEventListener('actions-changed', update);
  return {
    refresh,
    perform,
    dispose() {
      disposed = true;
      root.removeEventListener('click', click);
      root.removeEventListener('keydown', key);
      client.removeEventListener('actions-changed', update);
    },
  };
}
