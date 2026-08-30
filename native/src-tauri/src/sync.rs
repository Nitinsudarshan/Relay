//! Poison-tolerant locking.
//!
//! Relay is a long-running desktop recorder: audio capture, the hotkey
//! listener, and the meetings worker all run on their own threads and all
//! reach shared `Mutex` state. `std::sync::Mutex` poisons itself when a
//! thread panics while holding the guard, and every later `lock().unwrap()`
//! on that mutex then panics too. One recoverable panic on a capture thread
//! would therefore take down settings reads, the pill, and the in-progress
//! recording along with it — the user loses a meeting to a fault that had
//! nothing to do with their meeting.
//!
//! `lock_or_recover()` takes the guard through the poison instead. That is a
//! deliberate trade, not a free win: poisoning exists to signal that the
//! protected value may have been left half-updated, and recovering means
//! reading it anyway. It is the right trade *here* because of what these
//! mutexes hold — settings replaced wholesale, an `Option<Session>` handle,
//! sample buffers, cached diagnostics. A torn value in any of those degrades
//! one operation; a poisoned mutex ends the process's usefulness until
//! restart. Something holding an invariant across fields that must not be
//! observed mid-update should not use this, and should say so at its
//! definition.
//!
//! Recovery is never silent — each one logs at `warn` with the call site, so
//! a poisoned lock still shows up as the bug it is.

use std::sync::{Mutex, MutexGuard};

/// Locking that survives a panic in another thread.
pub trait MutexExt<T: ?Sized> {
    /// Locks the mutex, recovering the guard if the lock was poisoned.
    ///
    /// Prefer this over `lock().unwrap()` anywhere the process should keep
    /// running after an unrelated thread panics. Use `lock()` directly when
    /// the caller genuinely needs to handle poisoning itself.
    #[track_caller]
    fn lock_or_recover(&self) -> MutexGuard<'_, T>;
}

impl<T: ?Sized> MutexExt<T> for Mutex<T> {
    #[track_caller]
    fn lock_or_recover(&self) -> MutexGuard<'_, T> {
        match self.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                let location = std::panic::Location::caller();
                tracing::warn!(
                    target: "relay::sync",
                    file = location.file(),
                    line = location.line(),
                    "recovered a poisoned mutex — a thread panicked while holding it, \
                     so the protected value may be mid-update"
                );
                poisoned.into_inner()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// The behaviour this module exists to prevent: after one panic, every
    /// later `lock().unwrap()` panics too.
    #[test]
    fn plain_unwrap_stays_broken_after_a_panic() {
        let mutex = Arc::new(Mutex::new(vec![1, 2, 3]));

        let poisoner = Arc::clone(&mutex);
        let handle = std::thread::spawn(move || {
            let _guard = poisoner.lock().unwrap();
            panic!("capture thread died");
        });
        assert!(handle.join().is_err(), "the spawned thread should panic");

        assert!(mutex.is_poisoned());
        assert!(mutex.lock().is_err(), "the mutex is poisoned for everyone");
    }

    #[test]
    fn lock_or_recover_returns_the_value_through_a_poison() {
        let mutex = Arc::new(Mutex::new(vec![1, 2, 3]));

        let poisoner = Arc::clone(&mutex);
        let handle = std::thread::spawn(move || {
            let mut guard = poisoner.lock().unwrap();
            guard.push(4);
            panic!("capture thread died mid-update");
        });
        assert!(handle.join().is_err());
        assert!(mutex.is_poisoned());

        // The write the panicking thread had already made is visible — this
        // is the "may be mid-update" caveat, made concrete.
        let guard = mutex.lock_or_recover();
        assert_eq!(*guard, vec![1, 2, 3, 4]);
    }

    #[test]
    fn recovery_is_repeatable() {
        let mutex = Arc::new(Mutex::new(0u32));

        let poisoner = Arc::clone(&mutex);
        let handle = std::thread::spawn(move || {
            let _guard = poisoner.lock().unwrap();
            panic!("boom");
        });
        assert!(handle.join().is_err());

        // A poisoned mutex stays poisoned; every subsequent caller must still
        // get through, not just the first one.
        for expected in 1..=3u32 {
            let mut guard = mutex.lock_or_recover();
            *guard += 1;
            assert_eq!(*guard, expected);
        }
    }

    #[test]
    fn behaves_like_lock_when_healthy() {
        let mutex = Mutex::new(String::from("relay"));
        {
            let mut guard = mutex.lock_or_recover();
            guard.push_str("-ok");
        }
        assert_eq!(*mutex.lock_or_recover(), "relay-ok");
        assert!(!mutex.is_poisoned());
    }
}
