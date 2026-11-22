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

export interface PageRequest {
  offset: number;
  limit: number;
}

export interface Workspace {
  role: string;
  folder: string | null;
  selected: string;
  count: number;
  running: boolean;
  skipped: number;
  generation: number;
}

export interface RenderRequest {
  id: string;
  viewport: number;
}

export interface Thumbnail {
  id: string;
  url: string | null;
}

export interface Ids {
  ids: Array<string>;
}

export interface AppCommands {
  catalog(params?: Empty, options?: InvokeOptions): Promise<Array<Asset>>;
  selectAsset(params: Selection, options?: InvokeOptions): Promise<Asset>;
  importImage(params?: Empty, options?: InvokeOptions): Promise<Asset | null>;
  workspace(params?: Empty, options?: InvokeOptions): Promise<Workspace>;
  page(params: PageRequest, options?: InvokeOptions): Promise<Array<Asset>>;
  thumbnails(params: Ids, options?: InvokeOptions): Promise<Array<Thumbnail>>;
  renderAsset(params: RenderRequest, options?: InvokeOptions): Promise<Asset>;
}
export function bindCommands(client: RustyBoxClient): AppCommands;
