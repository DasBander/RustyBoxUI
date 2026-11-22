// Generated from commands.json.
import type { RustyBoxClient, InvokeOptions } from './sdk/rustybox.js';

export interface Selection {
  id: string;
}

export interface Band {
  station: string;
  frequency: number;
}

export interface DemoPack {
  id: string;
  channels: Array<Band>;
}

export interface AppCommands {
  demoPack(params: Selection, options?: InvokeOptions): Promise<DemoPack>;
}
export function bindCommands(client: RustyBoxClient): AppCommands;
