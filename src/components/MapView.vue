<template>
  <div style="height: 100vh; width: 100%; position: relative">
    <!-- Empty State Overlay -->
    <div v-if="!loading && mapPoints.length === 0" class="map-empty-state">
      <div
        class="d-flex flex-column align-center justify-center h-100 px-6 text-center animate-fade-in"
      >
        <v-icon size="48" color="rgba(var(--v-theme-on-surface), 0.7)" class="mb-4"
          >mdi-map-marker-off-outline</v-icon
        >
        <div class="text-h6 text-medium-emphasis font-weight-bold">{{ $t('map.no_photos') }}</div>
        <p class="text-body-2 text-disabled mt-1 max-w-400">{{ $t('map.no_photos_desc') }}</p>
      </div>
    </div>

    <!-- Loading Overlay -->
    <div v-if="loading" class="map-empty-state">
      <div class="d-flex flex-column align-center justify-center h-100">
        <v-progress-circular
          indeterminate
          color="rgba(var(--v-theme-on-surface), 0.7)"
          size="32"
          width="3"
        ></v-progress-circular>
      </div>
    </div>

    <!-- Date Range Filter Chip -->
    <v-chip
      v-if="hasFilter"
      class="filter-chip"
      closable
      color="primary"
      variant="tonal"
      @click:close="clearFilter"
    >
      <v-icon start size="16">mdi-calendar-outline</v-icon>
      {{ filterLabel }}
    </v-chip>

    <l-map
      v-model:zoom="zoom"
      :center="initialCenter"
      @ready="onMapReady"
      :minZoom="2"
      :options="{ zoomControl: false, attributionControl: false, preferCanvas: true }"
      style="height: 100%; width: 100%; background: rgb(var(--v-theme-surface))"
      class="light-map"
    >
      <l-tile-layer
        :url="tileUrl"
        layer-type="base"
        name="Base Map"
        :options="{
          updateWhenZooming: false,
          updateWhenIdle: true,
          keepBuffer: 2,
          crossOrigin: true,
        }"
      />
    </l-map>

    <MediaViewer
      v-model="viewerOpen"
      :photos="viewerPhotos"
      v-model:index="currentPhotoIndex"
      @update:photo="handlePhotoUpdated"
    />
  </div>
</template>

<script setup lang="ts">
import 'leaflet/dist/leaflet.css';
import { LMap, LTileLayer } from '@vue-leaflet/vue-leaflet';
import L from 'leaflet';
if (typeof window !== 'undefined') {
  window.L = L;
}
import 'leaflet.markercluster';
import { ref, computed, nextTick, watch, onMounted, onUnmounted } from 'vue';
import { useI18n } from 'vue-i18n';
import { invoke } from '@/services/invoke';
import type { Map as LeafletMap } from 'leaflet';
import MediaViewer from './MediaViewer.vue';
import { useMediaUrl } from '@/composables/useMediaUrl';
import { useMapFilterStore } from '@/stores/mapFilter';
import type { MediaItem } from '@/types/media';

interface MapPoint {
  id: number;
  latitude: number;
  longitude: number;
  location: string;
  created?: string;
}

const zoom = ref(2);
const initialCenter: [number, number] = [20, 0];
const mapPoints = ref<MapPoint[]>([]);
const loading = ref(true);
const viewerOpen = ref(false);
const viewerPhotos = ref<MediaItem[]>([]);
const currentPhotoIndex = ref(0);

const CARTO_TILE_URL = 'https://{s}.basemaps.cartocdn.com/light_all/{z}/{x}/{y}{r}.png';
const DEFAULT_TILE_URL = 'https://tile.openstreetmap.org/{z}/{x}/{y}.png';
const THUMB_ZOOM = 14;

const tileUrl = ref(DEFAULT_TILE_URL);

function buildTileUrl(config: Record<string, string>): string {
  const custom = (config['map_tile_url'] ?? '').trim();
  const key = (config['map_tile_key'] ?? '').trim();
  let url = custom || (key ? CARTO_TILE_URL : DEFAULT_TILE_URL);
  if (key && !url.includes('key=')) {
    url += `${url.includes('?') ? '&' : '?'}key=${encodeURIComponent(key)}`;
  }
  return url;
}

