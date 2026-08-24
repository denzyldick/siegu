pub mod config;
pub mod database;
pub mod error;
pub mod event_bus;
pub mod face_detector;
pub mod geocode;
pub mod lan_server;
pub mod library;
pub mod mdns;
pub mod mesh;
pub mod mesh_transport;
#[cfg(feature = "ml")]
pub mod ml_engine;
#[cfg(feature = "ml")]
pub mod ml_worker;
pub mod model_manager;
pub mod rpc;
pub mod scanner;
pub mod server;
pub mod shutdown;
pub mod signal;
pub mod signalling;
pub mod sync_transport;
pub mod thumbnail;
pub mod view_only;

pub use database::{
    AiStatus, Database, DeviceInfo, Face, ImportedPhoto, MapPoint, PeerDevice, PersonWithFace,
    Photo, PhotoSyncInfo, SavedSession, SearchSuggestion,
};
pub use error::{Result, SieguError};
pub use event_bus::{ArcEventBus, CallbackEventBus, EventBus, Level, LogCollector, NullEventBus};
pub use mesh::{MeshManager, SyncEvent, SyncMessage, SyncPhase, SyncProgress};
#[cfg(feature = "ml")]
pub use ml_worker::MlContext;
pub use rpc::ShareMode;
pub use scanner::{extract_photo_metadata, is_media_file, ScanGuard};
pub use server::{generate_pairing_codes, hash_pairing_code, PairingCodes};
pub use shutdown::{check_shutdown, ShutdownCoordinator};
pub use signal::SignalMessage;
pub use signalling::{normalize_signaling_url, ping_signaling, PingOutcome};
