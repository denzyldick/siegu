import { ref, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import {
  generatePairingCodes,
  hashPairingCode,
  startWebrtcSession,
  startLanHost,
  stopWebrtcSession,
  requestStartSync,
} from '@/services/tauri'
import type { DiscoveredHost, PeerDevice } from '@/types/sync'

export type ConnectMode = 'host' | 'join'

export interface SyncProgressState {
  status: string
  progress: number
  items_completed: number
  items_total: number
}

export function useConnect() {
  const { t } = useI18n()

  const mode = ref<ConnectMode>('host')
  const uuid = ref('')
  const passphrase = ref<string[]>([])
  const joinPassphrase = ref('')
  const connectionStatus = ref('')
  const isConnected = ref(false)
  const peerJoined = ref(false)
  const loading = ref(false)
  const syncing = ref(false)
  const disconnecting = ref(false)
  const syncProgress = ref<SyncProgressState>({ status: '', progress: 0, items_completed: 0, items_total: 0 })
  const selectedLanHost = ref<DiscoveredHost | null>(null)
  const peerList = ref<PeerDevice[]>([])
  const hostIp = ref('')
  const hostPort = ref(0)

  let unlistenWebRtc: UnlistenFn | null = null
  let unlistenSync: UnlistenFn | null = null
  let unlistenRoomCode: UnlistenFn | null = null
  let unlistenPeerConnected: UnlistenFn | null = null
  let unlistenPeerDisconnected: UnlistenFn | null = null

  function handleWebRtcState(payload: string): void {
    connectionStatus.value = payload
    if (payload === 'Peer Joined') {
      peerJoined.value = true
    }
    if (payload === 'Connected' || payload === 'connected') {
      isConnected.value = true
      peerJoined.value = false
      loading.value = false
    }
    if (
      payload.toLowerCase().includes('error') ||
      payload.toLowerCase().includes('failed') ||
      payload.toLowerCase().includes('disconnected')
    ) {
      isConnected.value = false
      peerJoined.value = false
      loading.value = false
    }
  }

  function handleSyncProgress(payload: {
    status: string
    phase?: 'idle' | 'syncing' | 'completed'
    progress: number
    items_completed: number
    items_total: number
  }): void {
    syncProgress.value = {
      status: payload.status,
      progress: payload.progress,
      items_completed: payload.items_completed ?? 0,
      items_total: payload.items_total ?? 0,
    }
    if (payload.phase === 'syncing') {
      syncing.value = true
    } else if (payload.phase === 'completed') {
      syncing.value = false
    }
  }

  function handleRoomCode(payload: string): void {
    passphrase.value = [payload]
    uuid.value = payload
  }

  function handlePeerConnected(device: PeerDevice): void {
    const existing = peerList.value.findIndex((p) => p.device_id === device.device_id)
    if (existing >= 0) {
      peerList.value[existing] = device
    } else {
      peerList.value.push(device)
    }
    connectionStatus.value = `Peer Connected: ${device.name}`
    isConnected.value = true
  }

  function handlePeerDisconnected(peerId: string): void {
    peerList.value = peerList.value.filter((p) => p.device_id !== peerId)
    if (peerList.value.length === 0) {
      isConnected.value = false
      connectionStatus.value = t('connect.disconnected')
    }
  }

  function getSignalingUrl(): string {
    if (selectedLanHost.value) {
      return `ws://${selectedLanHost.value.ip}:${selectedLanHost.value.port}`
    }
    if (hostIp.value && hostPort.value) {
      return `ws://${hostIp.value}:${hostPort.value}`
    }
    return import.meta.env.VITE_SIGNALING_URL || 'wss://siegu.io/ws'
  }

  async function initialize(): Promise<void> {
    connectionStatus.value = t('connect.generating_key')
    try {
      const codes = await generatePairingCodes()
      uuid.value = codes.uuid
      passphrase.value = codes.passphrase
      const roomId = await hashPairingCode(codes.uuid)
      if (mode.value === 'host') {
        const info = await startLanHost(roomId, false)
        hostIp.value = info.ip
        hostPort.value = info.port
      }
    } catch (error) {
      console.error('Pairing Error:', error)
      connectionStatus.value = t('connect.pairing_error')
    }
  }

  function selectLanHost(host: DiscoveredHost): void {
    selectedLanHost.value = host
  }

  async function joinWebRTC(ip?: string, port?: string): Promise<void> {
    if (!joinPassphrase.value || loading.value) return
    if (ip && port) {
      hostIp.value = ip
      hostPort.value = parseInt(port) || 0
    }
    loading.value = true
    connectionStatus.value = t('connect.joining_room')
    const signalingUrl = getSignalingUrl()
    try {
      const roomId = await hashPairingCode(joinPassphrase.value)
      await startWebrtcSession(roomId, true, signalingUrl)
      connectionStatus.value = t('connect.awaiting_webrtc_receiver')
    } catch (error) {
      loading.value = false
      connectionStatus.value = t('connect.error_joining', { error })
    }
  }

  async function triggerSync(): Promise<void> {
    syncing.value = true
    try {
      await requestStartSync()
    } catch (err) {
      console.error('Failed to start sync:', err)
      syncing.value = false
    }
  }

  async function disconnectSession(): Promise<void> {
    if (disconnecting.value) return
    disconnecting.value = true
    try {
      await stopWebrtcSession()
      connectionStatus.value = t('connect.disconnected')
      uuid.value = ''
      passphrase.value = []
      isConnected.value = false
      peerJoined.value = false
      syncProgress.value = { status: '', progress: 0, items_completed: 0, items_total: 0 }
      selectedLanHost.value = null
      peerList.value = []
    } catch (error) {
      console.error('Disconnect Error:', error)
    } finally {
      disconnecting.value = false
    }
  }

  async function startEventListeners(): Promise<void> {
    unlistenWebRtc = await listen<string>('webrtc-state', (event) => {
      handleWebRtcState(event.payload)
    })
    unlistenSync = await listen<{
      status: string
      progress: number
      items_completed: number
      items_total: number
    }>('sync-progress', (event) => {
      handleSyncProgress(event.payload)
    })
    unlistenRoomCode = await listen<string>('room-code', (event) => {
      handleRoomCode(event.payload)
    })
    unlistenPeerConnected = await listen<PeerDevice>('peer-connected', (event) => {
      handlePeerConnected(event.payload)
    })
    unlistenPeerDisconnected = await listen<string>('peer-disconnected', (event) => {
      handlePeerDisconnected(event.payload)
    })
  }

  function stopEventListeners(): void {
    if (unlistenWebRtc) {
      unlistenWebRtc()
      unlistenWebRtc = null
    }
    if (unlistenSync) {
      unlistenSync()
      unlistenSync = null
    }
    if (unlistenRoomCode) {
      unlistenRoomCode()
      unlistenRoomCode = null
    }
    if (unlistenPeerConnected) {
      unlistenPeerConnected()
      unlistenPeerConnected = null
    }
    if (unlistenPeerDisconnected) {
      unlistenPeerDisconnected()
      unlistenPeerDisconnected = null
    }
  }

  function resetJoinState(): void {
    connectionStatus.value = ''
    peerJoined.value = false
    syncProgress.value = { status: '', progress: 0, items_completed: 0, items_total: 0 }
    selectedLanHost.value = null
    hostIp.value = ''
    hostPort.value = 0
  }

  onUnmounted(() => {
    stopEventListeners()
  })

  return {
    mode,
    uuid,
    passphrase,
    joinPassphrase,
    connectionStatus,
    isConnected,
    peerJoined,
    loading,
    syncing,
    disconnecting,
    syncProgress,
    selectedLanHost,
    peerList,
    hostIp,
    hostPort,
    initialize,
    selectLanHost,
    joinWebRTC,
    triggerSync,
    disconnectSession,
    startEventListeners,
    stopEventListeners,
    resetJoinState,
  }
}
