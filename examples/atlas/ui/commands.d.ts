// Generated from commands.json.
import type { RustyBoxClient, InvokeOptions } from './sdk/rustybox.js';

export interface Empty {
}

export interface Asset {
  id: string;
  title: string;
  fileName: string;
  location: string;
  credit: string;
  thumbnail: string | null;
  width: number;
  height: number;
  bytes: number;
}

export interface Selection {
  id: string;
}

export interface AppCommands {
  catalog(params?: Empty, options?: InvokeOptions): Promise<Array<Asset>>;
  selectAsset(params: Selection, options?: InvokeOptions): Promise<Asset>;
  importImage(params?: Empty, options?: InvokeOptions): Promise<Asset | null>;
}
export function bindCommands(client: RustyBoxClient): AppCommands;
