/// <reference types="vite/client" />

declare module '*.vue' {
  import type { DefineComponent } from 'vue';
  const component: DefineComponent<Record<string, unknown>, Record<string, unknown>, unknown>;
  export default component;
}

interface Window {
  __img_mediaPort: number | null;
  __rail_mediaPort: number | null;
  __siegu_mediaPort: number | null;
  $scan: unknown;
  $photos: unknown;
  $status: unknown;
  L: typeof import('leaflet');
}
