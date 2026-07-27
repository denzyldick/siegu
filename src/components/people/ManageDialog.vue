<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import type { Person } from '@/types/person'

const props = defineProps<{
  modelValue: boolean
  activePerson: Person | null
  people: Person[]
}>()

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  rename: [id: number, newName: string]
  merge: [fromId: number, toId: number]
}>()

const newName = ref('')
const manageTab = ref('rename')
const mergeTargetId = ref<number | null>(null)

const otherPeople = computed(() => {
  return props.people.filter((p) => p.id !== props.activePerson?.id)
})

watch(() => props.modelValue, (open) => {
  if (open && props.activePerson) {
    newName.value = props.activePerson.name
    manageTab.value = 'rename'
    mergeTargetId.value = null
  }
})

function handleRename(): void {
  if (!props.activePerson || !newName.value) return
  emit('rename', props.activePerson.id, newName.value)
}

function handleMerge(): void {
  if (!props.activePerson || !mergeTargetId.value) return
  emit('merge', props.activePerson.id, mergeTargetId.value)
}
</script>

<template>
  <v-dialog
    :model-value="modelValue"
    max-width="480"
    transition="scale-transition"
    @update:model-value="(v: boolean) => emit('update:modelValue', v)"
  >
    <v-card class="rounded-xl pa-2 elevation-24 overflow-hidden border-subtle" color="surface">
      <div class="pa-6">
        <div class="d-flex align-center justify-space-between mb-6">
          <h3 class="text-h5 font-weight-black text-zinc-primary">
            {{ $t('people.profile_actions') }}
          </h3>
          <v-btn
            icon="mdi-close"
            variant="text"
            size="small"
            @click="emit('update:modelValue', false)"
          ></v-btn>
        </div>

        <v-tabs
          v-model="manageTab"
          bg-color="#f4f4f5"
          color="#18181b"
          grow
          mandatory
          class="rounded-xl mb-8 p-1 border-subtle"
        >
          <v-tab value="rename" class="rounded-lg text-none font-weight-bold">{{
            $t('people.rename')
          }}</v-tab>
          <v-tab value="merge" class="rounded-lg text-none font-weight-bold">{{
            $t('people.merge')
          }}</v-tab>
        </v-tabs>

        <v-window v-model="manageTab" class="py-2">
          <v-window-item value="rename">
            <label class="text-caption font-weight-bold text-zinc-muted mb-2 d-block px-1">{{
              $t('people.new_name_for', { name: activePerson?.name })
            }}</label>
            <v-text-field
              v-model="newName"
              variant="outlined"
              density="comfortable"
              class="name-field-modern mb-8"
              hide-details
              @keyup.enter="handleRename"
            ></v-text-field>

            <v-btn
              block
              size="x-large"
              variant="flat"
              class="siegu-btn rounded-xl text-none font-weight-bold py-7 shadow-lg"
              @click="handleRename"
            >
              {{ $t('people.update_name') }}
            </v-btn>
          </v-window-item>

          <v-window-item value="merge">
            <div
              class="bg-amber-50 rounded-xl pa-4 mb-8 d-flex align-start ga-3 border-amber-subtle"
            >
              <v-icon color="#b45309" size="20" class="mt-1">mdi-alert-circle-outline</v-icon>
              <div class="text-body-2 text-amber-darken-4 font-weight-medium">
                <span>{{ $t('people.merge_desc', { name: activePerson?.name }) }}</span>
              </div>
            </div>

            <v-select
              v-model="mergeTargetId"
              :items="otherPeople"
              item-title="name"
              item-value="id"
              :label="$t('people.target_profile')"
              variant="outlined"
              density="comfortable"
              class="name-field-modern mb-8"
              hide-details
            ></v-select>

            <v-btn
              block
              size="x-large"
              variant="flat"
              class="siegu-btn rounded-xl text-none font-weight-bold py-7 shadow-sm"
              :disabled="!mergeTargetId"
              @click="handleMerge"
            >
              {{ $t('people.confirm_merge') }}
            </v-btn>
          </v-window-item>
        </v-window>
      </div>
    </v-card>
  </v-dialog>
</template>

<style scoped>
.name-field-modern :deep(.v-field) {
  border-radius: 12px !important;
  background: white !important;
}
.name-field-modern :deep(.v-field__outline) {
  --v-field-border-opacity: 0.15 !important;
}
.bg-amber-50 {
  background-color: #fffbeb !important;
}
.border-amber-subtle {
  border: 1px solid #fde68a !important;
}
</style>
