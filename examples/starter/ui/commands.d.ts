// Generated from commands.json.
import type { RustyBoxClient, InvokeOptions } from './sdk/rustybox.js';

export interface Greeting {
  name: string;
}

export interface Message {
  text: string;
}

export interface AppCommands {
  greet(params: Greeting, options?: InvokeOptions): Promise<Message>;
}
export function bindCommands(client: RustyBoxClient): AppCommands;
