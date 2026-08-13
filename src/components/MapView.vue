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

    <l-map
      v-model:zoom="zoom"
      :center="initialCenter"
      @ready="onMapReady"
      :minZoom="2"
      :options="{ zoomControl: false, attributionControl: false, preferCanvas: true }"
      style="height: 100%; width: 100%; background: rgb(var(--v-theme-surface-light))"
      class="light-map"
    >
      <l-tile-layer
        url="https://{s}.basemaps.cartocdn.com/light_all/{z}/{x}/{y}{r}.png"
        layer-type="base"
        name="CartoDB Basemap"
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
import { ref, nextTick, watch, onUnmounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { Map as LeafletMap } from 'leaflet';
import MediaViewer from './MediaViewer.vue';
import { useMediaUrl } from '@/composables/useMediaUrl';
import type { MediaItem } from '@/types/media';

interface MapPoint {
  id: number;
  latitude: number;
  longitude: number;
  location: string;
}

const zoom = ref(2);
const initialCenter: [number, number] = [20, 0];
const mapPoints = ref<MapPoint[]>([]);
const loading = ref(true);
const viewerOpen = ref(false);
const viewerPhotos = ref<MediaItem[]>([]);
const currentPhotoIndex = ref(0);

const { thumbUrl } = useMediaUrl();

let leafletMap: LeafletMap | null = null;
let clusterGroup: L.MarkerClusterGroup | null = null;

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

async function openCarouselForPoint(point: MapPoint) {
  const nearest = nearestPoints(point.latitude, point.longitude, 50);
  const ids = nearest.map((p) => p.id);
  await openCarouselForIds(ids);
}

async function loadMapData() {
  if (!leafletMap) return;

  try {
    const pointsJson = await invoke<string>('get_heatmap_data');
    mapPoints.value = JSON.parse(pointsJson);
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
        return L.divIcon({
          html: `<div class="cluster-icon cluster-${size}"><span>${count}</span></div>`,
          className: 'custom-cluster',
          iconSize: L.point(44, 44),
        });
      },
    });

    const bounds: [number, number][] = [];
    for (const p of mapPoints.value) {
      const marker = L.circleMarker([p.latitude, p.longitude], {
        radius: 5,
        fillColor: 'rgb(var(--v-theme-info))',
        color: '#ffffff',
        weight: 1.5,
        opacity: 0.9,
        fillOpacity: 0.5,
      });
      marker.on('click', () => openCarouselForPoint(p));
      marker.on('mouseover', () => {
        const thumbnailDiv = L.DomUtil.create('div', 'map-thumb-popup');
        const thumb = thumbUrl(p.location);
        thumbnailDiv.innerHTML = thumb
          ? `<img src="${thumb}" class="thumb-img" decoding="async" loading="lazy">`
          : '<div class="thumb-loading"></div>';
        marker
          .bindPopup(thumbnailDiv, {
            closeButton: false,
            offset: L.point(0, -10),
          })
          .openPopup();
        if (thumb) {
          marker.setRadius(7);
          marker.setStyle({ fillOpacity: 0.8 });
        }
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

    leafletMap.on('click', (e: L.LeafletMouseEvent) => {
      const nearest = nearestPoints(e.latlng.lat, e.latlng.lng, 50);
      if (nearest.length === 0) return;
      const ids = nearest.map((p) => p.id);
      openCarouselForIds(ids);
    });
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

async function onMapReady(map: LeafletMap) {
  leafletMap = map;
  await nextTick();
  setTimeout(async () => {
    if (leafletMap) {
      leafletMap.invalidateSize();
      await loadMapData();
    }
  }, 100);
}

onUnmounted(() => {
  if (clusterGroup) {
    leafletMap?.removeLayer(clusterGroup);
    clusterGroup = null;
  }
  if (leafletMap) {
    leafletMap.off('click');
    leafletMap.remove();
    leafletMap = null;
  }
  pointGrid.clear();
  mapPoints.value = [];
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
  background: rgb(var(--v-theme-surface-light));
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

:deep(.cluster-icon) {
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 50%;
  font-weight: 600;
  font-size: 13px;
  color: #fff;
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
  border-radius: var(--radius-sm);
  animation: pulse 1.5s ease-in-out infinite;
}

:deep(.thumb-img) {
  width: 120px;
  height: 120px;
  object-fit: cover;
  border-radius: var(--radius-sm);
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
  border-radius: var(--radius-sm);
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.2);
}

:deep(.leaflet-popup-content) {
  margin: 0;
}

:deep(.leaflet-popup-tip-container) {
  display: none;
}
</style>
