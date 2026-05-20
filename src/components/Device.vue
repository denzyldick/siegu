<template>
  <v-card flat max-width="344">
    <qrcode-svg v-if="value !== null" :value />
    <v-card-title>{{ deviceName }}</v-card-title>
    <v-card-subtitle>{{ $t('devices.this_device') }}</v-card-subtitle>
    <v-card-actions>
      <v-btn variant="flat" class="siegu-btn px-4">{{ $t('devices.sync_now') }}</v-btn>
      <v-spacer></v-spacer>

      <v-btn
        class="siegu-btn"
        :icon="show ? 'mdi-chevron-up' : 'mdi-chevron-down'"
        @click="show = !show"
      ></v-btn>
    </v-card-actions>

    <v-expand-transition>
      <div v-show="show">
        <v-divider></v-divider>

        <v-card-text>{{ $t('devices.more_info') }}</v-card-text>
      </div>
    </v-expand-transition>
  </v-card>
</template>
<script>
import QrcodeVue, { QrcodeCanvas, QrcodeSvg } from "qrcode.vue";
export default {
  components: {
    QrcodeVue,
    QrcodeCanvas,
    QrcodeSvg,
  },
  data: () => ({
    show: false,
    value: null,
    deviceName: '',
  }),
  async created() {
    let port = "9489";
    let ip = "192.168.68.115";

    let path = "/new-device";
    let message = "http://" + ip + ":" + port + path;
    this.value = message;
    try {
      const os = await (await import('@tauri-apps/api/core')).invoke("get_os");
      this.deviceName = os.charAt(0).toUpperCase() + os.slice(1);
    } catch (e) {
      this.deviceName = navigator.platform || 'Unknown';
    }
  },
};
</script>
