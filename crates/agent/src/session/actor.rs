//! Per-session actor task (mirrors pi-rs' ACP `SessionTask` design).
//!
//! Each session is owned by a dedicated actor running on its own thread +
//! [`tokio::task::LocalSet`]. The actor owns the command mailbox and the
//! session itself (via `Arc`); LLM prompt turns run as *separate* tasks on
//! the same local executor, so the actor stays responsive to every command
//! while a turn is in flight — `Cancel` works without deadlocking on a shared
//! lock (pi-rs `prompt()` is `!Send`: it holds a `std::sync::MutexGuard`
//! across `.await`, which is why we need `spawn_local` on a dedicated thread
//! instead of `tokio::spawn`).
//!
//! The command mailbox is *unbounded* (matching pi-acp, which uses
//! `mpsc::unbounded_channel`): commands are never rejected for "busy" — only
//! prompts queue (inside the actor, in the turn queue), everything else is
//! handled immediately. Backpressure lives at the event layer instead (each
//! subscriber channel is bounded; a slow consumer stalls the turn's
//! completion, never the command loop).
//!
//! Turn queue semantics match pi-acp: prompts arriving mid-turn are queued
//! and started in order after the current turn settles; a *failed* turn
//! rejects the remaining queue (pi may be unhealthy — we don't auto-proceed).
//! Event backpressure also matches ACP: the turn subscribes an unbounded
//! event channel, serializes each event with a `seq` + ring entry, forwards
//! it to subscribers with an ack, and only replies to the prompt once all
//! forwarded events have been delivered (no event is lost before the turn
//! result is reported).

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use futures::future::LocalBoxFuture;
use pi_coding_agent::core::agent_session::AgentSessionEvent;
use pi_coding_agent::core::agent_session::{AgentSession, PromptOptions};
use tokio::sync::{mpsc, oneshot, Mutex as TokioMutex};

use super::factory::CreatedSession;
use super::slash::handle_slash_command;
use crate::events::{
    convert_session_event, SessionEventListener, SessionEventState, TranscriptEvent,
};

/// Bounded capacity of each subscriber's event channel (matches the previous
/// per-connection channel size).
pub const SUBSCRIBER_CHANNEL_CAPACITY: usize = 256;

/// Actor thread stack size (2 MiB is plenty for the pi agent loop).
const ACTOR_STACK_SIZE: usize = 2 * 1024 * 1024;

/// A prompt queued while another prompt was running (pi-acp turn queue).
struct QueuedPrompt {
    text: String,
    opts: PromptOptions,
    reply: oneshot::Sender<Result<(), String>>,
}

/// A command sent to a session actor.
#[allow(clippy::large_enum_variant)]
pub enum SessionMsg {
    /// Run a full prompt turn. The reply resolves when the turn has fully
    /// settled (all events delivered to subscribers), matching pi-acp's
    /// prompt-until-agent_settled semantics.
    Prompt {
        text: String,
        opts: PromptOptions,
        reply: oneshot::Sender<Result<(), String>>,
    },
    /// Cancel the in-flight turn and clear the queued prompts (pi-acp
    /// `session/cancel`). Never blocks — unbounded mailbox, immediate send.
    Cancel,
    /// Run a session-level slash command (model / name / compact / …).
    Slash {
        cmd: String,
        args: String,
        reply: oneshot::Sender<Result<String, String>>,
    },
    /// Force-write the session JSONL to disk; returns (session_file, name).
    FlushToDisk { reply: oneshot::Sender<FlushResult> },
    /// Subscribe an event channel. The sender is retained by the actor's
    /// fan-out task and receives every serialized event envelope (with `seq`).
    Subscribe {
        tx: mpsc::Sender<String>,
        reply: oneshot::Sender<Result<(), String>>,
    },
    /// Snapshot query (streaming / pending prompt count).
    Status {
        reply: oneshot::Sender<SessionStatus>,
    },
    /// Last assistant text (used by one-shot `prompt_once` flows).
    LastAssistantText {
        reply: oneshot::Sender<Option<String>>,
    },
    /// Stop the in-flight turn and exit the actor loop.
    Shutdown,
}

/// Result of [`SessionMsg::FlushToDisk`]: (session_file, session_name).
type FlushResult = Result<(Option<PathBuf>, Option<String>), String>;

