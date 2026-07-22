//! Global state management

use crate::ipc_client::IpcClient;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{error, info, warn};

/// Global IPC client instance
static IPC_CLIENT: Lazy<Mutex<Option<IpcClient>>> = Lazy::new(|| Mutex::new(None));

/// Global object count for DllCanUnloadNow
static OBJECT_COUNT: AtomicU32 = AtomicU32::new(0);

/// Retry state: timestamp (ms since epoch) of next allowed retry attempt
static NEXT_RETRY_TIME: AtomicU64 = AtomicU64::new(0);

/// Consecutive failure count for exponential backoff
static FAILURE_COUNT: AtomicU32 = AtomicU32::new(0);

/// Maximum backoff: 30 seconds
const MAX_BACKOFF_MS: u64 = 30_000;

/// Get current time in milliseconds since epoch
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Get or create the global IPC client
pub fn get_ipc_client() -> parking_lot::MutexGuard<'static, Option<IpcClient>> {
    let mut guard = IPC_CLIENT.lock();
    if guard.is_none() {
        // Check if we should retry (exponential backoff)
        let next_retry = NEXT_RETRY_TIME.load(Ordering::SeqCst);
        let current_time = now_ms();
        if current_time < next_retry {
            // Still in backoff period
            let remaining = next_retry - current_time;
            info!("get_ipc_client: waiting {}ms before retry", remaining);
            return guard;
        }

        info!("get_ipc_client: connecting to smac-helper...");

        // Try to connect to the helper
        match IpcClient::connect() {
            Ok(client) => {
                info!("get_ipc_client: connected successfully");
                info!("  endpoint_id: {:?}", &client.endpoint_id_bytes()[..8]);
                info!("  ticket: {}", client.our_ticket());
                *guard = Some(client);
                // Reset failure count on success
                FAILURE_COUNT.store(0, Ordering::SeqCst);
                NEXT_RETRY_TIME.store(0, Ordering::SeqCst);
            }
            Err(e) => {
                error!("get_ipc_client: Failed to connect to helper: {:?}", e);
                error!("  Make sure smac-helper is running before launching the game");

                // Exponential backoff: 1s, 2s, 4s, 8s, ... up to MAX_BACKOFF_MS
                let failures = FAILURE_COUNT.fetch_add(1, Ordering::SeqCst) + 1;
                let backoff_ms = std::cmp::min(1000 * (1u64 << (failures - 1)), MAX_BACKOFF_MS);
                NEXT_RETRY_TIME.store(current_time + backoff_ms, Ordering::SeqCst);
                warn!("get_ipc_client: will retry in {}ms (attempt {})", backoff_ms, failures);
            }
        }
    } else {
        info!("get_ipc_client: using existing connection");
    }
    guard
}

/// Check if IPC client is connected
pub fn has_ipc_client() -> bool {
    IPC_CLIENT.lock().is_some()
}

/// Cleanup on DLL unload
pub fn cleanup() {
    *IPC_CLIENT.lock() = None;
}

/// Increment object count
pub fn inc_object_count() -> u32 {
    OBJECT_COUNT.fetch_add(1, Ordering::SeqCst) + 1
}

/// Decrement object count
pub fn dec_object_count() -> u32 {
    OBJECT_COUNT.fetch_sub(1, Ordering::SeqCst) - 1
}

/// Get object count
pub fn get_object_count() -> u32 {
    OBJECT_COUNT.load(Ordering::SeqCst)
}
