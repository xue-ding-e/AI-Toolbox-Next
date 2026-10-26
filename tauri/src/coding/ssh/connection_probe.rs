//! Temporary-connection lifecycle, shared by the russh adapter and pure fakes.
use std::future::Future;
use std::time::Duration;

use super::super::types::{CleanupWarning, ProbeReason, SSHConnectionResult, SSHSyncCapability};

pub(super) const OUTPUT_LIMIT: usize = 64 * 1024;

#[derive(Default, Debug)]
pub(super) struct ProbeOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    status: Option<u32>,
    conflicting_status: bool,
    signalled: bool,
    closed: bool,
}

impl ProbeOutput {
    pub fn append(&mut self, data: &[u8], stderr: bool) -> Result<(), ProbeReason> {
        // Check raw, combined bytes BEFORE extending either buffer or decoding.
        let remaining = OUTPUT_LIMIT - self.stdout.len() - self.stderr.len();
        if data.len() > remaining {
            return Err(ProbeReason::OutputLimitExceeded);
        }
        if stderr { &mut self.stderr } else { &mut self.stdout }.extend_from_slice(data);
        Ok(())
    }

    pub fn status(&mut self, status: u32) {
        if self.status.is_some_and(|previous| previous != status) {
            self.conflicting_status = true;
        }
        self.status = Some(status);
    }

    pub fn signal(&mut self) { self.signalled = true; }
    pub fn close(&mut self) { self.closed = true; }

    pub fn capability(&self) -> (SSHSyncCapability, Option<ProbeReason>) {
        use ProbeReason::*;
        let reason = if !self.closed { Some(ChannelClosedEarly) }
            else if self.signalled { Some(ExitSignal) }
            else if self.conflicting_status { Some(ConflictingExitStatus) }
            else if self.status.is_none() { Some(MissingExitStatus) }
            else if self.status != Some(0) { Some(NonzeroExitStatus) }
            else { None };
        if let Some(reason) = reason {
            return (SSHSyncCapability::Unknown, Some(reason));
        }
        // Match raw stdout only. Accept the probe's LF and SSH CRLF variants,
        // not arbitrary whitespace, banners, substrings or decoded stderr.
        let token = self.stdout.strip_suffix(b"\r\n")
            .or_else(|| self.stdout.strip_suffix(b"\n")).unwrap_or(&self.stdout);
        match token {
            b"AI_TOOLBOX_POSIX_SYNC_V1" => (SSHSyncCapability::Supported, None),
            b"AI_TOOLBOX_POSIX_SYNC_NO_HOME" => (SSHSyncCapability::MissingHome, None),
            _ => (SSHSyncCapability::Unknown, Some(UnexpectedOutput)),
        }
    }
}

/// Small stream seam: the real russh channel and synthetic queues use exactly
/// the same drain, accumulator and remote-close confirmation logic.
pub(super) trait ChannelSource {
    fn next(&mut self) -> impl Future<Output = Option<russh::ChannelMsg>> + Send;
}

pub(super) async fn collect<C: ChannelSource>(
    channel: &mut C,
    closed: tokio::sync::oneshot::Receiver<u32>,
    expected_channel: u32,
) -> Result<ProbeOutput, ProbeReason> {
    use russh::ChannelMsg;
    let mut output = ProbeOutput::default();
    while let Some(message) = channel.next().await {
        match message {
            ChannelMsg::Data { data } => output.append(&data, false)?,
            ChannelMsg::ExtendedData { data, ext: 1 } => output.append(&data, true)?,
            ChannelMsg::ExitStatus { exit_status } => output.status(exit_status),
            ChannelMsg::ExitSignal { .. } => output.signal(),
            ChannelMsg::Failure => return Err(ProbeReason::ChannelFailure),
            _ => {}
        }
    }
    // EOF is not completion. Always drain to None first, even when the close
    // notification arrived early, so queued tail bytes/status cannot be lost.
    // The caller's single probe deadline also covers this callback wait.
    if closed.await.map_err(|_| ProbeReason::ChannelClosedEarly)? != expected_channel {
        return Err(ProbeReason::ChannelClosedEarly);
    }
    output.close();
    Ok(output)
}

pub(super) trait Transport {
    fn authenticate(&mut self) -> impl Future<Output = Result<(), String>> + Send;
    // This future includes channel open, exec request and all output collection.
    fn probe(&mut self, command: &str) -> impl Future<Output = Result<ProbeOutput, ProbeReason>> + Send;
    fn disconnect(&mut self) -> impl Future<Output = Result<(), ()>> + Send;
}

