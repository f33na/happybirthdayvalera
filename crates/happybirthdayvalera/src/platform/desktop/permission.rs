//! Microphone authorization before the stream opens.
//!
//! On macOS, cpal opens a microphone that the user denied without any error and then delivers silence, so
//! the widget would say "Didn't catch that". The adapter asks `AVFoundation` first. On other systems the OS
//! reports a denial through cpal, and the silence guard in `capture` covers the rest.

use std::future::Future;

use crate::machine::ErrorCode;

/// The OS answer, reduced to what the adapter acts on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MicAuth {
    Authorized,
    NotDetermined,
    /// Denied by the user, or restricted by a policy.
    Denied,
}

/// The error for a stream whose first samples are all exactly zero. On macOS that is how a denied
/// microphone looks; elsewhere it means a broken device or driver.
pub(crate) const SILENT_INPUT: ErrorCode =
    if cfg!(target_os = "macos") { ErrorCode::MicDenied } else { ErrorCode::MicUnavailable };

/// Resolves access before the stream opens. While it waits for the user's answer, the listener stays in
/// `requesting`.
pub(crate) async fn ensure_access() -> Result<(), ErrorCode> {
    resolve(os::status(), os::request).await
}

/// The decision, apart from the OS calls so that tests can stub both.
async fn resolve<F, Fut>(status: MicAuth, request: F) -> Result<(), ErrorCode>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = bool>,
{
    match status {
        MicAuth::Authorized => Ok(()),
        MicAuth::Denied => Err(ErrorCode::MicDenied),
        MicAuth::NotDetermined => {
            if request().await {
                Ok(())
            } else {
                Err(ErrorCode::MicDenied)
            }
        }
    }
}

#[cfg(target_os = "macos")]
#[allow(unsafe_code, reason = "AVFoundation calls; each unsafe block states why it is sound")]
mod os {
    use std::sync::Mutex;

    use block2::RcBlock;
    use futures_channel::oneshot;
    use objc2::runtime::Bool;
    use objc2_av_foundation::{AVAuthorizationStatus, AVCaptureDevice, AVMediaType, AVMediaTypeAudio};

    use super::MicAuth;

    fn audio() -> Option<&'static AVMediaType> {
        // SAFETY: `AVMediaTypeAudio` is an immutable NSString constant that AVFoundation exports for the whole
        // life of the process; reading it has no side effects.
        unsafe { AVMediaTypeAudio }
    }

    pub(super) fn status() -> MicAuth {
        let Some(audio) = audio() else {
            tracing::warn!("AVMediaTypeAudio is missing; cpal and the silence guard decide instead");
            return MicAuth::Authorized;
        };
        // SAFETY: a class method that only reads the privacy database; the argument is the audio media type,
        // one of the two values the method accepts.
        let status = unsafe { AVCaptureDevice::authorizationStatusForMediaType(audio) };
        match status {
            AVAuthorizationStatus::Authorized => MicAuth::Authorized,
            AVAuthorizationStatus::NotDetermined => MicAuth::NotDetermined,
            AVAuthorizationStatus::Denied | AVAuthorizationStatus::Restricted => MicAuth::Denied,
            // An open integer type from the SDK: a status that a later macOS adds must not block capture.
            other => {
                tracing::warn!("unknown microphone authorization status {}", other.0);
                MicAuth::Authorized
            }
        }
    }

    pub(super) async fn request() -> bool {
        let Some(audio) = audio() else {
            return true;
        };
        let (tx, rx) = oneshot::channel::<bool>();
        // AVFoundation wants a `Fn` block and calls it once, on an arbitrary queue; the slot hands the
        // one-shot sender to that single call.
        let slot = Mutex::new(Some(tx));
        let handler = RcBlock::new(move |granted: Bool| {
            if let Ok(mut slot) = slot.lock()
                && let Some(tx) = slot.take()
            {
                // A failed send means the listener stopped waiting; the answer is not needed then.
                let _ = tx.send(granted.as_bool());
            }
        });
        // SAFETY: AVFoundation copies the block before it returns, so the block outlives this call; the block
        // only touches a Mutex and a channel, which are safe to use from any thread.
        unsafe { AVCaptureDevice::requestAccessForMediaType_completionHandler(audio, &handler) };
        // A dropped handler without an answer is treated as a refusal: the stream would only be silent.
        rx.await.unwrap_or(false)
    }
}

#[cfg(not(target_os = "macos"))]
mod os {
    use super::MicAuth;

    /// Other systems report a denial through cpal itself.
    pub(super) fn status() -> MicAuth {
        MicAuth::Authorized
    }

    pub(super) fn request() -> std::future::Ready<bool> {
        std::future::ready(true)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use futures_executor::block_on;

    use super::*;

    #[test]
    fn authorized_opens_without_asking() {
        let asked = Cell::new(false);
        let result = block_on(resolve(MicAuth::Authorized, || {
            asked.set(true);
            std::future::ready(true)
        }));
        assert_eq!(result, Ok(()));
        assert!(!asked.get());
    }

    #[test]
    fn denied_fails_without_asking() {
        let asked = Cell::new(false);
        let result = block_on(resolve(MicAuth::Denied, || {
            asked.set(true);
            std::future::ready(true)
        }));
        assert_eq!(result, Err(ErrorCode::MicDenied));
        assert!(!asked.get());
    }

    #[test]
    fn not_determined_follows_the_answer() {
        assert_eq!(block_on(resolve(MicAuth::NotDetermined, || std::future::ready(true))), Ok(()));
        assert_eq!(
            block_on(resolve(MicAuth::NotDetermined, || std::future::ready(false))),
            Err(ErrorCode::MicDenied)
        );
    }
}
