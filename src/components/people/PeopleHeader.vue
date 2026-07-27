<script setup lang="ts">
defineProps<{
  indexingCount: number
  namedCount: number
}>()

defineEmits<{
  startIndexing: []
}>()

function formatCount(value: number): string {
  return value.toLocaleString(localStorage.getItem('siegu_language') || 'en')
}
</script>

<template>
  <div class="header-banner px-6 pt-8 pb-10 bg-white border-bottom-subtle w-100">
    <div class="w-100">
      <div class="d-flex align-center justify-space-between flex-wrap ga-4">
        <div>
          <h1 class="text-h3 font-weight-black tracking-tight text-zinc-primary mb-2">
            {{ $t('people.title') }}
          </h1>
          <p class="text-body-1 text-zinc-secondary font-weight-medium">
            {{ $t('people.desc') }}
          </p>
        </div>
        <div class="d-flex align-center ga-3 flex-wrap">
          <v-btn
            v-if="indexingCount === 0"
            variant="flat"
            class="siegu-btn text-none font-weight-bold rounded-lg px-6 h-100 py-3"
            prepend-icon="mdi-face-recognition"
            @click="$emit('startIndexing')"
          >
            {{ $t('people.index_faces') }}
          </v-btn>
          <div
            v-else
            class="d-flex align-center bg-white border-subtle rounded-lg px-4 py-2 shadow-sm animate-pulse"
          >
            <v-progress-circular
              indeterminate
              size="16"
              width="2"
              color="#18181b"
              class="mr-3"
            ></v-progress-circular>
            <div class="text-caption font-weight-black text-zinc-primary">
              {{ $t('people.indexing_remaining', { count: formatCount(indexingCount) }) }}
            </div>
          </div>

          <v-chip
            class="px-4 py-5 font-weight-bold border-subtle"
            variant="flat"
            color="white"
            rounded="lg"
          >
            <v-icon start size="18" color="#18181b">mdi-account-check</v-icon>
            {{ $t('people.named_count', { count: namedCount }) }}
          </v-chip>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.header-banner {
  box-shadow: 0 1px 2px 0 rgba(0, 0, 0, 0.05);
}
.animate-pulse {
  animation: pulse 2s infinite ease-in-out;
}
@keyframes pulse {
  0%, 100% { transform: scale(1); opacity: 0.8; }
  50% { transform: scale(1.1); opacity: 1; }
}
</style>
