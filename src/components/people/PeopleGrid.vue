<script setup lang="ts">
import type { Person } from '@/types/person'

defineProps<{
  people: Person[]
}>()

defineEmits<{
  viewPerson: [person: Person]
  openManage: [person: Person]
}>()
</script>

<template>
  <section v-if="people.length > 0" class="mb-12 animate-fade-up">
    <div class="d-flex align-center mb-8 flex-nowrap">
      <h2 class="text-h5 font-weight-black text-zinc-primary pr-6 flex-shrink-0">
        {{ $t('people.your_people') }}
      </h2>
      <v-divider class="border-subtle border-opacity-100"></v-divider>
    </div>

    <v-row class="ga-y-6">
      <v-col cols="6" sm="4" md="3" lg="2" xl="1" v-for="person in people" :key="person.id">
        <v-card
          class="person-card-reimagined overflow-hidden border-subtle"
          variant="flat"
          color="white"
          rounded="xl"
          @click="$emit('viewPerson', person)"
        >
          <div class="image-wrapper pos-rel">
            <v-img
              :src="getFaceImageSrc(person.representative_crop, person.encoded)"
              aspect-ratio="1"
              cover
              class="hover-scale transition-slow"
            ></v-img>
            <v-chip
              size="x-small"
              color="black"
              variant="flat"
              class="cluster-badge font-weight-bold"
            >
              {{ person.face_count }}
            </v-chip>
          </div>

          <div class="pa-3 bg-white text-center">
            <h3 class="text-subtitle-2 font-weight-bold text-zinc-primary text-truncate">
              {{ person.name }}
            </h3>
          </div>

          <div class="card-action-overlay">
            <v-btn
              icon="mdi-pencil"
              size="x-small"
              color="white"
              variant="flat"
              class="shadow-sm border-subtle"
              @click.stop="$emit('openManage', person)"
            ></v-btn>
          </div>
        </v-card>
      </v-col>
    </v-row>
  </section>
</template>

<script lang="ts">
import { getFaceImageSrc } from '@/composables/useMediaUtils'
export default { name: 'PeopleGrid' }
</script>

<style scoped>
.person-card-reimagined {
  cursor: pointer;
  background: white;
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  position: relative;
}
.person-card-reimagined:hover {
  transform: translateY(-4px);
  box-shadow: 0 12px 20px -5px rgba(0, 0, 0, 0.1) !important;
  border-color: rgba(0, 0, 0, 0.2) !important;
}
.card-action-overlay {
  position: absolute;
  top: 12px;
  right: 12px;
  opacity: 0;
  transform: scale(0.8);
  transition: all 0.2s ease;
}
.person-card-reimagined:hover .card-action-overlay {
  opacity: 1;
  transform: scale(1);
}
.image-wrapper {
  background: #f1f5f9;
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
