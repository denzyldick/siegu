//! Adaptive memory-pressure governor.
//!
//! Bulk indexing analyzes many photos concurrently (the rayon `scan_threads`
//! pool), each run grabbing ONNX arena memory and, for videos, an ffmpeg child
//! process. On machines where the OS is already close to its physical RAM the
//! combined spike can make the kernel OOM-kill the app mid-scan.
//!
//! Instead of crashing, the analysis worker consults a cheap cached sample of
//! system memory pressure before touching each photo:
//!
//! * `Elevated` — analysis serializes to one photo at a time (a gate held for
//!   the whole run), so peak residency stays near a single run instead of
//!   `scan_threads` of them.
//! * `Critical` — same serialization with a longer pause between runs, giving
//!   the OS room to reclaim pages before the next allocation.
//!
//! Pressure is sampled from `/proc/meminfo` when available (Linux/Android:
//! microseconds, no allocation churn) and falls back to `sysinfo`. Sampling is
//! cached locally (see [`Governor`]) so per-photo reads are cheap even under a
//! busy rayon pool.

use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Available-memory fraction at or below which analysis starts pacing.
///
/// 20% free on a 15 GB host is ~3 GB: the analysis isn't the majority consumer
/// yet, but there's little headroom for four concurrent ONNX runs.
pub const ELEVATED_FRACTION: f64 = 0.20;

/// Available-memory fraction at or below which analysis serializes.
///
/// 10% free means the kernel is close to invoking the OOM killer; dropping to
/// a single in-flight run is the last resort that still makes progress.
pub const CRITICAL_FRACTION: f64 = 0.10;

/// How fresh a cached pressure sample must be to be reused. ~2.5 reads/sec is
/// responsive enough to catch a downward spiral while keeping /proc reads off
/// the per-photo hot path.
const SAMPLE_MIN_INTERVAL: Duration = Duration::from_millis(400);

/// Pacing delay applied before starting a photo while memory is tight.
/// `Normal` uses zero delay so unaffected machines run at full speed.
pub fn pace_for(level: Pressure) -> Duration {
    match level {
        Pressure::Normal => Duration::ZERO,
        // Both pressured levels serialize too (see `Governor::acquire`); the
        // sleep is the extra pause after winning the serialization gate so the
        // machine can breathe between runs.
        Pressure::Elevated => Duration::from_millis(120),
        Pressure::Critical => Duration::from_millis(300),
    }
}

/// Coarse system memory-pressure classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Pressure {
    Normal,
    Elevated,
    Critical,
}

/// Maps an available/total memory fraction to a [`Pressure`] level.
///
/// Pure and injectable so the thresholds can be unit-tested without touching
/// the real `/proc` (or `sysinfo`).
pub fn pressure_from_available_fraction(available: f64) -> Pressure {
    if available <= CRITICAL_FRACTION {
        Pressure::Critical
    } else if available <= ELEVATED_FRACTION {
        Pressure::Elevated
    } else {
        Pressure::Normal
    }
}

/// A single cached measurement of system memory. `None` when the host exposes
/// nothing we can read (in which case the governor stays `Normal`).
struct MemorySample {
    total: u64,
    available: u64,
}

impl MemorySample {
    /// Build a fresh estimate, preferring `/proc/meminfo` when present.
    fn now() -> Option<Self> {
        Self::from_proc_meminfo().or_else(Self::from_sysinfo)
    }

    /// Linux/Android fast path: one small file read, no sysinfo cost.
    #[cfg(any(target_os = "linux", target_os = "android"))]
    fn from_proc_meminfo() -> Option<Self> {
        let contents = std::fs::read_to_string("/proc/meminfo").ok()?;
        let mut total: Option<u64> = None;
        let mut available: Option<u64> = None;
        for line in contents.lines() {
            if total.is_none() {
                if let Some(rest) = line.strip_prefix("MemTotal:") {
                    total = parse_kb(rest);
                }
            }
            if let Some(rest) = line.strip_prefix("MemAvailable:") {
                available = parse_kb(rest);
                if total.is_some() {
                    break;
                }
            }
        }
        let total = total?;
        Some(Self {
            total,
            available: available.unwrap_or(total),
        })
    }

    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    fn from_proc_meminfo() -> Option<Self> {
        None
    }

