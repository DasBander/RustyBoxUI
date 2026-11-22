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
    workspace(params = {}, options) {
      return client.invoke('host.workspace', params, options);
    },
    page(params, options) {
      return client.invoke('host.page', params, options);
    },
    thumbnails(params, options) {
      return client.invoke('host.thumbnails', params, options);
    },
    renderAsset(params, options) {
      return client.invoke('host.renderAsset', params, options);
    },
  };
}