/// Snapshot of actor state for REST queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionStatus {
    pub streaming: bool,
    pub pending_prompts: usize,
}

/// Cloneable handle to a running session actor.
#[derive(Clone)]
pub struct SessionHandle {
    cmd_tx: mpsc::UnboundedSender<SessionMsg>,
    cwd: String,
    /// Lock-free streaming flag for high-frequency REST queries
    /// (`GET /sessions` lists "streaming" status without a round-trip).
    streaming: Arc<AtomicBool>,
}

impl SessionHandle {
    pub fn cwd(&self) -> &str {
        &self.cwd
    }

    /// Run a full prompt turn; resolves when the turn has fully settled.
    pub async fn prompt(&self, text: &str, opts: PromptOptions) -> Result<(), String> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.cmd_tx
            .send(SessionMsg::Prompt {
                text: text.to_string(),
                opts,
                reply: reply_tx,
            })
            .map_err(|_| "session actor closed".to_string())?;
        reply_rx
            .await
            .map_err(|_| "session actor dropped reply".to_string())?
    }

    /// Signal the in-flight turn to abort and clear queued prompts.
    ///
    /// Returns `false` if the actor is already gone. The mailbox is
    /// unbounded (matching pi-acp), so an interrupt is never blocked.
    pub async fn cancel(&self) -> bool {
        self.cmd_tx.send(SessionMsg::Cancel).is_ok()
    }

    /// Run a session-level slash command; returns the rendered notice text.
    pub async fn slash(&self, cmd: &str, args: &str) -> Result<String, String> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.cmd_tx
            .send(SessionMsg::Slash {
                cmd: cmd.to_string(),
                args: args.to_string(),
                reply: reply_tx,
            })
            .map_err(|_| "session actor closed".to_string())?;
        reply_rx
            .await
            .map_err(|_| "session actor dropped reply".to_string())?
    }

    /// Force-write the session JSONL to disk.
    pub async fn flush_to_disk(&self) -> Result<(Option<PathBuf>, Option<String>), String> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.cmd_tx
            .send(SessionMsg::FlushToDisk { reply: reply_tx })
            .map_err(|_| "session actor closed".to_string())?;
        reply_rx
            .await
            .map_err(|_| "session actor dropped reply".to_string())?
    }

    /// Subscribe to the session's event stream.
    ///
    /// Returns `(tx, rx)`: the actor's fan-out task holds a clone of `tx` and
    /// pushes every serialized envelope; the caller keeps `rx`. The returned
    /// `tx` can be cloned further for sending local notices (e.g. slash
    /// results) into the same per-connection stream (mpsc is multi-producer).
    pub async fn subscribe(&self) -> (mpsc::Sender<String>, mpsc::Receiver<String>) {
        let (tx, rx) = mpsc::channel(SUBSCRIBER_CHANNEL_CAPACITY);
        let (reply_tx, _reply_rx) = oneshot::channel();
        let _ = self.cmd_tx.send(SessionMsg::Subscribe {
            tx: tx.clone(),
            reply: reply_tx,
        });
        (tx, rx)
    }

    /// Snapshot of actor state (`None` if the actor is gone).
    pub async fn status(&self) -> Option<SessionStatus> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.cmd_tx
            .send(SessionMsg::Status { reply: reply_tx })
            .ok()?;
        reply_rx.await.ok()
    }

    /// Last assistant text for one-shot flows.
    pub async fn last_assistant_text(&self) -> Option<String> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.cmd_tx
            .send(SessionMsg::LastAssistantText { reply: reply_tx })
            .ok()?;
        reply_rx.await.ok().flatten()
    }

    /// Ask the actor to stop and exit its thread.
    pub async fn shutdown(&self) {
        let _ = self.cmd_tx.send(SessionMsg::Shutdown);
    }

    /// Lock-free "is a turn streaming right now" (REST hot path).
    pub fn is_streaming(&self) -> bool {
        self.streaming.load(Ordering::SeqCst)
    }
}