#[derive(Clone, Copy)]
pub(super) struct Deadlines {
    pub auth: Duration,
    pub probe: Duration,
    pub cleanup: Duration,
}

impl Default for Deadlines {
    fn default() -> Self {
        Self { auth: Duration::from_secs(10), probe: Duration::from_secs(10), cleanup: Duration::from_secs(2) }
    }
}

pub(super) struct Outcome {
    pub authentication: Result<(), String>,
    pub probe: Option<Result<ProbeOutput, ProbeReason>>,
    pub cleanup_warning: Option<CleanupWarning>,
}

impl Outcome {
    pub fn into_result(self) -> SSHConnectionResult {
        let (sync_capability, probe_reason) = match self.probe {
            Some(Ok(output)) => {
                let (capability, reason) = output.capability();
                (Some(capability), reason)
            }
            Some(Err(reason)) => (Some(SSHSyncCapability::Unknown), Some(reason)),
            None => (None, None),
        };
        SSHConnectionResult {
            connected: self.authentication.is_ok(),
            error: self.authentication.err(),
            // A POSIX token is not server information; no OS/banner command.
            server_info: None,
            sync_capability,
            probe_reason,
            cleanup_warning: self.cleanup_warning,
        }
    }
}

/// Every normal return after obtaining a handle attempts disconnect exactly once.
/// Cancellation/dropping this future cannot guarantee asynchronous cleanup.
/// Tokio deadlines bound asynchronous waits, not synchronous private-key reads
/// or parsing performed by the existing authentication implementation.
pub(super) async fn run<T: Transport>(transport: &mut T, command: &str, limits: Deadlines) -> Outcome {
    let authentication = tokio::time::timeout(limits.auth, transport.authenticate()).await
        .unwrap_or_else(|_| Err("SSH 认证超时".to_string()));
    let probe = if authentication.is_ok() {
        Some(tokio::time::timeout(limits.probe, transport.probe(command)).await
            .unwrap_or(Err(ProbeReason::ProbeTimeout)))
    } else { None };
    let cleanup_warning = match tokio::time::timeout(limits.cleanup, transport.disconnect()).await {
        Ok(Ok(())) => None,
        Ok(Err(())) => Some(CleanupWarning::DisconnectFailed),
        Err(_) => Some(CleanupWarning::DisconnectTimeout),
    };
    Outcome { authentication, probe, cleanup_warning }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy)]
    enum Mode { Ok, Fail, Pending, Streaming, UnclosedStream, DrainedWithoutClose }

    struct Fake {
        auth: Mode,
        probe: Mode,
        cleanup: Mode,
        calls: Vec<&'static str>,
        chunks: usize,
        pending_close: Option<tokio::sync::oneshot::Receiver<u32>>,
        drained: Option<tokio::sync::oneshot::Sender<()>>,
    }

    impl Default for Fake {
        fn default() -> Self {
            Self {
                auth: Mode::Ok, probe: Mode::Ok, cleanup: Mode::Ok, calls: vec![], chunks: 0,
                pending_close: None, drained: None,
            }
        }
    }

    impl Transport for Fake {
        async fn authenticate(&mut self) -> Result<(), String> {
            self.calls.push("auth");
            match self.auth {
                Mode::Fail => Err("authentication rejected".into()),
                Mode::Pending => std::future::pending().await,
                _ => Ok(()),
            }
        }

        async fn probe(&mut self, command: &str) -> Result<ProbeOutput, ProbeReason> {
            self.calls.push("probe");
            assert_eq!(command, super::super::super::capabilities::POSIX_SYNC_PROBE);
            match self.probe {
                Mode::Fail => Err(ProbeReason::ExecFailed),
                Mode::Pending => std::future::pending().await,
                Mode::Streaming => {
                    let mut output = ProbeOutput::default();
                    loop {
                        // Incoming data must not restart the one probe deadline.
                        output.append(b"x", false)?;
                        self.chunks += 1;
                        tokio::time::sleep(Duration::from_millis(1)).await;
                    }
                }
                Mode::UnclosedStream => {
                    let mut queue = Queue::new(success_messages());
                    queue.pending_at_end = true;
                    let (closed_tx, closed_rx) = tokio::sync::oneshot::channel();
                    closed_tx.send(7).unwrap();
                    collect(&mut queue, closed_rx, 7).await
                }
                Mode::DrainedWithoutClose => {
                    let mut queue = Queue::new(success_messages());
                    queue.drained = self.drained.take();
                    let closed = self.pending_close.take().expect("close receiver provided by test");
                    collect(&mut queue, closed, 7).await
                }
                Mode::Ok => {
                    let mut output = ProbeOutput::default();
                    output.append(b"AI_TOOLBOX_POSIX_", false)?;
                    output.append(b"SYNC_V1\n", false)?;
                    output.status(0);
                    output.close();
                    Ok(output)
                }
            }
        }

        async fn disconnect(&mut self) -> Result<(), ()> {
            self.calls.push("disconnect");
            match self.cleanup {
                Mode::Fail => Err(()),
                Mode::Pending => std::future::pending().await,
                _ => Ok(()),
            }
        }
    }

    async fn exercise(fake: &mut Fake) -> Outcome {
        let limits = Deadlines {
            auth: Duration::from_millis(5),
            probe: Duration::from_millis(5),
            cleanup: Duration::from_millis(5),
        };
        tokio::time::timeout(Duration::from_secs(3), run(
            fake, super::super::super::capabilities::POSIX_SYNC_PROBE, limits,
        )).await.expect("the whole lifecycle must remain bounded")
    }

    #[tokio::test]
    async fn authenticated_success_disconnects_once() {
        let mut fake = Fake::default();
        let outcome = exercise(&mut fake).await;
        assert!(outcome.authentication.is_ok());
        assert_eq!(outcome.probe.unwrap().unwrap().capability(), (SSHSyncCapability::Supported, None));
        assert_eq!(outcome.cleanup_warning, None);
        assert_eq!(fake.calls, ["auth", "probe", "disconnect"]);
    }

    #[tokio::test]
    async fn rejected_or_pending_auth_skips_probe_and_disconnects_once() {
        for auth in [Mode::Fail, Mode::Pending] {
            let mut fake = Fake { auth, ..Fake::default() };
            let outcome = exercise(&mut fake).await;
            assert!(outcome.authentication.is_err());
            assert!(outcome.probe.is_none());
            let result = outcome.into_result();
            assert!(!result.connected);
            assert!(result.error.is_some());
            assert_eq!(result.sync_capability, None);
            assert_eq!(result.probe_reason, None);
            assert_eq!(fake.calls, ["auth", "disconnect"]);
        }
    }

    #[tokio::test]
    async fn failed_or_pending_probe_preserves_authentication_and_disconnects() {
        for (probe, reason) in [(Mode::Fail, ProbeReason::ExecFailed), (Mode::Pending, ProbeReason::ProbeTimeout)] {
            let mut fake = Fake { probe, ..Fake::default() };
            let outcome = exercise(&mut fake).await;
            assert!(outcome.authentication.is_ok());
            let result = outcome.into_result();
            assert!(result.connected);
            assert!(result.error.is_none());
            assert_eq!(result.sync_capability, Some(SSHSyncCapability::Unknown));
            assert_eq!(result.probe_reason, Some(reason));
            assert_eq!(fake.calls, ["auth", "probe", "disconnect"]);
        }
    }

    #[tokio::test]
    async fn cleanup_failure_or_timeout_is_separate_from_primary_result() {
        for (cleanup, warning) in [(Mode::Fail, CleanupWarning::DisconnectFailed), (Mode::Pending, CleanupWarning::DisconnectTimeout)] {
            for auth in [Mode::Ok, Mode::Fail] {
                let mut fake = Fake { auth, cleanup, ..Fake::default() };
                let outcome = exercise(&mut fake).await;
                assert_eq!(outcome.authentication.is_ok(), matches!(auth, Mode::Ok));
                assert_eq!(outcome.cleanup_warning, Some(warning));
                assert_eq!(fake.calls.iter().filter(|call| **call == "disconnect").count(), 1);
            }
        }
    }

    #[tokio::test]
    async fn continuous_data_does_not_reset_probe_deadline() {
        let mut fake = Fake { probe: Mode::Streaming, ..Fake::default() };
        let outcome = exercise(&mut fake).await;
        assert!(fake.chunks > 0);
        assert_eq!(outcome.probe.unwrap().unwrap_err(), ProbeReason::ProbeTimeout);
        assert_eq!(fake.calls, ["auth", "probe", "disconnect"]);
    }

    struct Queue {
        messages: std::collections::VecDeque<russh::ChannelMsg>,
        pending_at_end: bool,
        drained: Option<tokio::sync::oneshot::Sender<()>>,
    }

    impl Queue {
        fn new(messages: Vec<russh::ChannelMsg>) -> Self {
            Self { messages: messages.into(), pending_at_end: false, drained: None }
        }
    }

    impl ChannelSource for Queue {
        async fn next(&mut self) -> Option<russh::ChannelMsg> {
            if let Some(message) = self.messages.pop_front() {
                return Some(message);
            }
            if self.pending_at_end {
                return std::future::pending().await;
            }
            if let Some(drained) = self.drained.take() {
                let _ = drained.send(());
            }
            None
        }
    }

    fn data(bytes: &[u8]) -> russh::ChannelMsg {
        russh::ChannelMsg::Data { data: russh::CryptoVec::from_slice(bytes) }
    }

    fn success_messages() -> Vec<russh::ChannelMsg> {
        vec![data(b"AI_TOOLBOX_POSIX_SYNC_V1\n"), russh::ChannelMsg::ExitStatus { exit_status: 0 }]
    }

    #[tokio::test]
    async fn early_close_notification_never_skips_queued_tail_or_status() {
        for (tail, reason) in [
            (data(b"unexpected tail"), ProbeReason::UnexpectedOutput),
            (russh::ChannelMsg::ExitStatus { exit_status: 1 }, ProbeReason::ConflictingExitStatus),
        ] {
            let mut messages = success_messages();
            messages.push(tail);
            let mut queue = Queue::new(messages);
            let (closed_tx, closed_rx) = tokio::sync::oneshot::channel();
            closed_tx.send(7).unwrap();
            let output = collect(&mut queue, closed_rx, 7).await.unwrap();
            assert!(queue.messages.is_empty());
            assert_eq!(output.capability(), (SSHSyncCapability::Unknown, Some(reason)));
        }
    }

    #[tokio::test]
    async fn drained_stream_waits_for_real_close_confirmation() {
        let (closed_tx, closed_rx) = tokio::sync::oneshot::channel();
        let (drained_tx, drained_rx) = tokio::sync::oneshot::channel();
        let mut queue = Queue::new(success_messages());
        queue.drained = Some(drained_tx);
        let (output, ()) = tokio::time::timeout(Duration::from_secs(3), async {
            tokio::join!(
                collect(&mut queue, closed_rx, 7),
                async {
                    drained_rx.await.unwrap();
                    // This notification cannot arrive until the stream yielded None.
                    closed_tx.send(7).unwrap();
                },
            )
        }).await.expect("drain and close confirmation must finish");
        assert_eq!(output.unwrap().capability(), (SSHSyncCapability::Supported, None));
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn drained_stream_without_close_notification_obeys_probe_deadline() {
        let (closed_tx, closed_rx) = tokio::sync::oneshot::channel();
        let (drained_tx, mut drained_rx) = tokio::sync::oneshot::channel();
        let mut fake = Fake {
            probe: Mode::DrainedWithoutClose,
            pending_close: Some(closed_rx),
            drained: Some(drained_tx),
            ..Fake::default()
        };
        let limits = Deadlines::default();
        let started = tokio::time::Instant::now();
        let mut lifecycle = Box::pin(run(
            &mut fake, super::super::super::capabilities::POSIX_SYNC_PROBE, limits,
        ));

        // No task is spawned and no await occurs before this observation. Poll
        // the real lifecycle once: auth succeeds, the queue yields all bytes and
        // status followed by None, then collect must wait for the real callback.
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        assert!(lifecycle.as_mut().poll(&mut context).is_pending());
        drained_rx.try_recv().expect("the stream must have actually yielded None");
        assert!(!closed_tx.is_closed(), "collect must still own the close receiver");
        assert_eq!(started.elapsed(), Duration::ZERO);

        // Keep the sender alive and never send. Paused Tokio time advances to
        // run's existing probe deadline, not a wall-clock sleep or fake timeout.
        // The wider outer bound fails if that production deadline is removed.
        let outcome = tokio::time::timeout(
            limits.probe + limits.cleanup + Duration::from_secs(1), lifecycle,
        ).await.expect("the production probe deadline must bound the callback wait");
        let elapsed = started.elapsed();
        assert!(elapsed >= limits.probe);
        assert!(elapsed < limits.probe + limits.cleanup);
        assert!(closed_tx.is_closed(), "timeout must drop the waiting receiver");
        drop(closed_tx);

        assert_eq!(outcome.cleanup_warning, None);
        let result = outcome.into_result();
        assert!(result.connected);
        assert!(result.error.is_none());
        assert_eq!(result.sync_capability, Some(SSHSyncCapability::Unknown));
        assert_eq!(result.probe_reason, Some(ProbeReason::ProbeTimeout));
        assert_eq!(fake.calls, ["auth", "probe", "disconnect"]);
    }

    #[tokio::test]
    async fn canceled_or_wrong_channel_close_cannot_confirm_probe() {
        for wrong_channel in [false, true] {
            let (closed_tx, closed_rx) = tokio::sync::oneshot::channel();
            if wrong_channel { closed_tx.send(8).unwrap(); } else { drop(closed_tx); }
            let mut queue = Queue::new(success_messages());
            let probe = collect(&mut queue, closed_rx, 7).await;
            assert_eq!(probe.as_ref().unwrap_err(), &ProbeReason::ChannelClosedEarly);
            let result = Outcome { authentication: Ok(()), probe: Some(probe), cleanup_warning: None }.into_result();
            assert!(result.connected);
            assert!(result.error.is_none());
            assert_eq!(result.sync_capability, Some(SSHSyncCapability::Unknown));
        }
    }

    #[tokio::test]
    async fn token_status_and_close_signal_without_stream_end_still_time_out() {
        let mut fake = Fake { probe: Mode::UnclosedStream, ..Fake::default() };
        let result = exercise(&mut fake).await.into_result();
        assert!(result.connected);
        assert_eq!(result.sync_capability, Some(SSHSyncCapability::Unknown));
        assert_eq!(result.probe_reason, Some(ProbeReason::ProbeTimeout));
        assert_eq!(fake.calls, ["auth", "probe", "disconnect"]);
    }

    fn completed(stdout: &[u8]) -> ProbeOutput {
        let mut output = ProbeOutput::default();
        output.append(stdout, false).unwrap();
        output.status(0);
        output.close();
        output
    }

    #[test]
    fn combined_raw_limit_is_checked_before_extending() {
        let mut output = ProbeOutput::default();
        output.append(&vec![0xff; OUTPUT_LIMIT - 1], true).unwrap();
        output.append(b"x", false).unwrap();
        assert_eq!(output.append(b"y", false), Err(ProbeReason::OutputLimitExceeded));
        assert_eq!(output.stdout, b"x");
        assert_eq!(output.stderr.len(), OUTPUT_LIMIT - 1);
        let mut oversized = ProbeOutput::default();
        assert_eq!(oversized.append(&vec![0; OUTPUT_LIMIT + 1], true), Err(ProbeReason::OutputLimitExceeded));
        assert!(oversized.stderr.is_empty());
    }

    #[test]
    fn only_exact_raw_tokens_with_zero_status_and_close_are_classified() {
        for suffix in ["", "\n", "\r\n"] {
            for (token, capability) in [("AI_TOOLBOX_POSIX_SYNC_V1", SSHSyncCapability::Supported), ("AI_TOOLBOX_POSIX_SYNC_NO_HOME", SSHSyncCapability::MissingHome)] {
                assert_eq!(completed(format!("{token}{suffix}").as_bytes()).capability(), (capability, None));
            }
        }
        for raw in [b"AI_TOOLBOX_POSIX_SYNC_V1 extra".as_slice(), b"banner\nAI_TOOLBOX_POSIX_SYNC_V1\n", b"AI_TOOLBOX_POSIX_SYNC_V1\n\n", b" AI_TOOLBOX_POSIX_SYNC_V1", b"AI_TOOLBOX_POSIX_SYNC_V1\xff", b""] {
            assert_eq!(completed(raw).capability(), (SSHSyncCapability::Unknown, Some(ProbeReason::UnexpectedOutput)));
        }
        let mut output = completed(b"");
        output.append(b"AI_TOOLBOX_POSIX_SYNC_V1\n", true).unwrap();
        assert_eq!(output.capability().1, Some(ProbeReason::UnexpectedOutput));
        let mut output = completed(b"AI_TOOLBOX_POSIX_SYNC_V1\n");
        output.append(b"\xff\xfe\x00", true).unwrap();
        assert_eq!(output.capability(), (SSHSyncCapability::Supported, None));
    }

    #[test]
    fn token_never_overrides_close_exit_signal_or_status_failures() {
        let mut output = ProbeOutput::default();
        output.append(b"AI_TOOLBOX_POSIX_SYNC_V1\n", false).unwrap();
        assert_eq!(output.capability().1, Some(ProbeReason::ChannelClosedEarly));
        output.close();
        assert_eq!(output.capability().1, Some(ProbeReason::MissingExitStatus));
        output.status(1);
        assert_eq!(output.capability().1, Some(ProbeReason::NonzeroExitStatus));
        output.status(0);
        assert_eq!(output.capability().1, Some(ProbeReason::ConflictingExitStatus));
        output.signal();
        assert_eq!(output.capability().1, Some(ProbeReason::ExitSignal));
        let mut same_status = completed(b"AI_TOOLBOX_POSIX_SYNC_V1\n");
        same_status.status(0);
        assert_eq!(same_status.capability().0, SSHSyncCapability::Supported);
    }
}
