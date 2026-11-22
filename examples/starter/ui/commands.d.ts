// Generated from commands.json.
import type { RustyBoxClient, InvokeOptions } from './sdk/rustybox.js';

export interface Greeting {
  name: string;
}

export interface Message {
  text: string;
}

export interface OpenedItem {
  title: string;
  text: string;
  url: string;
}

export interface Empty {
}

export interface AppCommands {
  greet(params: Greeting, options?: InvokeOptions): Promise<Message>;
  openedItems(params?: Empty, options?: InvokeOptions): Promise<Array<OpenedItem>>;
}
export function bindCommands(client: RustyBoxClient): AppCommands;