/// Spawn a session actor on a dedicated thread with its own `LocalSet`.
///
/// The dedicated thread hosts the actor loop and every turn task via
/// `spawn_local`, so pi-rs `prompt()` (a `!Send` future) runs without `Send`
/// bounds and without `spawn_blocking` + `block_on` wrappers. The thread
/// exits when the actor processes `Shutdown` (pool close / LRU eviction).
pub fn spawn_session_actor(
    session_id: String,
    created: CreatedSession,
    event_state: Arc<SessionEventState>,
) -> SessionHandle {
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<SessionMsg>();
    let (notif_tx, notif_rx) = mpsc::unbounded_channel::<(String, oneshot::Sender<()>)>();
    let subscribers: Arc<TokioMutex<HashMap<u64, mpsc::Sender<String>>>> = Default::default();
    let cancelled = Arc::new(AtomicBool::new(false));
    let streaming = Arc::new(AtomicBool::new(false));

    let handle = SessionHandle {
        cmd_tx: cmd_tx.clone(),
        cwd: created.cwd.clone(),
        streaming: streaming.clone(),
    };

    let thread_name = format!("session-actor-{session_id}");
    let sid_for_err = session_id.clone();
    // The caller is inside a tokio runtime (pool methods are async), so we can
    // capture the handle here and drive the actor's LocalSet from its own
    // thread via `Handle::block_on`.
    let rt = match tokio::runtime::Handle::try_current() {
        Ok(rt) => rt,
        Err(_) => {
            eprintln!("session actor {session_id}: no tokio runtime context");
            // Return a dead handle: later `send` calls fail with
            // "session actor closed", which callers surface as errors.
            return handle;
        }
    };
    if let Err(e) = std::thread::Builder::new()
        .name(thread_name)
        .stack_size(ACTOR_STACK_SIZE)
        .spawn(move || {
            // Drive a LocalSet from this thread via `Handle::block_on`
            // (LocalSet::block_on needs a `&Runtime`, which a spawned thread
            // doesn't own). `run_until` returns a `!Send` future, which
            // `Handle::block_on` accepts — this is how we host `!Send`
            // pi-rs `prompt()` futures without `spawn_blocking` wrappers.
            rt.block_on(async move {
                let local = tokio::task::LocalSet::new();
                local
                    .run_until(async move {
                        // Local-future spawner: every spawned future (fan-out, turns,
                        // abort tasks) runs on this thread's LocalSet, so `!Send`
                        // futures are fine. JoinHandles are observed so a panicked
                        // task is never silently swallowed (project rule).
                        let spawn: Arc<dyn Fn(LocalBoxFuture<'static, ()>)> = Arc::new(|fut| {
                            let handle = tokio::task::spawn_local(fut);
                            tokio::task::spawn_local(async move {
                                if let Err(e) = handle.await {
                                    eprintln!("session actor task panicked: {e}");
                                }
                            });
                        });

                        let fanout = fanout_loop(notif_rx, Arc::clone(&subscribers));
                        tokio::task::spawn_local(fanout);

                        let actor = SessionActor {
                            session: created.session,
                            _services: created.services,
                            cmd_rx,
                            event_state,
                            subscribers,
                            notif_tx,
                            prompt_queue: VecDeque::new(),
                            turn_done_rx: None,
                            cancelled,
                            streaming,
                            next_sub_id: 0,
                            spawn,
                        };
                        actor.run().await;
                    })
                    .await;
            });
        })
    {
        eprintln!("session actor {sid_for_err}: failed to spawn thread: {e}");
    }

    handle
}

/// Which event the actor loop should react to next.
enum NextEvent {
    Command(Option<SessionMsg>),
    TurnDone(Result<(), String>),
}

