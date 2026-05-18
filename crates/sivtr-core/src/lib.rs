pub mod ai;
pub mod buffer;
pub mod capture;
pub mod claude;
pub mod codebuddy;
pub mod codex;
pub mod config;
pub mod export;
pub mod history;
pub mod parse;
pub mod search;
pub mod selection;
pub mod session;

#[cfg(test)]
pub(crate) mod test_env {
    use std::sync::{Mutex, MutexGuard, OnceLock};

    pub(crate) fn lock() -> MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}
