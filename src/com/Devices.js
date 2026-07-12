import { invoke } from '@tauri-apps/api/core';
import { appDataDir, dataDir } from '@tauri-apps/api/path';

class Devices {
  /**
   * List all the devices.
   */
  list() {
    let devices = invoke('list_devices').then(function (resp) {
      console.log(resp);
    });

    return devices;
  }
}
