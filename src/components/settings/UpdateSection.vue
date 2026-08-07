<template>
  <v-card variant="flat" color="surface" rounded="xl" class="mb-6 overflow-hidden border-subtle">
    <v-card-item class="bg-zinc-100 py-4">
      <template v-slot:prepend>
        <div class="siegu-icon-circle-dark mr-3">
          <v-icon color="var(--color-text-btn)" size="small">mdi-update</v-icon>
        </div>
      </template>
      <v-card-title class="text-h6 text-zinc-primary font-weight-bold">{{
        $t('update_title')
      }}</v-card-title>
    </v-card-item>
    <v-card-text class="pt-2">
      <v-list lines="two" class="bg-transparent">
        <v-list-item class="px-0">
          <template v-slot:title>
            <span class="font-weight-bold text-zinc-primary">{{ $t('update_desc') }}</span>
          </template>
          <template v-slot:subtitle>
            <span class="text-zinc-secondary">{{
              supported ? statusText : $t('update_not_supported')
            }}</span>
          </template>
          <template v-slot:append>
            <v-btn
              size="small"
              variant="flat"
              theme="dark"
              :loading="status === 'checking'"
              :disabled="!supported || status === 'downloading'"
              @click="status === 'available' ? $emit('download-update') : $emit('check-update')"
              class="siegu-btn px-4"
            >
              <div class="d-flex align-center">
                <div class="siegu-icon-circle siegu-icon-circle-md mr-3">
                  <v-icon color="var(--color-text-btn)" size="small">{{ btnIcon }}</v-icon>
                </div>
                <span class="font-weight-bold">{{ btnText }}</span>
              </div>
            </v-btn>
          </template>
        </v-list-item>
      </v-list>
    </v-card-text>
  </v-card>
</template>

<script setup lang="ts">
defineProps<{
  status: string;
  statusText: string;
  btnText: string;
  btnIcon: string;
  supported: boolean;
}>();

defineEmits<{
  'check-update': [];
  'download-update': [];
}>();
</script>
