<script setup lang="ts">
import { ref, computed } from 'vue'
import { BrowserQRCodeReader } from '@zxing/library'

const props = defineProps<{
  modelValue: string
  loading: boolean
  isConnected: boolean
  showSyncButton: boolean
  syncing: boolean
}>()

const emit = defineEmits<{
  'update:modelValue': [value: string]
  join: []
  sync: []
}>()

const isScanning = ref(false)
const scannerVideo = ref<HTMLVideoElement | null>(null)
const codeReader = new BrowserQRCodeReader()

const localValue = computed({
  get: () => props.modelValue,
  set: (val: string) => emit('update:modelValue', val),
})

async function startScanner(): Promise<void> {
  isScanning.value = true
  try {
    const videoElement = scannerVideo.value
    if (!videoElement) {
      isScanning.value = false
      return
    }
    await codeReader.decodeFromVideoDevice(null, videoElement, (result) => {
      if (result) {
        emit('update:modelValue', result.getText())
        stopScanner()
        emit('join')
      }
    })
  } catch {
    isScanning.value = false
  }
}

function stopScanner(): void {
  codeReader.reset()
  isScanning.value = false
}
</script>

<template>
  <div class="d-flex justify-center mb-6 flex-column ga-4">
    <v-card
      variant="flat"
      class="bg-zinc-50 border-subtle pa-6 rounded-xl mb-2 text-center w-100"
    >
      <div v-if="!isScanning">
        <div class="siegu-icon-circle mx-auto mb-4">
          <v-icon color="white">mdi-qrcode-scan</v-icon>
        </div>
        <div class="text-h6 font-weight-bold text-zinc-primary mb-2">
          {{ $t('connect.scan_qr') }}
        </div>
        <p class="text-caption text-zinc-secondary mb-6">{{ $t('connect.scan_qr_desc') }}</p>
        <v-btn color="black" block height="56" class="siegu-btn" @click="startScanner">
          {{ $t('connect.open_camera') }}
        </v-btn>
      </div>

      <div v-else class="position-relative">
        <video
          ref="scannerVideo"
          style="
            width: 100%;
            border-radius: 12px;
            background: black;
            max-height: 250px;
            object-fit: cover;
          "
        ></video>
        <v-btn
          icon
          size="small"
          color="white"
          class="position-absolute"
          style="position: absolute; top: 8px; right: 8px; z-index: 20"
          @click="stopScanner"
        >
          <v-icon>mdi-close</v-icon>
        </v-btn>
      </div>
    </v-card>

    <div class="text-caption text-zinc-muted text-center uppercase tracking-widest">
      {{ $t('connect.or_enter_manually') }}
    </div>

    <v-text-field
      v-model="localValue"
      :placeholder="$t('connect.phrase_placeholder')"
      variant="solo-filled"
      density="comfortable"
      hide-details
      flat
      rounded="lg"
      class="text-center siegu-field"
      @keyup.enter="emit('join')"
      :disabled="loading || isConnected"
    ></v-text-field>

    <v-btn
      v-if="!showSyncButton"
      variant="flat"
      @click="emit('join')"
      class="siegu-btn py-6"
      block
      :loading="loading"
      :disabled="!modelValue || isConnected"
    >
      <div class="d-flex align-center">
        <div class="siegu-icon-circle mr-3">
          <v-icon size="14">mdi-link-variant</v-icon>
        </div>
        <span>{{ $t('connect.link_device_button') }}</span>
      </div>
    </v-btn>

    <div
      v-else-if="syncing"
      class="d-flex align-center justify-center py-4"
    >
      <v-progress-linear
        indeterminate
        color="success"
        height="4"
        class="w-100"
      />
      <span class="text-caption text-success ml-3">{{ $t('connect.syncing') }}</span>
    </div>

    <div
      v-else-if="isConnected"
      class="d-flex align-center justify-center py-4"
    >
      <v-icon size="16" color="success" class="mr-2">mdi-check-circle</v-icon>
      <span class="text-caption text-success">{{ $t('connect.sync_complete') }}</span>
    </div>
  </div>
</template>

<style scoped>
.siegu-icon-circle {
  width: 28px;
  height: 28px;
  background: rgba(255, 255, 255, 0.2);
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
}

.uppercase {
  text-transform: uppercase;
}

.tracking-widest {
  letter-spacing: 0.1em;
}
</style>