async function loadTileConfig(): Promise<void> {
  try {
    const raw = await invoke<string>('get_config');
    tileUrl.value = buildTileUrl(JSON.parse(raw) as Record<string, string>);
  } catch (e) {
    console.error('[MapView] Failed to load tile config', e);
  }
}

const { mediaSrc } = useMediaUrl();
const mapFilterStore = useMapFilterStore();
const { t: $t } = useI18n();

let leafletMap: LeafletMap | null = null;
let clusterGroup: L.MarkerClusterGroup | null = null;
let mapClickHandler: ((e: L.LeafletMouseEvent) => void) | null = null;

function resolveThemeColor(varName: string): string {
  const raw = getComputedStyle(document.documentElement).getPropertyValue(varName).trim();
  if (!raw) return '#3b82f6';
  if (raw.startsWith('#')) return raw;
  const m = raw.match(/[\d.]+/g);
  if (m && m.length >= 3) return `rgb(${m[0]}, ${m[1]}, ${m[2]})`;
  return '#3b82f6';
}

function themeTextColor(): string {
  const bg = resolveThemeColor('--v-theme-info');
  const m = bg.match(/[\d.]+/g);
  if (!m || m.length < 3) return '#fff';
  const lum = (0.299 * +m[0] + 0.587 * +m[1] + 0.114 * +m[2]) / 255;
  return lum > 0.6 ? '#1a1a2e' : '#ffffff';
}

function makeDotIcon(): L.DivIcon {
  return L.divIcon({
    className: 'photo-marker',
    html: '<div class="pm-dot"></div>',
    iconSize: L.point(14, 14),
    iconAnchor: L.point(7, 7),
  });
}

function makeLoadingIcon(): L.DivIcon {
  return L.divIcon({
    className: 'photo-marker',
    html: '<div class="pm-loading"></div>',
    iconSize: L.point(40, 40),
    iconAnchor: L.point(20, 20),
  });
}

function makeThumbIcon(url: string): L.DivIcon {
  return L.divIcon({
    className: 'photo-marker',
    html: `<img src="${url}" class="pm-img" decoding="async" loading="lazy">`,
    iconSize: L.point(44, 44),
    iconAnchor: L.point(22, 22),
  });
}

let refreshTimer: ReturnType<typeof setTimeout> | null = null;

function refreshVisibleMarkers(): void {
  if (!leafletMap || refreshTimer) return;
  refreshTimer = setTimeout(() => {
    refreshTimer = null;
    if (!leafletMap || !clusterGroup) return;
    const useThumbs = leafletMap.getZoom() >= THUMB_ZOOM;
    const bounds = leafletMap.getBounds().pad(0.25);
    clusterGroup.eachLayer((layer) => {
      if (!(layer instanceof L.Marker)) return;
      const m = layer as L.Marker & { _p?: MapPoint; _thumbLoaded?: boolean };
      if (!m._p) return;
      if (!useThumbs) {
        if (m._thumbLoaded) {
          m._thumbLoaded = false;
          m.setIcon(makeDotIcon());
        }
        return;
      }
      if (m._thumbLoaded) return;
      if (!bounds.contains(m.getLatLng())) return;
      m._thumbLoaded = true;
      m.setIcon(makeLoadingIcon());
      void mediaSrc(
        { id: m._p.id, location: m._p.location, encoded: null } as unknown as MediaItem,
        'thumb',
      ).then((url) => {
        if (url && m._thumbLoaded) m.setIcon(makeThumbIcon(url));
      });
    });
  }, 150);
}

const GRID_CELL_DEG = 1;
const pointGrid = new Map<string, MapPoint[]>();

function buildPointGrid(points: MapPoint[]): void {
  pointGrid.clear();
  for (const p of points) {
    const key = `${Math.floor(p.latitude / GRID_CELL_DEG)}:${Math.floor(p.longitude / GRID_CELL_DEG)}`;
    const cell = pointGrid.get(key);
    if (cell) cell.push(p);
    else pointGrid.set(key, [p]);
  }
}