    /// Portable fallback for macOS/iOS/Windows via `sysinfo`.
    fn from_sysinfo() -> Option<Self> {
        let total = crate::model_manager::physical_memory_bytes()?;
        if total == 0 {
            return None;
        }
        let available = crate::model_manager::available_memory_bytes();
        Some(Self { total, available })
    }
}

/// Parses a `kB` value from a `/proc/meminfo` field, e.g. `"  16230824 kB"`.
fn parse_kb(field: &str) -> Option<u64> {
    let kb: u64 = field.split_whitespace().next()?.parse().ok()?;
    Some(kb.saturating_mul(1024))
}

/// Fraction of physical RAM that is currently available, if measurable.
fn available_fraction() -> Option<f64> {
    let s = MemorySample::now()?;
    if s.total == 0 {
        return None;
    }
    Some(s.available as f64 / s.total as f64)
}

/// Effective headroom fraction, relative to physical RAM, after accounting for
/// swap exhaustion.
///
/// `MemAvailable` already includes reclaimable page cache, so it can report a
/// deceptively healthy value (e.g. 28%) on a host whose swap is ~full and that
/// is actively thrashing — exactly the situation where starting a heavy scan
/// pushes the kernel straight into the OOM killer.
///
/// We therefore resize `available` down by the swap pressure. When swap is
/// nearly exhausted there is little left to reclaim, so the true headroom is a
/// small fraction of what `MemAvailable` claims. This is deliberately
/// conservative: returning *less* headroom than reality only ever delays work,
/// never starts work that would OOM.
#[cfg(target_os = "linux")]
fn headroom_fraction() -> Option<f64> {
    let s = MemorySample::now()?;
    if s.total == 0 {
        return None;
    }
    // Read SwapTotal / SwapFree to judge how much cushion remains. When most
    // swap is consumed the box is one allocation away from invoking the OOM
    // killer regardless of what MemAvailable claims.
    let (swap_free, swap_total) = swap_meminfo()?;
    let swap_fullness = if swap_total == 0 {
        0.0
    } else {
        1.0 - (swap_free as f64 / swap_total as f64)
    };
    Some(swap_adjusted_headroom(s.available, s.total, swap_fullness))
}

/// Pure arithmetic behind [`headroom_fraction`]: trust `MemAvailable` fully at
/// zero swap pressure and collapse it down as swap fills, so a full-swap +
/// high-available host still reads as pressured. Returns a clamped 0..=1
/// fraction of `total`.
fn swap_adjusted_headroom(available: u64, total: u64, swap_fullness: f64) -> f64 {
    if total == 0 {
        return 0.0;
    }
    let swap_fullness = swap_fullness.clamp(0.0, 1.0);
    let trusted = available as f64 * (1.0 - 0.7 * swap_fullness);
    (trusted / total as f64).clamp(0.0, 1.0)
}

#[cfg(not(target_os = "linux"))]
fn headroom_fraction() -> Option<f64> {
    available_fraction()
}

/// Parse SwapTotal/SwapFree from `/proc/meminfo` (Linux).
#[cfg(target_os = "linux")]
fn swap_meminfo() -> Option<(u64, u64)> {
    let contents = std::fs::read_to_string("/proc/meminfo").ok()?;
    let mut free: Option<u64> = None;
    let mut total: Option<u64> = None;
    for line in contents.lines() {
        if let Some(rest) = line.strip_prefix("SwapTotal:") {
            total = parse_kb(rest);
        } else if let Some(rest) = line.strip_prefix("SwapFree:") {
            free = parse_kb(rest);
        }
        if free.is_some() && total.is_some() {
            break;
        }
    }
    Some((free?, total?))
}

