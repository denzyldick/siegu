<script setup lang="ts">
import { ref } from 'vue';
import logo from '../assets/logo.png';

const emit = defineEmits<{
  'setup-local': [];
  'setup-sync': [];
}>();

const logoUrl = ref(logo);
</script>

<template>
  <v-container class="fill-height pb-16 bg-siegu-white" fluid>
    <v-row justify="center" align="center">
      <v-col cols="12" sm="10" md="8" lg="6" class="text-center">
        <div class="onboarding-card animate-fade-in">
          <v-avatar size="120" color="transparent" class="mb-8 elevation-0">
            <v-img :src="logoUrl"></v-img>
          </v-avatar>

          <h1 class="text-h2 font-weight-black mb-4 tracking-tighter text-zinc-primary">Siegu</h1>
          <p class="text-subtitle-1 text-zinc-secondary mb-10 max-w-500 mx-auto">
            {{ $t('onboarding.greet_desc') }}
          </p>

          <div class="d-flex flex-column ga-4">
            <v-card variant="flat" class="siegu-btn py-4 px-6" @click="emit('setup-local')">
              <div class="d-flex align-center">
                <div class="siegu-icon-circle mr-4">
                  <v-icon>mdi-folder-plus</v-icon>
                </div>
                <div class="text-left">
                  <div class="font-weight-bold">{{ $t('onboarding.setup_local') }}</div>
                  <div class="text-caption text-zinc-muted">
                    {{ $t('onboarding.setup_local_desc') }}
                  </div>
                </div>
              </div>
            </v-card>

            <v-card variant="flat" class="siegu-btn py-4 px-6" @click="emit('setup-sync')">
              <div class="d-flex align-center">
                <div class="siegu-icon-circle mr-4">
                  <v-icon>mdi-sync</v-icon>
                </div>
                <div class="text-left">
                  <div class="font-weight-bold">{{ $t('onboarding.join_library') }}</div>
                  <div class="text-caption text-zinc-muted">
                    {{ $t('onboarding.join_library_desc') }}
                  </div>
                </div>
              </div>
            </v-card>
          </div>

          <div class="text-caption text-zinc-muted px-10 mt-10">
            {{ $t('onboarding.ai_disclaimer') }}
          </div>
        </div>
      </v-col>
    </v-row>
  </v-container>
</template>

<style scoped>
.onboarding-card {
  padding: 40px 20px;
}

.animate-fade-in {
  animation: fadeIn 0.8s ease-out;
}

@keyframes fadeIn {
  from {
    opacity: 0;
    transform: translateY(20px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}
</style>
