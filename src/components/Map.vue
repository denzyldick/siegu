<template>
  <div style="height: 100vh; width: 100%; position: relative;">
    <!-- Empty State Overlay -->
    <div v-if="!loading && mapPoints.length === 0" class="map-empty-state">
      <div class="d-flex flex-column align-center justify-center h-100 px-6 text-center animate-fade-in">
        <v-icon size="48" color="#3f3f46" class="mb-4">mdi-map-marker-off-outline</v-icon>
        <div class="text-h6 text-zinc-secondary font-weight-bold">No location data found</div>
        <p class="text-body-2 text-zinc-muted mt-1 max-w-400">Photos with EXIF GPS coordinates will automatically appear on this map after indexing.</p>
      </div>
    </div>

    <!-- Loading Overlay -->
    <div v-if="loading" class="map-empty-state">
      <div class="d-flex flex-column align-center justify-center h-100">
        <v-progress-circular indeterminate color="white" size="32" width="3"></v-progress-circular>
      </div>
    </div>

    <l-map
      ref="map"
      v-model:zoom="zoom"
      :center="initialCenter"
      @ready="onMapReady"
      :minZoom="2"
      :options="{ zoomControl: false, attributionControl: false, preferCanvas: true }"
      style="height: 100%; width: 100%; background: #e2e2e7;"
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
          crossOrigin: true
        }"
      />
    </l-map>

    <PhotoViewer
      v-model="viewerOpen"
      :photos="viewerPhotos"
      v-model:index="currentPhotoIndex"
      @update:photo="handlePhotoUpdated"
    />
  </div>
</template>

<script>
import "leaflet/dist/leaflet.css";
import { LMap, LTileLayer } from "@vue-leaflet/vue-leaflet";
import L from "leaflet";
if (typeof window !== 'undefined') {
  window.L = L;
}
import "leaflet.markercluster";
import { invoke } from "@tauri-apps/api/core";
import PhotoViewer from "./PhotoViewer.vue";

export default {
  components: {
    LMap,
    LTileLayer,
    PhotoViewer
  },
  data() {
    return {
      zoom: 2,
      initialCenter: [20, 0],
      map: null,
      mapPoints: [],
      clusterGroup: null,
      loading: true,
      viewerOpen: false,
      viewerPhotos: [],
      currentPhotoIndex: 0,
    };
  },
  methods: {
    async onMapReady(map) {
      this.map = map;
      this.$nextTick(async () => {
        setTimeout(async () => {
          if (this.map) {
            this.map.invalidateSize();
            await this.loadMapData();
          }
        }, 100);
      });
    },
    async loadMapData() {
        if (!this.map) return;

        try {
            const pointsJson = await invoke("get_heatmap_data");
            this.mapPoints = JSON.parse(pointsJson);

            if (this.mapPoints.length === 0) {
                this.loading = false;
                return;
            }

            let size = this.map.getSize();
            let retries = 0;
            while ((size.x === 0 || size.y === 0) && retries < 15) {
                await new Promise(r => setTimeout(r, 300));
                this.map.invalidateSize();
                size = this.map.getSize();
                retries++;
            }

            this.clusterGroup = L.markerClusterGroup({
                chunkedLoading: true,
                maxClusterRadius: 60,
                spiderfyOnMaxZoom: true,
                showCoverageOnHover: false,
                zoomToBoundsOnClick: true,
                iconCreateFunction: (cluster) => {
                    const count = cluster.getChildCount();
                    const size = count < 10 ? 'sm' : count < 100 ? 'md' : 'lg';
                    return L.divIcon({
                        html: `<div class="cluster-icon cluster-${size}"><span>${count}</span></div>`,
                        className: 'custom-cluster',
                        iconSize: L.point(44, 44)
                    });
                }
            });

            const bounds = [];
            for (const p of this.mapPoints) {
                const marker = L.circleMarker([p.latitude, p.longitude], {
                    radius: 5,
                    fillColor: '#2563eb',
                    color: '#ffffff',
                    weight: 1.5,
                    opacity: 0.9,
                    fillOpacity: 0.5
                });
                marker._mapPointId = p.id;
                marker.on('click', () => this.openCarouselForPoint(p));
                marker.on('mouseover', (e) => {
                    const thumbnailDiv = L.DomUtil.create('div', 'map-thumb-popup');
                    thumbnailDiv.innerHTML = '<div class="thumb-loading"></div>';
                    marker.bindPopup(thumbnailDiv, {
                        closeButton: false,
                        offset: L.point(0, -10)
                    }).openPopup();
                    invoke("get_photo_encoded_batch", { ids: [p.id] }).then(thumbnails => {
                        const encoded = thumbnails[p.id];
                        if (encoded) {
                            thumbnailDiv.innerHTML = `<img src="${encoded}" class="thumb-img">`;
                            marker.setRadius(7);
                            marker.setStyle({ fillOpacity: 0.8 });
                        }
                    });
                });
                marker.on('mouseout', () => {
                    marker.closePopup();
                });
                this.clusterGroup.addLayer(marker);
                bounds.push([p.latitude, p.longitude]);
            }

            this.map.addLayer(this.clusterGroup);

            if (bounds.length > 1) {
                this.map.fitBounds(bounds, { padding: [50, 50], maxZoom: 10 });
            } else {
                this.map.setView(bounds[0], 4);
            }

            this.map.on('click', (e) => this.handleMapClick(e));
        } catch (e) {
            console.error("Failed to load map data", e);
        } finally {
            this.loading = false;
        }
    },
    async openCarouselForIds(ids) {
        if (ids.length === 0) return;
        try {
            const photosJson = await invoke("get_photos_for_map_click", { ids });
            this.viewerPhotos = JSON.parse(photosJson);
            if (this.viewerPhotos.length > 0) {
                this.currentPhotoIndex = 0;
                this.viewerOpen = true;
            }
        } catch (e) {
            console.error("Failed to load photo details", e);
        }
    },
    async openCarouselForPoint(point) {
        const nearest = this.nearestPoints(point.latitude, point.longitude, 50);
        const ids = nearest.map(p => p.id);
        await this.openCarouselForIds(ids);
    },
    nearestPoints(lat, lng, count) {
        return this.mapPoints
            .map(p => ({
                ...p,
                dist: Math.sqrt(Math.pow(p.latitude - lat, 2) + Math.pow(p.longitude - lng, 2))
            }))
            .sort((a, b) => a.dist - b.dist)
            .slice(0, count);
    },
    async handleMapClick(e) {
        const nearest = this.nearestPoints(e.latlng.lat, e.latlng.lng, 50);
        if (nearest.length === 0) return;
        const ids = nearest.map(p => p.id);
        await this.openCarouselForIds(ids);
    },
    handlePhotoUpdated(updatedPhoto) {
        const idx = this.viewerPhotos.findIndex(p => p.id === updatedPhoto.id);
        if (idx !== -1) {
            this.viewerPhotos[idx] = updatedPhoto;
        }
    },
  },
  watch: {
    currentPhotoIndex(idx) {
        const photo = this.viewerPhotos[idx];
        if (!photo || !this.map) return;
        const lat = photo.latitude;
        const lng = photo.longitude;
        if (lat && lng) {
            this.map.panTo([lat, lng], { animate: true, duration: 0.5 });
        }
    }
  }
};
</script>

