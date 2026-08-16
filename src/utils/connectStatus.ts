export type ConnectionStatusKey =
  | 'connect.selected_device'
  | 'connect.status_connecting_signaling'
  | 'connect.status_waiting_peer'
  | 'connect.status_secure_ready'
  | 'connect.status_peer_joined'
  | 'connect.status_peer_connected'
  | 'connect.status_peer_disconnected'
  | 'connect.status_room_closed'
  | 'connect.status_sync_started'
  | 'connect.status_connected'
  | 'connect.status_connecting_webrtc'
  | 'connect.status_connection_failed'
  | 'connect.status_awaiting_connection'
  | 'connect.disconnected';

const STATUS_TO_KEY: Record<string, ConnectionStatusKey> = {
  'Connecting to signaling...': 'connect.status_connecting_signaling',
  'Connected to signaling. Waiting for peer...': 'connect.status_waiting_peer',
  'Secure Data Channel Ready': 'connect.status_secure_ready',
  'Peer Joined': 'connect.status_peer_joined',
  'Peer Connected': 'connect.status_peer_connected',
  'Room joined. Waiting for peer...': 'connect.status_waiting_peer',
  'Waiting for peer to join...': 'connect.status_waiting_peer',
  'Waiting for peer...': 'connect.status_waiting_peer',
  'Peer disconnected': 'connect.status_peer_disconnected',
  'Peer Disconnected': 'connect.status_peer_disconnected',
  'Room closed': 'connect.status_room_closed',
  'Sync started': 'connect.status_sync_started',
  Connected: 'connect.status_connected',
  'Connecting WebRTC...': 'connect.status_connecting_webrtc',
  'Connection Failed': 'connect.status_connection_failed',
  'Awaiting connection...': 'connect.status_awaiting_connection',
  Disconnected: 'connect.disconnected',
};

export function connectionStatusKey(status: string): ConnectionStatusKey | null {
  return STATUS_TO_KEY[status] ?? null;
}
