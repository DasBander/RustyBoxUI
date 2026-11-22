// Generated from commands.json.
export function bindCommands(client) {
  return {
    demoPack(params, options) {
      return client.invoke('host.demoPack', params, options);
    },
  };
}