<style scoped>
:deep(.leaflet-container) {
    height: 100%;
    background: #f4f4f5;
}

.map-empty-state {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(255, 255, 255, 0.7);
  backdrop-filter: blur(4px);
  z-index: 1001;
  pointer-events: none;
}

.animate-fade-in {
  animation: fadeIn 0.4s ease-out;
}

@keyframes fadeIn {
  from { opacity: 0; transform: translateY(10px); }
  to { opacity: 1; transform: translateY(0); }
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
  border: 2px solid rgba(255,255,255,0.6);
  box-shadow: 0 1px 4px rgba(0,0,0,0.15);
}

:deep(.cluster-sm) {
  width: 40px;
  height: 40px;
  background: rgba(37, 99, 235, 0.7);
}

:deep(.cluster-md) {
  width: 44px;
  height: 44px;
  background: rgba(37, 99, 235, 0.8);
  font-size: 14px;
}

:deep(.cluster-lg) {
  width: 48px;
  height: 48px;
  background: rgba(37, 99, 235, 0.9);
  font-size: 15px;
}

:deep(.map-thumb-popup) {
  width: 120px;
  height: 120px;
}

:deep(.thumb-loading) {
  width: 120px;
  height: 120px;
  background: #e4e4e7;
  border-radius: 6px;
  animation: pulse 1.5s ease-in-out infinite;
}

:deep(.thumb-img) {
  width: 120px;
  height: 120px;
  object-fit: cover;
  border-radius: 6px;
  box-shadow: 0 2px 8px rgba(0,0,0,0.15);
}

@keyframes pulse {
  0%, 100% { opacity: 0.4; }
  50% { opacity: 0.8; }
}

:deep(.leaflet-popup-content-wrapper) {
  padding: 0;
  overflow: hidden;
  border-radius: 8px;
  box-shadow: 0 4px 16px rgba(0,0,0,0.2);
}

:deep(.leaflet-popup-content) {
  margin: 0;
}

:deep(.leaflet-popup-tip-container) {
  display: none;
}
</style>
