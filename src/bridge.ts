import type { Snapshot } from './types';

declare global {
  interface Window {
    __TAURI_INTERNALS__?: {
      invoke<T>(command: string, args?: Record<string, unknown>): Promise<T>;
      transformCallback(callback: (event: { payload: unknown }) => void, once?: boolean): number;
      unregisterCallback(id: number): void;
    };
  }
}

export const isDesktop = !!window.__TAURI_INTERNALS__;

// Browser previews deliberately do not pretend to start real services.
export const preview: Snapshot = {
  ports: { website: 8800, phpmyadmin: 8801, mysql: 13306, php: 19000 },
  services: [
    { id: 'nginx', running: false, ready: false, port: 8800, pid: null, message: null },
    { id: 'php', running: false, ready: false, port: 19000, pid: null, message: null },
    { id: 'mysql', running: false, ready: false, port: 13306, pid: null, message: null },
    { id: 'phpmyadmin', running: false, ready: false, port: 8801, pid: null, message: null },
  ],
  documentRoot: '~/fairway/www', dataDir: 'Fairway XMP application data',
  runtimeReady: false, runtimeVersions: null, pathInstalled: false,
  pathPhp: null, pathComposer: null, version: '0.1.0-alpha.1',
};

export function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!window.__TAURI_INTERNALS__) return Promise.reject(new Error('Open the Fairway XMP desktop app to use this control.'));
  return window.__TAURI_INTERNALS__.invoke<T>(command, args);
}

export async function listen(event: string, callback: (payload: unknown) => void): Promise<() => void> {
  const api = window.__TAURI_INTERNALS__;
  if (!api) return () => {};
  const handler = api.transformCallback(e => callback(e.payload));
  const eventId = await api.invoke<number>('plugin:event|listen', { event, target: { kind: 'Any' }, handler });
  return () => {
    void api.invoke('plugin:event|unlisten', { event, eventId }).catch(() => {});
    api.unregisterCallback(handler);
  };
}