/// The per-session actor: owns the session and the command mailbox.
struct SessionActor {
    session: Arc<AgentSession>,
    /// Kept alive for the session's lifetime (cwd-bound services /
    /// extension registry the session depends on).
    _services: pi_coding_agent::core::agent_session_services::AgentSessionServices,
    cmd_rx: mpsc::UnboundedReceiver<SessionMsg>,
    event_state: Arc<SessionEventState>,
    subscribers: Arc<TokioMutex<HashMap<u64, mpsc::Sender<String>>>>,
    notif_tx: mpsc::UnboundedSender<(String, oneshot::Sender<()>)>,
    prompt_queue: VecDeque<QueuedPrompt>,
    turn_done_rx: Option<mpsc::Receiver<Result<(), String>>>,
    cancelled: Arc<AtomicBool>,
    streaming: Arc<AtomicBool>,
    next_sub_id: u64,
    spawn: Arc<dyn Fn(LocalBoxFuture<'static, ()>)>,
}

impl SessionActor {
    /// The actor loop (mirrors pi-rs ACP `SessionTask::run`): process
    /// commands, and when the in-flight turn settles, start the next queued
    /// prompt. Commands are never rejected for "busy" — only prompts queue.
    async fn run(mut self) {
        loop {
            match self.next_event().await {
                NextEvent::Command(Some(SessionMsg::Prompt { text, opts, reply })) => {
                    self.on_prompt(text, opts, reply).await;
                }
                NextEvent::Command(Some(SessionMsg::Cancel)) => {
                    self.on_cancel().await;
                }
                NextEvent::Command(Some(SessionMsg::Slash { cmd, args, reply })) => {
                    let result = handle_slash_command(&cmd, &args, &self.session).await;
                    let _ = reply.send(result);
                }
                NextEvent::Command(Some(SessionMsg::FlushToDisk { reply })) => {
                    let _ = reply.send(self.flush_to_disk());
                }
                NextEvent::Command(Some(SessionMsg::Subscribe { tx, reply })) => {
                    let _ = reply.send(self.subscribe(tx).await);
                }
                NextEvent::Command(Some(SessionMsg::Status { reply })) => {
                    let _ = reply.send(self.status());
                }
                NextEvent::Command(Some(SessionMsg::LastAssistantText { reply })) => {
                    let text = self.session.get_last_assistant_text().await;
                    let _ = reply.send(text);
                }
                NextEvent::Command(Some(SessionMsg::Shutdown)) | NextEvent::Command(None) => {
                    // Reject any queued prompts, stop the in-flight turn, and
                    // let the abort settle before the actor thread exits so it
                    // doesn't keep running against a torn-down session.
                    while let Some(queued) = self.prompt_queue.pop_front() {
                        let _ = queued.reply.send(Err("session closed".to_string()));
                    }
                    if self.turn_done_rx.is_some() {
                        self.cancelled.store(true, Ordering::SeqCst);
                        let session = Arc::clone(&self.session);
                        // `abort()` waits for the agent to become idle; bound
                        // it so a wedged agent can't hang shutdown forever.
                        let abort = async move { session.abort().await };
                        tokio::pin!(abort);
                        tokio::select! {
                            _ = &mut abort => {}
                            _ = tokio::time::sleep(std::time::Duration::from_secs(10)) => {
                                eprintln!("session actor: abort timed out on shutdown");
                            }
                        }
                    }
                    break;
                }
                NextEvent::TurnDone(outcome) => {
                    // The turn settled. Start the next queued prompt, or (on
                    // failure) reject the rest of the queue — pi may be
                    // unhealthy, so we don't auto-proceed after a failure
                    // (mirrors pi-acp).
                    self.turn_done_rx = None;
                    if outcome.is_err() {
                        while let Some(queued) = self.prompt_queue.pop_front() {
                            let _ = queued
                                .reply
                                .send(Err("session is busy processing a prompt".to_string()));
                        }
                    } else if let Some(queued) = self.prompt_queue.pop_front() {
                        self.start_prompt_turn(queued);
                    }
                }
            }
        }
    }

    /// Wait for the next event: a command from the mailbox, or (while a turn
    /// is in flight) the turn settling (mirrors pi-rs ACP `next_event`).
    async fn next_event(&mut self) -> NextEvent {
        match &mut self.turn_done_rx {
            Some(rx) => tokio::select! {
                cmd = self.cmd_rx.recv() => NextEvent::Command(cmd),
                done = rx.recv() => match done {
                    Some(outcome) => NextEvent::TurnDone(outcome),
                    // The turn task died without reporting — treat as a
                    // failed turn so the queue is rejected rather than stuck.
                    None => NextEvent::TurnDone(Err(
                        "turn task ended unexpectedly".to_string()
                    )),
                },
            },
            None => NextEvent::Command(self.cmd_rx.recv().await),
        }
    }

    /// Handle a prompt command: if a turn is already running, queue it
    /// (pi-acp turn queue); otherwise start it immediately.
    async fn on_prompt(
        &mut self,
        text: String,
        opts: PromptOptions,
        reply: oneshot::Sender<Result<(), String>>,
    ) {
        if self.turn_done_rx.is_some() {
            self.prompt_queue
                .push_back(QueuedPrompt { text, opts, reply });
            return;
        }
        self.start_prompt_turn(QueuedPrompt { text, opts, reply });
    }

