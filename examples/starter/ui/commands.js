// Generated from commands.json.
export function bindCommands(client) {
  return {
    greet(params, options) {
      return client.invoke('host.greet', params, options);
    },
    openedItems(params = {}, options) {
      return client.invoke('host.openedItems', params, options);
    },
  };
}
