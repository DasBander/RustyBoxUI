import type { RustyBoxClient } from './rustybox.js';

export interface ActionBinding {
  refresh(): Promise<void>;
  perform(id: string): Promise<void>;
  dispose(): void;
}

export function bindActions(
  client: RustyBoxClient,
  root?: Document | HTMLElement
): Promise<ActionBinding>;