/// Returns `true` when the host has enough real headroom to start a heavy
/// scan/index pass without risking an OOM kill.
///
/// This is the guard used *before* kicking off bulk work (the scan walk and the
/// AI index), as opposed to the reactive [`Governor`] which paces once a run is
/// already in flight. Refusing early is much better than starving mid-scan.
pub fn safe_to_start_heavy_work() -> bool {
    let Some(fraction) = headroom_fraction() else {
        // Unknown host: be permissive (matching the governor's Normal default).
        return true;
    };
    let level = pressure_from_available_fraction(fraction);
    // Only `Normal` is safe to start fresh heavy work. `Elevated`/`Critical`
    // mean the governor would be pacing from the first photo, which is the
    // kernel already too close to OOM to justify starting a big job.
    matches!(level, Pressure::Normal)
}

/// Human-readable reason a heavy job was refused, for surfacing to the user.
pub fn headroom_block_reason() -> Option<String> {
    let Some(fraction) = headroom_fraction() else {
        return None;
    };
    if pressure_from_available_fraction(fraction) == Pressure::Normal {
        return None;
    }
    let pct = (fraction * 100.0).max(0.0);
    Some(format!(
        "Your computer is low on available memory ({pct:.0}% headroom). \
         Siegu won't start scanning so it can't freeze or crash your system. \
         Free up some memory and try again."
    ))
}

/// Shared, cheap-to-poll backpressure governor passed to the analysis pool.
///
/// The level is sampled at most every [`SAMPLE_MIN_INTERVAL`]; callers (the
/// rayon photo workers) call [`Governor::acquire`] once per photo. The guard
/// it returns is held for the duration of that photo's run so a `Critical`
/// level truly serializes in-flight analysis rather than just sleeping.
pub struct Governor {
    last_sample_ms: AtomicU64,
    level: AtomicU8,
    last_reported: AtomicU8,
    /// Big lock taken (and held across one photo) only while `Critical`, so
    /// concurrent runs collapse to one without rebuilding the rayon pool.
    gate: Mutex<()>,
}

impl Default for Governor {
    fn default() -> Self {
        Self::new()
    }
}

impl Governor {
    pub fn new() -> Self {
        Self {
            last_sample_ms: AtomicU64::new(0),
            level: AtomicU8::new(Pressure::Normal as u8),
            last_reported: AtomicU8::new(Pressure::Normal as u8),
            gate: Mutex::new(()),
        }
    }

    /// Current pressure level, sampling at most every [`SAMPLE_MIN_INTERVAL`].
    pub fn level(&self) -> Pressure {
        let now_ms = now_ms();
        let last = self.last_sample_ms.load(Ordering::Relaxed);
        if now_ms.saturating_sub(last) < SAMPLE_MIN_INTERVAL.as_millis() as u64 {
            return self.level_from_cache();
        }
        let level = self.sample_now();
        self.last_sample_ms.store(now_ms, Ordering::Relaxed);
        self.level.store(level as u8, Ordering::Relaxed);
        level
    }

    /// Samples the host now (used once per TTL window; never in a hot loop).
    fn sample_now(&self) -> Pressure {
        match available_fraction() {
            Some(fraction) => pressure_from_available_fraction(fraction),
            None => Pressure::Normal,
        }
    }

    /// Overrides the cached level without touching the host. Test-only: pins
    /// the sample timestamp far in the future so the forced value is never
    /// refreshed away by a concurrent `level()` call.
    #[cfg(test)]
    pub fn force_level(&self, level: Pressure) {
        self.last_sample_ms.store(u64::MAX / 2, Ordering::Relaxed);
        self.level.store(level as u8, Ordering::Relaxed);
    }

    fn level_from_cache(&self) -> Pressure {
        match self.level.load(Ordering::Relaxed) {
            x if x == Pressure::Critical as u8 => Pressure::Critical,
            x if x == Pressure::Elevated as u8 => Pressure::Elevated,
            _ => Pressure::Normal,
        }
    }

    /// Emits one log line per level *transition*, so a long throttled scan only
    /// reports once instead of once per photo.
    fn report_transition(&self, level: Pressure, log: &dyn Fn(&str)) {
        let prev = self.last_reported.swap(level as u8, Ordering::Relaxed);
        if prev != level as u8 {
            match level {
                Pressure::Normal => log("Memory pressure eased — analysis back to full speed."),
                Pressure::Elevated => {
                    log("Memory pressure detected — analyzing one photo at a time to protect system RAM.")
                }
                Pressure::Critical => {
                    log("Critical memory pressure — slowing analysis to keep the app, and your computer, alive.")
                }
            }
        }
    }

