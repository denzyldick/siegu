use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::watch;

pub struct ShutdownCoordinator {
    sender: watch::Sender<bool>,
    shutdown_flag: Arc<AtomicBool>,
}

impl ShutdownCoordinator {
    pub fn new() -> Self {
        let (sender, _) = watch::channel(false);
        Self {
            sender,
            shutdown_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn signal(&self) {
        self.shutdown_flag.store(true, Ordering::SeqCst);
        let _ = self.sender.send(true);
    }

    pub fn is_shutdown(&self) -> bool {
        self.shutdown_flag.load(Ordering::SeqCst)
    }

    pub fn receiver(&self) -> watch::Receiver<bool> {
        self.sender.subscribe()
    }

    pub fn flag(&self) -> Arc<AtomicBool> {
        self.shutdown_flag.clone()
    }
}

impl Default for ShutdownCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for ShutdownCoordinator {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            shutdown_flag: self.shutdown_flag.clone(),
        }
    }
}

pub async fn wait_for_shutdown(coordinator: &ShutdownCoordinator) {
    let mut rx = coordinator.receiver();
    let _ = rx.changed().await;
}

pub fn check_shutdown(flag: &AtomicBool) -> bool {
    flag.load(Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shutdown_coordinator_new() {
        let coord = ShutdownCoordinator::new();
        assert!(!coord.is_shutdown());
    }

    #[test]
    fn test_shutdown_coordinator_signal() {
        let coord = ShutdownCoordinator::new();
        coord.signal();
        assert!(coord.is_shutdown());
    }

    #[test]
    fn test_shutdown_coordinator_flag() {
        let coord = ShutdownCoordinator::new();
        let flag = coord.flag();
        assert!(!flag.load(Ordering::SeqCst));
        coord.signal();
        assert!(flag.load(Ordering::SeqCst));
    }

    #[test]
    fn test_check_shutdown() {
        let flag = Arc::new(AtomicBool::new(false));
        assert!(!check_shutdown(&flag));
        flag.store(true, Ordering::SeqCst);
        assert!(check_shutdown(&flag));
    }

    #[tokio::test]
    async fn test_wait_for_shutdown() {
        let coord = ShutdownCoordinator::new();
        let coord2 = coord.clone();

        let handle = tokio::spawn(async move {
            wait_for_shutdown(&coord2).await;
        });

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        coord.signal();

        let result = tokio::time::timeout(std::time::Duration::from_secs(1), handle).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_shutdown_coordinator_clone() {
        let coord1 = ShutdownCoordinator::new();
        let coord2 = coord1.clone();
        coord1.signal();
        assert!(coord2.is_shutdown());
    }
}
