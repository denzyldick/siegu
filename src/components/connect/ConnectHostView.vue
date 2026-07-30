<script setup lang="ts">
const props = defineProps<{
  passphrase: string[]
}>()
</script>

<template>
  <div class="text-caption text-zinc-muted text-center mb-4 px-2" style="max-width: 320px">
    {{ $t('connect.host_instructions') }}
  </div>

  <div class="text-caption text-zinc-muted mb-2 uppercase tracking-widest">
    {{ $t('connect.manual_phrase') }}
  </div>
  <div class="d-flex justify-center flex-wrap gap-2 mb-6" v-if="passphrase.length > 0">
    <v-chip
      v-for="(word, index) in passphrase"
      :key="index"
      color="#f4f4f5"
      variant="flat"
      class="font-weight-medium mx-1 text-zinc-primary border-subtle"
      size="small"
    >
      {{ word }}
    </v-chip>
  </div>
</template>

<style scoped>
.uppercase {
  text-transform: uppercase;
}

.tracking-widest {
  letter-spacing: 0.1em;
}
</style>