function squaredDist(a: MapPoint, lat: number, lng: number): number {
  return Math.pow(a.latitude - lat, 2) + Math.pow(a.longitude - lng, 2);
}

function nearestPoints(lat: number, lng: number, count: number): MapPoint[] {
  const cx = Math.floor(lat / GRID_CELL_DEG);
  const cy = Math.floor(lng / GRID_CELL_DEG);
  for (let radius = 0; radius < 8; radius++) {
    const candidates: MapPoint[] = [];
    for (let dx = -radius; dx <= radius; dx++) {
      for (let dy = -radius; dy <= radius; dy++) {
        if (Math.max(Math.abs(dx), Math.abs(dy)) !== radius) continue;
        const cell = pointGrid.get(`${cx + dx}:${cy + dy}`);
        if (cell) candidates.push(...cell);
      }
    }
    if (candidates.length >= count) {
      return candidates
        .map((p) => ({ p, d: squaredDist(p, lat, lng) }))
        .sort((a, b) => a.d - b.d)
        .slice(0, count)
        .map((x) => x.p);
    }
  }
  return mapPoints.value
    .map((p) => ({ p, d: squaredDist(p, lat, lng) }))
    .sort((a, b) => a.d - b.d)
    .slice(0, count)
    .map((x) => x.p);
}

async function openCarouselForIds(ids: number[]) {
  if (ids.length === 0) return;
  try {
    const photosJson = await invoke<string>('get_photos_by_ids', { ids });
    viewerPhotos.value = JSON.parse(photosJson);
    if (viewerPhotos.value.length > 0) {
      currentPhotoIndex.value = 0;
      viewerOpen.value = true;
    }
  } catch (e) {
    console.error('Failed to load photo details', e);
  }
}

function onMapClick(e: L.LeafletMouseEvent): void {
  const nearest = nearestPoints(e.latlng.lat, e.latlng.lng, 50);
  if (nearest.length === 0) return;
  const ids = nearest.map((p) => p.id);
  openCarouselForIds(ids);
}

async function openCarouselForPoint(point: MapPoint) {
  const nearest = nearestPoints(point.latitude, point.longitude, 50);
  const ids = nearest.map((p) => p.id);
  await openCarouselForIds(ids);
}

async function loadMapData() {
  if (!leafletMap) return;

  try {
    const pointsJson = await invoke<string>('get_heatmap_data');
    const allPoints: MapPoint[] = JSON.parse(pointsJson);
    mapPoints.value = filterByDateRange(allPoints);
    buildPointGrid(mapPoints.value);

    if (mapPoints.value.length === 0) {
      loading.value = false;
      return;
    }

    let size = leafletMap.getSize();
    let retries = 0;
    while ((size.x === 0 || size.y === 0) && retries < 15) {
      await new Promise((r) => setTimeout(r, 300));
      leafletMap.invalidateSize();
      size = leafletMap.getSize();
      retries++;
    }

    clusterGroup = L.markerClusterGroup({
      chunkedLoading: true,
      maxClusterRadius: 60,
      spiderfyOnMaxZoom: true,
      showCoverageOnHover: false,
      zoomToBoundsOnClick: true,
      iconCreateFunction: (cluster: L.MarkerCluster) => {
        const count = cluster.getChildCount();
        const size = count < 10 ? 'sm' : count < 100 ? 'md' : 'lg';
        const textColor = themeTextColor();
        return L.divIcon({
          html: `<div class="cluster-icon cluster-${size}" style="color: ${textColor}"><span>${count}</span></div>`,
          className: 'custom-cluster',
          iconSize: L.point(44, 44),
        });
      },
    });

    const bounds: [number, number][] = [];
    for (const p of mapPoints.value) {
      const marker: L.Marker & { _p?: MapPoint; _thumbLoaded?: boolean } = L.marker(
        [p.latitude, p.longitude],
        { icon: makeDotIcon() },
      );
      marker._p = p;
      marker.on('click', (e: L.LeafletMouseEvent) => {
        L.DomEvent.stop(e);
        openCarouselForPoint(p);
      });
      marker.on('mouseover', () => {
        if (!marker.getPopup()) {
          const thumbnailDiv = L.DomUtil.create('div', 'map-thumb-popup');
          thumbnailDiv.innerHTML = '<div class="thumb-loading"></div>';
          marker.bindPopup(thumbnailDiv, {
            closeButton: false,
            offset: L.point(0, -10),
          });
          void mediaSrc(
            { id: p.id, location: p.location, encoded: null } as unknown as MediaItem,
            'thumb',
          ).then((thumb) => {
            if (thumb && thumbnailDiv.isConnected) {
              thumbnailDiv.innerHTML = `<img src="${thumb}" class="thumb-img" decoding="async" loading="lazy">`;
            }
          });
        }
        marker.openPopup();
      });
      marker.on('mouseout', () => {
        marker.closePopup();
      });
      clusterGroup.addLayer(marker);
      bounds.push([p.latitude, p.longitude]);
    }

    leafletMap.addLayer(clusterGroup);

    if (bounds.length > 1) {
      leafletMap.fitBounds(bounds, { padding: [50, 50], maxZoom: 10 });
    } else {
      leafletMap.setView(bounds[0], 4);
    }
  } catch (e) {
    console.error('Failed to load map data', e);
  } finally {
    loading.value = false;
  }
}