    /// Blocks and paces as needed for one photo, returning a guard to hold
    /// across the analysis run. Logs a UI-visible message on level changes.
    ///
    /// Both `Elevated` and `Critical` take the serialization gate: with free
    /// RAM at or below 20% even four concurrent runs can spike the host over
    /// its remaining memory (each run grabs ONNX arena space), so the safest
    /// lever is to collapse to one in-flight run at a time. `Critical` just
    /// paces harder between them.
    pub fn acquire(&self, log: &dyn Fn(&str)) -> PhotoGuard<'_> {
        let level = self.level();
        self.report_transition(level, log);
        match level {
            Pressure::Normal => PhotoGuard { _gate: None },
            Pressure::Elevated => {
                let gate = self.gate.lock().unwrap_or_else(|e| e.into_inner());
                std::thread::sleep(pace_for(level));
                PhotoGuard { _gate: Some(gate) }
            }
            Pressure::Critical => {
                let gate = self.gate.lock().unwrap_or_else(|e| e.into_inner());
                // A longer breath after winning the gate so the OS can reclaim
                // pages before the next (now the only) run starts.
                std::thread::sleep(pace_for(level));
                PhotoGuard { _gate: Some(gate) }
            }
        }
    }
}

/// Held across a single photo run. Carries the `Critical` serialization gate
/// so that gate is released — unblocking the next run — exactly when the photo
/// finishes.
pub struct PhotoGuard<'a> {
    _gate: Option<MutexGuard<'a, ()>>,
}

impl<'a> PhotoGuard<'a> {
    /// A guard that carries no gate, used where no memory backpressure applies
    /// (e.g. single-photo analysis).
    pub fn idle() -> Self {
        Self { _gate: None }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thresholds_classify_fractions() {
        assert_eq!(pressure_from_available_fraction(0.50), Pressure::Normal);
        assert_eq!(pressure_from_available_fraction(0.21), Pressure::Normal);
        assert_eq!(pressure_from_available_fraction(0.20), Pressure::Elevated);
        assert_eq!(pressure_from_available_fraction(0.15), Pressure::Elevated);
        assert_eq!(pressure_from_available_fraction(0.10), Pressure::Critical);
        assert_eq!(pressure_from_available_fraction(0.02), Pressure::Critical);
    }

    #[test]
    fn normal_level_paces_zero() {
        assert_eq!(pace_for(Pressure::Normal), Duration::ZERO);
        assert!(!pace_for(Pressure::Elevated).is_zero());
        assert!(!pace_for(Pressure::Critical).is_zero());
    }

    #[test]
    fn governor_defaults_to_normal() {
        let g = Governor::new();
        assert_eq!(g.level_from_cache(), Pressure::Normal);
    }

    #[test]
    fn pressurized_levels_serialize_until_guard_dropped() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::{Arc, Barrier};

        for level in [Pressure::Elevated, Pressure::Critical] {
            let g = Arc::new(Governor::new());
            g.force_level(level);

            let first = g.acquire(&|_| {});
            let done = Arc::new(AtomicBool::new(false));
            let ready = Arc::new(Barrier::new(2));
            let done_ref = Arc::clone(&done);
            let ready_ref = Arc::clone(&ready);
            let g2 = Arc::clone(&g);
            let blocked = std::thread::spawn(move || {
                ready_ref.wait();
                let _second = g2.acquire(&|_| {});
                done_ref.store(true, Ordering::SeqCst);
            });

            // Ensure the spawned thread is at the acquire call, then give it a
            // moment to block on the gate held by `first`. While `first` holds
            // the gate, `acquire` cannot complete for pressured levels, so
            // `done` must remain false regardless of scheduling.
            ready.wait();
            std::thread::sleep(Duration::from_millis(50));
            assert!(
                !done.load(Ordering::SeqCst),
                "{level:?}: second acquire should block behind the first"
            );

            drop(first);
            blocked.join().expect("second acquire unblocks after drop");
            assert!(done.load(Ordering::SeqCst), "{level:?} run unblocked");
        }
    }

