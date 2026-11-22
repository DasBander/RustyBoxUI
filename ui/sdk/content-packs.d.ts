export interface ContentKeys {
  publisher: JsonWebKey;
  contentKey: string;
}
export class ContentPacks {
  constructor(namespace: string, client: ContentKeys);
  installed(id: string): Promise<number | null>;
  install(
    id: string,
    manifestURL: string | URL,
    options?: {
      signal?: AbortSignal;
      onProgress?: (bytes: { verified: number; total: number }) => void;
    }
  ): Promise<number>;
  read(id: string, name: string, type?: string): Promise<Blob>;
  remove(id: string): Promise<void>;
}