function handlePhotoUpdated(updatedPhoto: MediaItem) {
  const idx = viewerPhotos.value.findIndex((p) => p.id === updatedPhoto.id);
  if (idx !== -1) {
    viewerPhotos.value[idx] = updatedPhoto;
  }
}

const hasFilter = computed(() => !!mapFilterStore.dateFrom || !!mapFilterStore.dateTo);
const filterLabel = computed(() => {
  if (!hasFilter.value) return '';
  const fmt = (s: string) => s.slice(0, 10);
  if (mapFilterStore.dateFrom && mapFilterStore.dateTo) {
    return `${fmt(mapFilterStore.dateFrom)} – ${fmt(mapFilterStore.dateTo)}`;
  }
  if (mapFilterStore.dateFrom) return $t('map.filter_from', { date: fmt(mapFilterStore.dateFrom) });
  return $t('map.filter_until', { date: fmt(mapFilterStore.dateTo!) });
});

function filterByDateRange(points: MapPoint[]): MapPoint[] {
  const from = mapFilterStore.dateFrom;
  const to = mapFilterStore.dateTo;
  if (!from && !to) return points;
  return points.filter((p) => {
    if (!p.created) return false;
    const d = p.created.slice(0, 10);
    if (from && d < from.slice(0, 10)) return false;
    if (to && d > to.slice(0, 10)) return false;
    return true;
  });
}

watch(
  () => [mapFilterStore.dateFrom, mapFilterStore.dateTo],
  () => {
    if (leafletMap && !loading.value) {
      loading.value = true;
      if (clusterGroup) {
        leafletMap.removeLayer(clusterGroup);
      }
      void loadMapData();
    }
  },
);

function clearFilter(): void {
  mapFilterStore.clearDateRange();
}

async function onMapReady(map: LeafletMap) {
  leafletMap = map;
  if (!mapClickHandler) {
    mapClickHandler = onMapClick;
    leafletMap.on('click', mapClickHandler);
  }
  leafletMap.on('zoomend moveend', refreshVisibleMarkers);
  await nextTick();
  setTimeout(async () => {
    if (leafletMap) {
      leafletMap.invalidateSize();
      await loadMapData();
      refreshVisibleMarkers();
    }
  }, 100);
}

onMounted(() => {
  void loadTileConfig();
});

