<script setup lang="ts">
import type { Person } from '@/types/person';
import { getFaceImageSrc } from '@/composables/useMediaUtils';

defineProps<{
  faces: Person[];
}>();

defineEmits<{
  viewCluster: [group: Person];
  promptName: [group: Person];
}>();
</script>

<template>
  <section v-if="faces.length > 0" class="animate-fade-up" :style="{ animationDelay: '0.1s' }">
    <div class="d-flex align-center mb-8 flex-nowrap">
      <h2 class="text-h5 font-weight-black text-zinc-primary pr-6 flex-shrink-0">
        {{ $t('people.new_faces') }}
      </h2>
      <v-divider class="border-subtle border-opacity-100"></v-divider>
    </div>

    <v-row class="ga-y-6">
      <v-col cols="6" sm="4" md="3" lg="2" xl="1" v-for="group in faces" :key="group.id">
        <v-card
          class="unnamed-card-reimagined overflow-hidden border-subtle"
          variant="flat"
          color="white"
          rounded="xl"
          @click="$emit('viewCluster', group)"
        >
          <div class="image-wrapper pos-rel">
            <v-img
              :src="getFaceImageSrc(group.representative_crop, group.encoded)"
              aspect-ratio="1"
              cover
              class="hover-scale transition-slow"
            ></v-img>
            <v-chip
              v-if="group.face_count > 1"
              size="x-small"
              color="black"
              variant="flat"
              class="cluster-badge font-weight-bold"
            >
              {{ group.face_count }}
            </v-chip>
          </div>
          <div class="pa-2 bg-surface">
            <v-btn
              block
              variant="flat"
              color="var(--color-bg-zinc-100)"
              size="small"
              class="text-none font-weight-bold rounded-lg py-4 text-zinc-primary border-subtle"
              @click.stop="$emit('promptName', group)"
            >
              {{ $t('people.name_group') }}
            </v-btn>
          </div>
        </v-card>
      </v-col>
    </v-row>
  </section>
</template>

<style scoped>
.unnamed-card-reimagined {
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  background: var(--color-bg-surface) !important;
}
.unnamed-card-reimagined:hover {
  transform: scale(1.02);
  box-shadow: var(--shadow-lg) !important;
}
.image-wrapper {
  background: var(--color-bg-zinc-100);
  overflow: hidden;
}
.hover-scale:hover {
  transform: scale(1.1);
}
.transition-slow {
  transition: all 0.6s cubic-bezier(0.4, 0, 0.2, 1);
}
.cluster-badge {
  position: absolute;
  top: 12px;
  left: 12px;
  z-index: 2;
  opacity: 0.9;
}
.pos-rel {
  position: relative;
}
</style>
