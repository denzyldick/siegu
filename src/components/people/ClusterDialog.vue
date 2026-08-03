<script setup lang="ts">
import type { Person, UnnamedFace } from '@/types/person'
import { getFaceImageSrc } from '@/composables/useMediaUtils'

const props = defineProps<{
  modelValue: boolean
  cluster: Person | null
  faces: UnnamedFace[]
}>()

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  removeFace: [faceId: number]
  promptName: [group: Person]
}>()
</script>

<template>
  <v-dialog
    :model-value="modelValue"
    max-width="800"
    transition="dialog-bottom-transition"
    scrollable
    @update:model-value="(v: boolean) => emit('update:modelValue', v)"
  >
    <v-card class="rounded-xl border-subtle overflow-hidden" color="surface">
      <v-card-title class="pa-6 bg-zinc-100 border-bottom-subtle d-flex align-center">
        <div>
          <div class="text-h5 font-weight-black text-zinc-primary">
            {{ $t('people.grouped_faces') }}
          </div>
          <div
            class="text-caption text-zinc-secondary font-weight-bold uppercase tracking-widest"
          >
            {{ $t('people.appearances_in_cluster', { count: faces.length }) }}
          </div>
        </div>
        <v-spacer></v-spacer>
        <v-btn
          icon="mdi-close"
          variant="text"
          size="small"
          @click="emit('update:modelValue', false)"
        ></v-btn>
      </v-card-title>

      <v-card-text class="pa-6">
        <v-row class="ga-4">
          <v-col cols="4" sm="3" md="2" v-for="face in faces" :key="face.face_id">
            <v-card
              variant="flat"
              border
              class="border-subtle overflow-hidden rounded-lg pos-rel group-face-card"
            >
              <v-img
                :src="getFaceImageSrc(face.crop_path, face.encoded)"
                aspect-ratio="1"
                cover
              ></v-img>
              <div class="face-remove-btn">
                <v-btn
                  icon="mdi-close"
                  size="x-small"
                  color="error"
                  variant="flat"
                  @click="emit('removeFace', face.face_id)"
                ></v-btn>
              </div>
            </v-card>
          </v-col>
        </v-row>
      </v-card-text>

      <v-card-actions class="pa-6 bg-zinc-50 border-top-subtle">
        <v-btn
          block
          color="primary"
          variant="flat"
          height="56"
          class="rounded-xl font-weight-bold text-none"
          prepend-icon="mdi-pencil"
          @click="cluster && emit('promptName', cluster)"
        >
          {{ $t('people.name_this_group') }}
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<style scoped>
.group-face-card:hover .face-remove-btn {
  opacity: 1;
}
.face-remove-btn {
  position: absolute;
  top: 4px;
  right: 4px;
  opacity: 0;
  transition: opacity 0.2s ease;
  z-index: 3;
}
.pos-rel {
  position: relative;
}
</style>
