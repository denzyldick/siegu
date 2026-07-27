<script setup lang="ts">
</script>

<template>
  <div
    class="d-flex flex-column align-center justify-center py-16 animate-fade-in"
  >
    <div class="empty-icon-pulse mb-8">
      <v-icon size="80" color="#d4d4d8">mdi-face-recognition</v-icon>
    </div>
    <h3 class="text-h5 font-weight-bold text-zinc-primary mb-2">{{ $t('people.no_faces') }}</h3>
    <p class="text-zinc-secondary text-center max-w-400">
      {{ $t('people.no_faces_desc') }}
    </p>
  </div>
</template>

<style scoped>
.max-w-400 {
  max-width: 400px !important;
}
.empty-icon-pulse {
  animation: pulse 2s infinite ease-in-out;
}
@keyframes pulse {
  0%, 100% { transform: scale(1); opacity: 0.8; }
  50% { transform: scale(1.1); opacity: 1; }
}
</style>
