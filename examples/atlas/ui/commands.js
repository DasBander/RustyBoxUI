// Generated from commands.json.
export function bindCommands(client) {
  return {
    catalog(params = {}, options) {
      return client.invoke('host.catalog', params, options);
    },
    selectAsset(params, options) {
      return client.invoke('host.selectAsset', params, options);
    },
    importImage(params = {}, options) {
      return client.invoke('host.importImage', params, options);
    },
  };
}
