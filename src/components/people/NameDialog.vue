<script setup lang="ts">
import { ref, watch } from 'vue';
import type { Person } from '@/types/person';
import { getFaceImageSrc } from '@/composables/useMediaUtils';

const props = defineProps<{
  modelValue: boolean;
  activeFace: Person | null;
  people: Person[];
}>();

const emit = defineEmits<{
  'update:modelValue': [value: boolean];
  save: [faceId: number, name: string];
}>();

const newName = ref('');

watch(
  () => props.modelValue,
  (open) => {
    if (open) newName.value = '';
  },
);

function handleSave(): void {
  if (!newName.value || !props.activeFace) return;
  if (props.activeFace.representative_face_id === null) return;
  emit('save', props.activeFace.representative_face_id, newName.value);
}
</script>

<template>
  <v-dialog
    :model-value="modelValue"
    max-width="440"
    transition="dialog-bottom-transition"
    @update:model-value="(v: boolean) => emit('update:modelValue', v)"
  >
    <v-card class="rounded-xl pa-2 elevation-24 border" color="surface">
      <div class="pa-6">
        <div class="d-flex align-center justify-space-between mb-8">
          <h3 class="text-h5 font-weight-black text-high-emphasis">
            {{ $t('people.who_is_this') }}
          </h3>
          <v-btn
            icon="mdi-close"
            variant="text"
            size="small"
            @click="emit('update:modelValue', false)"
          ></v-btn>
        </div>

        <div class="text-body-2 text-disabled mb-4 text-center">
          {{ $t('people.name_dialog_hint') }}
        </div>

        <div class="d-flex justify-center mb-8">
          <v-avatar size="160" color="surface-light" class="border shadow-xl elevation-2">
            <v-img
              v-if="activeFace"
              :src="getFaceImageSrc(activeFace.representative_crop, activeFace.encoded)"
              cover
            ></v-img>
          </v-avatar>
        </div>

        <v-combobox
          v-model="newName"
          :items="people"
          item-title="name"
          item-value="name"
          :return-object="false"
          :placeholder="$t('people.name_placeholder')"
          variant="outlined"
          density="comfortable"
          class="name-field-modern mb-6"
          persistent-placeholder
          autofocus
          hide-details
          @keyup.enter="handleSave"
        >
          <template v-slot:item="{ props: itemProps, item }">
            <v-list-item v-bind="itemProps" class="py-2">
              <template v-slot:prepend>
                <v-avatar size="32" class="mr-2 border">
                  <v-img :src="getFaceImageSrc(item.representative_crop, item.encoded)"></v-img>
                </v-avatar>
              </template>
            </v-list-item>
          </template>
        </v-combobox>

        <v-btn
          block
          size="x-large"
          variant="flat"
          class="rounded-xl text-none font-weight-bold shadow-lg py-7"
          color="primary"
          :disabled="!newName"
          @click="handleSave"
        >
          {{ $t('people.confirm_identity') }}
        </v-btn>
      </div>
    </v-card>
  </v-dialog>
</template>

<style scoped>
.name-field-modern :deep(.v-field) {
  border-radius: var(--radius-md) !important;
  background: rgb(var(--v-theme-surface-light)) !important;
}
.name-field-modern :deep(.v-field__outline) {
  --v-field-border-opacity: 0.15 !important;
}
</style>
