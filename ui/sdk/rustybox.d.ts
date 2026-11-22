export interface RuntimeInfo {
  name: string;
  version: string;
  abi: number;
  platform: string;
  engine: string;
  browserVersion?: string;
  graphicsAvailable: boolean;
  uptimeMs?: number;
  debug?: boolean;
  composition?: boolean;
}
export interface GraphicsInfo {
  active: boolean;
  backend?: 'Direct3D 11';
  hardware?: boolean;
  frames?: number;
  composed?: boolean;
  source?: 'scene' | 'hostTexture';
  bounds?: { x: number; y: number; width: number; height: number };
  layoutPending?: boolean;
}
export interface InvokeOptions {
  signal?: AbortSignal;
  timeout?: number;
}
export class RustyBoxClient extends EventTarget {
  readonly isNative: boolean;
  invoke(method: 'runtime.info', params?: {}, options?: InvokeOptions): Promise<RuntimeInfo>;
  invoke(
    method: 'graphics.info' | 'graphics.attach',
    params?: { x: number; y: number; width: number; height: number },
    options?: InvokeOptions
  ): Promise<GraphicsInfo>;
  invoke(
    method: 'graphics.view',
    params: { zoom: number; channel: 'rgb' | 'red' | 'green' | 'blue' | 'alpha'; motion: boolean },
    options?: InvokeOptions
  ): Promise<{ zoom: number; channel: string; motion: boolean }>;
  invoke<T = unknown>(method: string, params?: unknown, options?: InvokeOptions): Promise<T>;
  dispose(): void;
}
export const rustybox: RustyBoxClient;
export function createPreviewClient(): Pick<RustyBoxClient, 'isNative' | 'invoke'>;