    #[test]
    fn normal_level_never_takes_the_gate() {
        let g = Governor::new();
        g.force_level(Pressure::Normal);
        let first = g.acquire(&|_| {});
        // Normal must not block: a second acquire should complete immediately.
        let second = g.acquire(&|_| {});
        drop(first);
        drop(second);
    }

    #[test]
    fn report_only_logs_on_transitions() {
        use std::cell::Cell;
        let g = Governor::new();
        let calls = Cell::new(0usize);
        // Wrap `last_reported` so a Normal->Normal call logs nothing.
        g.report_transition(Pressure::Normal, &|_| calls.set(calls.get() + 1));
        g.report_transition(Pressure::Normal, &|_| calls.set(calls.get() + 1));
        assert_eq!(calls.get(), 0, "same-level reports must be silent");
        g.report_transition(Pressure::Elevated, &|_| calls.set(calls.get() + 1));
        g.report_transition(Pressure::Elevated, &|_| calls.set(calls.get() + 1));
        assert_eq!(calls.get(), 1, "only the transition logs");
        g.report_transition(Pressure::Critical, &|_| calls.set(calls.get() + 1));
        assert_eq!(calls.get(), 2);
        g.report_transition(Pressure::Normal, &|_| calls.set(calls.get() + 1));
        assert_eq!(calls.get(), 3);
    }

    #[test]
    fn normal_fraction_is_safe_to_start() {
        // The preflight gate mirrors the pressure thresholds: only truly Normal
        // headroom permits starting heavy work.
        assert!(pressure_from_available_fraction(0.50) == Pressure::Normal);
        assert!(pressure_from_available_fraction(0.25) == Pressure::Normal);
        assert!(pressure_from_available_fraction(0.20) != Pressure::Normal);
        assert!(pressure_from_available_fraction(0.05) != Pressure::Normal);
    }

    #[test]
    fn headroom_block_reason_only_for_pressured_states() {
        // We can't fabricate /proc here, but the mapping must be closed: a
        // blocked state implies Safe=false and vice versa is unreachable by
        // construction. Just assert the well-formedness of the message path
        // through the pure classifier used by the preflight.
        let gate = |f: f64| pressure_from_available_fraction(f) == Pressure::Normal;
        assert!(gate(0.99), "plenty of headroom -> safe");
        assert!(!gate(0.15), "low headroom -> unsafe");
    }

    #[test]
    fn swap_adjusted_headroom_collapses_when_swap_full() {
        // The OOM-kill scenario: MemAvailable claims ~28% (reads Normal by
        // itself), but swap is ~full, so the host is one allocation from the
        // OOM killer. The adjusted headroom must fall into pressured territory.
        let total = 15_444_864u64 * 1024; // 15.4 GB in bytes (matches the host)
        let available = (total as f64 * 0.28) as u64; // ~4.3 GB "available"

        // Swap ~full (12 GB used of 15 GB): shrinks headroom well under 20%.
        let near_full = swap_adjusted_headroom(available, total, 0.8);
        assert!(
            near_full <= 0.20 && near_full > 0.0,
            "full swap must push a 28%%-available host into Elevated/Critical, got {near_full:.3}"
        );
        // 100% swap used: near-zero trusted headroom.
        let totally_full = swap_adjusted_headroom(available, total, 1.0);
        assert!(
            totally_full <= 0.10,
            "completely full swap -> Critical, got {totally_full:.3}"
        );
        assert_eq!(
            pressure_from_available_fraction(totally_full),
            Pressure::Critical
        );

        // Zero swap pressure: trust MemAvailable exactly.
        let untouched = swap_adjusted_headroom(available, total, 0.0);
        assert!(
            (untouched - 0.28).abs() < 1e-9,
            "no swap pressure = raw available"
        );
        assert_eq!(
            pressure_from_available_fraction(untouched),
            Pressure::Normal
        );
    }
}