onUnmounted(() => {
  if (refreshTimer) {
    clearTimeout(refreshTimer);
    refreshTimer = null;
  }
  if (clusterGroup) {
    leafletMap?.removeLayer(clusterGroup);
    clusterGroup = null;
  }
  if (leafletMap) {
    if (mapClickHandler) {
      leafletMap.off('click', mapClickHandler);
      mapClickHandler = null;
    }
    leafletMap.off('zoomend moveend', refreshVisibleMarkers);
    leafletMap.remove();
    leafletMap = null;
  }
  pointGrid.clear();
  mapPoints.value = [];
  mapFilterStore.clearDateRange();
});

watch(currentPhotoIndex, (idx) => {
  const photo = viewerPhotos.value[idx];
  if (!photo || !leafletMap) return;
  const lat = photo.latitude;
  const lng = photo.longitude;
  if (lat && lng) {
    leafletMap.panTo([lat, lng], { animate: true, duration: 0.5 });
  }
});
</script>

<style scoped>
:deep(.leaflet-container) {
  height: 100%;
  background: rgb(var(--v-theme-surface));
}

.map-empty-state {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: color-mix(in srgb, rgb(var(--v-theme-background)) 70%, transparent);
  backdrop-filter: blur(4px);
  z-index: 1001;
  pointer-events: none;
}

.animate-fade-in {
  animation: fadeIn 0.4s ease-out;
}

@keyframes fadeIn {
  from {
    opacity: 0;
    transform: translateY(10px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.max-w-400 {
  max-width: 400px;
}

:deep(.custom-cluster) {
  background: none !important;
  border: none !important;
}

:deep(.photo-marker) {
  background: none !important;
  border: none !important;
}

:deep(.pm-dot) {
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: rgb(var(--v-theme-info));
  border: 2px solid #fff;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.35);
}

:deep(.pm-loading) {
  width: 40px;
  height: 40px;
  border-radius: 8px;
  background: rgb(var(--v-theme-surface-light));
  border: 2px solid rgba(255, 255, 255, 0.6);
  animation: pulse 1.5s ease-in-out infinite;
}

:deep(.pm-img) {
  width: 44px;
  height: 44px;
  object-fit: cover;
  border-radius: 8px;
  border: 2px solid #fff;
  box-shadow: 0 2px 6px rgba(0, 0, 0, 0.35);
}

:deep(.cluster-icon) {
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 50%;
  font-weight: 600;
  font-size: 13px;
  border: 2px solid rgba(255, 255, 255, 0.6);
  box-shadow: 0 1px 4px rgba(0, 0, 0, 0.15);
}

:deep(.cluster-sm) {
  width: 40px;
  height: 40px;
  background: color-mix(in srgb, rgb(var(--v-theme-info)) 70%, transparent);
}

:deep(.cluster-md) {
  width: 44px;
  height: 44px;
  background: color-mix(in srgb, rgb(var(--v-theme-info)) 80%, transparent);
  font-size: 14px;
}

:deep(.cluster-lg) {
  width: 48px;
  height: 48px;
  background: color-mix(in srgb, rgb(var(--v-theme-info)) 90%, transparent);
  font-size: 15px;
}

:deep(.map-thumb-popup) {
  width: 120px;
  height: 120px;
}

:deep(.thumb-loading) {
  width: 120px;
  height: 120px;
  background: rgb(var(--v-theme-surface-light));
  border-radius: 8px;
  animation: pulse 1.5s ease-in-out infinite;
}

:deep(.thumb-img) {
  width: 120px;
  height: 120px;
  object-fit: cover;
  border-radius: 8px;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.15);
}

@keyframes pulse {
  0%,
  100% {
    opacity: 0.4;
  }
  50% {
    opacity: 0.8;
  }
}

:deep(.leaflet-popup-content-wrapper) {
  padding: 0;
  overflow: hidden;
  border-radius: 8px;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.2);
}

:deep(.leaflet-popup-content) {
  margin: 0;
}

:deep(.leaflet-popup-tip-container) {
  display: none;
}

.filter-chip {
  position: absolute;
  top: 12px;
  left: 50%;
  transform: translateX(-50%);
  z-index: 1000;
  backdrop-filter: blur(8px);
}
</style>
