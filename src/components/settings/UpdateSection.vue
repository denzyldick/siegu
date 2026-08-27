<template>
  <v-card variant="flat" color="surface" rounded="xl" class="mb-6 overflow-hidden border">
    <v-card-item class="py-4">
      <template v-slot:prepend>
        <v-avatar color="surface" size="32" class="mr-3">
          <v-icon color="on-surface" size="small">mdi-update</v-icon>
        </v-avatar>
      </template>
      <v-card-title class="text-h6 text-high-emphasis font-weight-bold">{{
        $t('update_title')
      }}</v-card-title>
    </v-card-item>
    <v-card-text class="pt-2">
      <v-list lines="two" class="bg-transparent">
        <v-list-item class="px-0">
          <template v-slot:title>
            <span class="font-weight-bold text-high-emphasis">{{ $t('update_desc') }}</span>
          </template>
          <template v-slot:subtitle>
            <span class="text-medium-emphasis">{{
              supported ? statusText : $t('update_not_supported')
            }}</span>
          </template>
          <template v-slot:append>
            <v-btn
              size="small"
              variant="flat"
              color="primary"
              :loading="status === 'checking'"
              :disabled="!supported || status === 'downloading'"
              @click="status === 'available' ? $emit('download-update') : $emit('check-update')"
              class="px-4"
            >
              <div class="d-flex align-center">
                <v-avatar color="rgba(255,255,255,0.2)" size="32" class="mr-3">
                  <v-icon color="surface" size="small">{{ btnIcon }}</v-icon>
                </v-avatar>
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
