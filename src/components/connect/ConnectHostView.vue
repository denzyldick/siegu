<template>
  <div class="d-flex justify-center ga-5 mb-4">
    <v-icon size="28" color="#a1a1aa">mdi-microsoft-windows</v-icon>
    <v-icon size="28" color="#a1a1aa">mdi-apple</v-icon>
    <v-icon size="28" color="#a1a1aa">mdi-linux</v-icon>
    <v-icon size="28" color="#a1a1aa">mdi-android</v-icon>
    <v-icon size="28" color="#a1a1aa">mdi-apple-ios</v-icon>
  </div>

  <div class="text-caption text-zinc-muted text-center mb-4 px-2" style="max-width: 320px">
    {{ $t('connect.host_instructions') }}
  </div>

  <div class="d-flex justify-center flex-nowrap gap-2 mb-3 overflow-x-auto" v-if="passphrase.length > 0">
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

  <v-btn
    v-if="!copied"
    variant="outlined"
    color="black"
    size="small"
    class="text-none text-caption"
    prepend-icon="mdi-content-copy"
    @click="copyPassphrase"
  >
    {{ $t('connect.copy_phrase') }}
  </v-btn>
  <v-btn
    v-else
    variant="outlined"
    color="green"
    size="small"
    class="text-none text-caption"
    prepend-icon="mdi-check"
  >
    {{ $t('connect.copied') }}
  </v-btn>
</template>

<script setup lang="ts">
import { ref } from 'vue'

const props = defineProps<{
  passphrase: string[]
}>()

const copied = ref(false)

function copyPassphrase() {
  const text = props.passphrase.join(' ')
  navigator.clipboard.writeText(text)
  copied.value = true
  setTimeout(() => { copied.value = false }, 2000)
}
</script>