    /// Spawn the LLM turn as a task on the actor's executor. The task streams
    /// events to subscribers, replies with the turn outcome, and reports the
    /// outcome on a fresh `turn_done_rx` channel so the actor can start the
    /// next queued prompt (mirrors pi-rs ACP `start_prompt_turn`).
    fn start_prompt_turn(&mut self, prompt: QueuedPrompt) {
        let (turn_done_tx, turn_done_rx) = mpsc::channel::<Result<(), String>>(1);
        self.turn_done_rx = Some(turn_done_rx);
        // A cancel flag from a previous turn must not leak into this one.
        self.cancelled.store(false, Ordering::SeqCst);
        self.streaming.store(true, Ordering::SeqCst);
        let fut = run_prompt_turn(
            Arc::clone(&self.session),
            Arc::clone(&self.cancelled),
            Arc::clone(&self.streaming),
            self.notif_tx.clone(),
            Arc::clone(&self.event_state),
            turn_done_tx,
            prompt,
        );
        (self.spawn)(Box::pin(fut));
    }

    /// Cancel: stop the current turn without blocking the command loop,
    /// resolve queued prompts as cancelled (mirrors pi-acp `on_cancel`).
    async fn on_cancel(&mut self) {
        if self.turn_done_rx.is_none() {
            // Nothing is running — no-op.
            return;
        }
        self.cancelled.store(true, Ordering::SeqCst);
        // `abort()` waits for the agent to become idle, so it runs as a
        // separate task; the turn task observes the abort, settles, and
        // replies. The actor loop stays responsive meanwhile.
        let session = Arc::clone(&self.session);
        let fut: LocalBoxFuture<'static, ()> = Box::pin(async move {
            session.abort().await;
        });
        (self.spawn)(fut);
        // Cancel clears the queued prompts (mirrors pi-acp: cancel resolves
        // queued turns as cancelled).
        while let Some(queued) = self.prompt_queue.pop_front() {
            let _ = queued.reply.send(Ok(()));
        }
    }

    /// Subscribe an event channel to the fan-out task's subscriber set.
    async fn subscribe(&mut self, tx: mpsc::Sender<String>) -> Result<(), String> {
        let id = self.next_sub_id;
        self.next_sub_id += 1;
        self.subscribers.lock().await.insert(id, tx);
        Ok(())
    }

    fn status(&self) -> SessionStatus {
        SessionStatus {
            streaming: self.turn_done_rx.is_some(),
            pending_prompts: self.prompt_queue.len(),
        }
    }

    /// Force-write the session JSONL to disk so list-sessions can find it.
    fn flush_to_disk(&self) -> Result<(Option<PathBuf>, Option<String>), String> {
        let session = &self.session;
        let mgr = session.get_session_manager();
        let sfile = mgr.get_session_file().map(Path::to_path_buf);
        let sname = mgr.get_session_name();
        drop(mgr);

        if let Some(ref path) = sfile {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let mut lines: Vec<String> = Vec::new();
            if let Some(hdr) = session.get_session_manager().get_header() {
                if let Ok(json) = serde_json::to_string(hdr) {
                    lines.push(json);
                }
            }
            for entry in session.get_session_manager().get_entries() {
                if let Ok(json) = serde_json::to_string(entry) {
                    lines.push(json);
                }
            }
            if !lines.is_empty() {
                std::fs::write(path, lines.join("\n") + "\n").map_err(|e| e.to_string())?;
            }
        }
        Ok((sfile, sname))
    }
}

