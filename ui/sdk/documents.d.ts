import type { RustyBoxClient } from './rustybox.js';
export class DocumentDraft<T> {
  constructor(name: string);
  read(): Promise<{ id: string; name: string; source: Blob; edit: T } | null>;
  replace(name: string, source: Blob, edit: T): Promise<void>;
  update(edit: T): void;
}
export function saveDocument(client: RustyBoxClient, blob: Blob, name: string): Promise<boolean>;
