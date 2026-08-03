<template>
  <div class="d-flex justify-center ga-5 mb-4">
    <v-icon size="28" color="#a1a1aa">mdi-microsoft-windows</v-icon>
    <v-icon size="28" color="#a1a1aa">mdi-apple</v-icon>
    <v-icon size="28" color="#a1a1aa">mdi-linux</v-icon>
    <v-icon size="28" color="#a1a1aa">mdi-android</v-icon>
    <v-icon size="28" color="#a1a1aa">mdi-apple-ios</v-icon>
  </div>

  <template v-if="isConnected">
    <div class="d-flex flex-column align-center py-2 ga-2" style="width: 100%">
      <div class="d-flex align-center ga-2">
        <v-icon color="success" size="18">mdi-check-circle</v-icon>
        <span class="text-body-2 font-weight-bold text-success">{{ $t('connect.device_linked') }}</span>
      </div>
      <div v-if="peerName" class="text-caption text-zinc-secondary">
        {{ $t('connect.connected_to') }} <strong>{{ peerName }}</strong>
      </div>
      <div v-if="syncing" class="w-100">
        <div v-if="itemsTotal > 0" class="d-flex align-center ga-2 w-100">
          <v-progress-linear
            :model-value="(itemsCompleted / itemsTotal) * 100"
            color="success"
            height="6"
            rounded
            class="flex-grow-1"
          />
          <span class="text-caption text-success font-weight-bold">
            {{ itemsCompleted }}/{{ itemsTotal }}
          </span>
        </div>
        <v-progress-linear
          v-else
          indeterminate
          color="success"
          height="4"
          class="w-100"
        />
        <span class="text-caption text-zinc-muted">{{ $t('connect.syncing') }}</span>
      </div>
    </div>
  </template>

  <template v-else>
    <div class="text-caption text-zinc-muted text-center mb-4 px-2" style="max-width: 320px">
      {{ $t('connect.host_instructions') }}
    </div>

    <div class="d-flex justify-center flex-wrap gap-2 mb-3" v-if="passphrase.length > 0">
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
      color="primary"
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
</template>

<script setup lang="ts">
import { ref } from 'vue'

const props = withDefaults(defineProps<{
  passphrase: string[]
  isConnected?: boolean
  syncing?: boolean
  itemsCompleted?: number
  itemsTotal?: number
  peerName?: string
}>(), {
  isConnected: false,
  syncing: false,
  itemsCompleted: 0,
  itemsTotal: 0,
  peerName: '',
})

const copied = ref(false)

function copyPassphrase() {
  const text = props.passphrase.join(' ')
  navigator.clipboard.writeText(text)
  copied.value = true
  setTimeout(() => { copied.value = false }, 2000)
}
</script>