<script setup lang="ts">
import QrcodeVue from 'qrcode.vue'

defineProps<{
  uuid: string
  passphrase: string[]
  peerJoined: boolean
  isConnected: boolean
}>()
</script>

<template>
  <div class="d-flex justify-center mb-6" v-if="uuid">
    <v-sheet
      class="bg-siegu-white rounded-xl pa-6 shadow-lg border-subtle position-relative overflow-hidden"
    >
      <v-fade-transition hide-on-leave>
        <div
          v-if="uuid && peerJoined && !isConnected"
          class="overlay-connecting d-flex flex-column align-center justify-center"
        >
          <v-progress-circular
            indeterminate
            color="black"
            size="48"
            width="4"
            class="mb-4"
          ></v-progress-circular>
          <div class="text-subtitle-2 font-weight-bold text-zinc-primary animate-pulse">
            {{ $t('connect.device_found') }}
          </div>
          <div class="text-caption text-zinc-secondary">
            {{ $t('connect.establishing_link') }}
          </div>
        </div>
      </v-fade-transition>
      <v-fade-transition hide-on-leave>
        <div
          v-if="isConnected"
          class="overlay-connecting d-flex flex-column align-center justify-center bg-white"
        >
          <div class="siegu-icon-circle-success mx-auto mb-4 scale-up">
            <v-icon color="white">mdi-check-bold</v-icon>
          </div>
          <div class="text-subtitle-2 font-weight-bold text-zinc-primary">
            {{ $t('connect.link_established') }}
          </div>
          <div class="text-caption text-zinc-secondary">
            {{ $t('connect.ready_to_sync') }}
          </div>
        </div>
      </v-fade-transition>
      <qrcode-vue
        :value="uuid"
        :size="200"
        level="H"
        :class="{ 'opacity-20 blur-sm transition-all': uuid && (peerJoined || isConnected) }"
      />
    </v-sheet>
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
.overlay-connecting {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  z-index: 10;
  background: rgba(0, 0, 0, 0.3);
  backdrop-filter: blur(4px);
}

.blur-sm {
  filter: blur(2px);
}

.opacity-20 {
  opacity: 0.2;
}

.transition-all {
  transition: all 0.3s ease;
}

.animate-pulse {
  animation: pulse 1.5s infinite;
}

@keyframes pulse {
  0%,
  100% {
    opacity: 1;
  }
  50% {
    opacity: 0.6;
  }
}

.position-relative {
  position: relative;
}

.overflow-hidden {
  overflow: hidden;
}

.siegu-icon-circle-success {
  width: 48px;
  height: 48px;
  background: #22c55e;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  box-shadow: 0 4px 12px rgba(34, 197, 94, 0.3);
}

.scale-up {
  animation: scaleUp 0.4s cubic-bezier(0.175, 0.885, 0.32, 1.275);
}

@keyframes scaleUp {
  from {
    transform: scale(0.5);
    opacity: 0;
  }
  to {
    transform: scale(1);
    opacity: 1;
  }
}

.uppercase {
  text-transform: uppercase;
}

.tracking-widest {
  letter-spacing: 0.1em;
}
</style>
