<script setup lang="ts">
defineProps<{
  loading: boolean
}>()

const emit = defineEmits<{
  click: []
}>()
</script>

<template>
  <div class="d-flex justify-center mb-6 flex-column ga-4">
    <v-btn
      variant="flat"
      color="success"
      class="siegu-btn py-6"
      block
      :loading="loading"
      @click="emit('click')"
    >
      <div class="d-flex align-center">
        <div class="siegu-icon-circle mr-3">
          <v-icon size="14">mdi-sync</v-icon>
        </div>
        <span>{{ $t('connect.start_syncing') }}</span>
      </div>
    </v-btn>
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
</style>
