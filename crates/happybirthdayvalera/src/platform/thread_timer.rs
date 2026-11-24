//! A runtime-free timer for native builds.

/// A timer without a runtime: a short-lived thread completes a oneshot.
pub(crate) async fn sleep_ms(ms: u32) {
    let (tx, rx) = futures_channel::oneshot::channel::<()>();
    // The thread only sleeps and sends, so there is no panic to collect and no reason to keep its handle.
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(u64::from(ms)));
        // The waiter may be gone (the listener unmounted); then nobody needs the wake-up.
        let _ = tx.send(());
    });
    // A dropped sender also ends the wait, which is all a timer needs.
    let _ = rx.await;
}