/// Forward serialized event envelopes from turn tasks to all subscribers.
///
/// Each event is only acked after every live subscriber channel accepted it
/// (ACP semantics: a slow consumer backpressures the turn's completion; a
/// dead consumer — channel closed — is removed). Runs as its own task on the
/// actor thread, so a stalled fan-out never blocks the actor command loop.
async fn fanout_loop(
    mut notif_rx: mpsc::UnboundedReceiver<(String, oneshot::Sender<()>)>,
    subscribers: Arc<TokioMutex<HashMap<u64, mpsc::Sender<String>>>>,
) {
    while let Some((json, ack)) = notif_rx.recv().await {
        // Collect targets under the lock quickly; never hold the lock across
        // `.await` (a stalled subscriber would otherwise block the actor).
        let targets: Vec<(u64, mpsc::Sender<String>)> = {
            let subs = subscribers.lock().await;
            subs.iter().map(|(id, tx)| (*id, tx.clone())).collect()
        };
        let mut dead = Vec::new();
        for (id, tx) in targets {
            if tx.send(json.clone()).await.is_err() {
                dead.push(id);
            }
        }
        if !dead.is_empty() {
            let mut subs = subscribers.lock().await;
            for id in dead {
                subs.remove(&id);
            }
        }
        let _ = ack.send(());
    }
}

/// Run one prompt turn: stream events to subscribers with acks, then report
/// the outcome (mirrors pi-rs ACP `run_prompt_turn`).
async fn run_prompt_turn(
    session: Arc<AgentSession>,
    cancelled: Arc<AtomicBool>,
    streaming: Arc<AtomicBool>,
    notif_tx: mpsc::UnboundedSender<(String, oneshot::Sender<()>)>,
    event_state: Arc<SessionEventState>,
    turn_done: mpsc::Sender<Result<(), String>>,
    prompt: QueuedPrompt,
) {
    let QueuedPrompt { text, opts, reply } = prompt;
    let outcome =
        run_single_prompt(&session, &cancelled, &notif_tx, &event_state, &text, opts).await;
    streaming.store(false, Ordering::SeqCst);
    let _ = reply.send(outcome.clone());
    let _ = turn_done.send(outcome).await;
}

/// Run one prompt turn: subscribe events, drive `prompt()`, serialize every
/// event with a `seq` + ring entry, forward to subscribers with acks, and
/// only resolve once all forwarded events were delivered (ACP flush-acks
/// semantics). `cancelled` is set by the actor on `Cancel`; the turn task
/// simply settles when the agent aborts.
async fn run_single_prompt(
    session: &AgentSession,
    _cancelled: &AtomicBool,
    notif_tx: &mpsc::UnboundedSender<(String, oneshot::Sender<()>)>,
    event_state: &Arc<SessionEventState>,
    text: &str,
    opts: PromptOptions,
) -> Result<(), String> {
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<AgentSessionEvent>();
    let listener: SessionEventListener = Arc::new(move |event| {
        let _ = event_tx.send(event);
    });
    let handle = session.subscribe(listener);

    let prompt_fut = session.prompt(text, Some(opts));
    tokio::pin!(prompt_fut);

    // Acks of notifications forwarded while the turn ran. We don't await them
    // inline (that would delay reacting to the prompt future); they are
    // flushed when the prompt future completes (pi-acp flushEmits).
    let mut pending_acks: Vec<oneshot::Receiver<()>> = Vec::new();

    let outcome: Result<(), String> = loop {
        tokio::select! {
            result = &mut prompt_fut => {
                // Drain any events still queued (e.g. AgentEnd) so the
                // client receives the terminal event before the reply.
                let mut disconnected = false;
                while let Ok(event) = event_rx.try_recv() {
                    let payload = convert_session_event(event);
                    let json = event_state.emit(TranscriptEvent::Session(payload));
                    let (ack_tx, ack_rx) = oneshot::channel();
                    if notif_tx.send((json, ack_tx)).is_err() {
                        disconnected = true;
                        break;
                    }
                    pending_acks.push(ack_rx);
                }
                // Flush: wait for all forwarded notifications to be delivered
                // before replying to the prompt.
                for ack in pending_acks.drain(..) {
                    if ack.await.is_err() {
                        disconnected = true;
                        break;
                    }
                }
                if disconnected {
                    break Err("client disconnected".to_string());
                }
                break result;
            }
            Some(event) = event_rx.recv() => {
                let payload = convert_session_event(event);
                let json = event_state.emit(TranscriptEvent::Session(payload));
                let (ack_tx, ack_rx) = oneshot::channel();
                if notif_tx.send((json, ack_tx)).is_err() {
                    break Err("client disconnected".to_string());
                }
                pending_acks.push(ack_rx);
            }
        }
    };

    handle.unsubscribe();
    outcome
}
