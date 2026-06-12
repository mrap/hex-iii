// Copyright Motia LLC and/or licensed to Motia LLC under one or more
// contributor license agreements. Licensed under the Elastic License 2.0;
// you may not use this file except in compliance with the Elastic License 2.0.
// This software is patent protected. We welcome discussions - reach out at team@iii.dev
// See LICENSE and PATENTS files for details.

use std::{
    path::Path,
    process::Stdio,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use anyhow::Result;
use colored::Colorize;
use notify::{
    Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher,
    event::{CreateKind, DataChange, ModifyKind, RemoveKind, RenameMode},
};
use tokio::{
    process::{Child, Command},
    sync::{Mutex, mpsc},
    time::{Duration, timeout},
};

use crate::workers::shell::{
    config::{ExecConfig, HealthCheck, RestartPolicy},
    glob_exec::GlobExec,
};

#[derive(Debug, Clone)]
pub struct Exec {
    exec: Vec<String>,
    glob_exec: Option<GlobExec>,
    child: Arc<Mutex<Option<Child>>>,
    /// Crash-respawn policy for the long-lived (last) command. None = legacy
    /// spawn-once, never respawned.
    restart: Option<RestartPolicy>,
    /// Liveness probe for the long-lived command. None = never probed.
    health: Option<HealthCheck>,
    shutdown_tx: Arc<tokio::sync::watch::Sender<bool>>,
    shutdown_rx: tokio::sync::watch::Receiver<bool>,
    shutdown_called: Arc<AtomicBool>,
}

const MAX_WATCH_EVENTS: usize = 100;

impl Exec {
    pub fn new(config: ExecConfig) -> Self {
        tracing::info!("Creating Exec module with config: {:?}", config);

        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        Self {
            glob_exec: config.watch.map(GlobExec::new),
            exec: config.exec,
            child: Arc::new(Mutex::new(None::<Child>)),
            restart: config.restart,
            health: config.health,
            shutdown_tx: Arc::new(shutdown_tx),
            shutdown_rx,
            shutdown_called: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Signal all loops in run()/run_pipeline() to stop, then kill the child process.
    pub async fn shutdown(&self) {
        if self.shutdown_called.swap(true, Ordering::SeqCst) {
            return; // Already called
        }
        tracing::info!("ExecModule received shutdown signal, stopping process");
        let _ = self.shutdown_tx.send(true);
        self.stop_process().await;
    }

    pub async fn run(self) -> anyhow::Result<()> {
        let (tx, mut rx) = mpsc::channel::<Event>(MAX_WATCH_EVENTS);
        let mut watcher: RecommendedWatcher;

        if let Some(ref glob_exec) = self.glob_exec {
            tracing::info!("Creating watcher for glob exec: {:?}", glob_exec);

            watcher = Watcher::new(
                move |res| {
                    if let Ok(event) = res {
                        let _ = tx.blocking_send(event);
                    }
                },
                Config::default(),
            )?;

            for root in glob_exec.watch_roots() {
                watcher.watch(
                    Path::new(&root.path),
                    if root.recursive {
                        RecursiveMode::Recursive
                    } else {
                        RecursiveMode::NonRecursive
                    },
                )?;
            }
        }

        let cwd = std::env::current_dir().unwrap_or_default();

        // 🔥 start pipeline
        self.run_pipeline().await?;

        let mut shutdown_rx = self.shutdown_rx.clone();

        // Supervised daemon: await the long-lived (last) command's exit and
        // probe its health alongside the file-watcher. Without restart/health
        // this block is skipped and the legacy watch/shutdown loop runs as-is.
        if self.restart.is_some() || self.health.is_some() {
            self.supervise(rx, shutdown_rx, &cwd).await;
            return Ok(());
        }

        loop {
            tokio::select! {
                event = rx.recv() => {
                    match event {
                        Some(event) if self.should_restart(&event) => {
                            tracing::info!(
                                "File change detected {} → restarting pipeline",
                                event
                                    .paths
                                    .iter()
                                    .map(|p| {
                                        p.strip_prefix(&cwd)
                                            .map(|s| s.to_string_lossy().to_string())
                                            .unwrap_or_else(|_| p.to_string_lossy().to_string())
                                    })
                                    .collect::<Vec<_>>()
                                    .join(", ")
                                    .purple()
                            );

                            self.kill_process().await;
                            self.run_pipeline().await?;
                        }
                        Some(_) => continue,
                        None => break,
                    }
                }
                _ = shutdown_rx.changed() => {
                    tracing::info!("ExecModule file watcher shutting down");
                    break;
                }
            }
        }

        Ok(())
    }

    /// Supervise the long-lived (last) command: respawn it on exit (per
    /// `restart`) and probe its liveness (per `health`), while still honoring
    /// file-watch restarts and shutdown. Entered from `run()` only when
    /// `restart` or `health` is configured.
    async fn supervise(
        &self,
        mut rx: mpsc::Receiver<Event>,
        mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
        cwd: &Path,
    ) {
        // The daemon is the last pipeline command (already spawned by
        // run_pipeline, sitting in self.child). Respawns re-run ONLY this
        // command — earlier pipeline steps (e.g. a `pkill` cleanup) ran once at
        // startup and are deliberately not repeated on each crash.
        let daemon_cmd = match self.exec.last() {
            Some(cmd) => cmd.clone(),
            None => {
                // Nothing long-lived to supervise; just wait for shutdown.
                let _ = shutdown_rx.changed().await;
                self.stop_process().await;
                return;
            }
        };

        // Hold the daemon outside the mutex so we can await its exit. It is put
        // back into self.child before any stop/kill so the existing
        // stop_process/kill_process paths reap it (mirrors run_pipeline's
        // shutdown handling).
        let mut daemon: Option<Child> = self.child.lock().await.take();

        let (init_backoff, max_backoff) = self
            .restart
            .as_ref()
            .map(|r| (r.backoff_secs, r.max_backoff_secs))
            .unwrap_or((0, 0));
        let mut backoff = init_backoff;

        let mut health_timer = self.health.as_ref().map(|h| {
            let mut iv = tokio::time::interval(Duration::from_secs(h.interval_secs.max(1)));
            iv.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            iv
        });
        // The first interval tick fires immediately; consume it so the daemon
        // gets one interval to come up before the first probe.
        if let Some(iv) = health_timer.as_mut() {
            iv.tick().await;
        }
        let mut health_failures: u32 = 0;

        loop {
            tokio::select! {
                // 1. The daemon exited on its own.
                status = wait_opt(&mut daemon) => {
                    let on_crash = self.restart.as_ref().is_some_and(|r| r.on_crash);
                    if !on_crash {
                        tracing::warn!(
                            "exec daemon '{}' exited ({:?}); no restart.on_crash — left down",
                            daemon_cmd, status
                        );
                        daemon = None; // health/shutdown still served; just no respawn
                        continue;
                    }
                    tracing::error!(
                        "exec daemon '{}' exited ({:?}); respawning after {}s backoff",
                        daemon_cmd, status, backoff
                    );
                    if backoff > 0 {
                        tokio::select! {
                            _ = tokio::time::sleep(Duration::from_secs(backoff)) => {}
                            _ = shutdown_rx.changed() => { self.stop_process().await; return; }
                        }
                    }
                    daemon = self.respawn(&daemon_cmd).await;
                    backoff = next_backoff(backoff, init_backoff, max_backoff);
                    health_failures = 0;
                }
                // 2. Health probe tick.
                _ = tick_opt(&mut health_timer) => {
                    if let Some(h) = &self.health {
                        if probe_health(h).await {
                            health_failures = 0;
                        } else {
                            health_failures += 1;
                            tracing::warn!(
                                "exec daemon '{}' health probe failed ({}/{})",
                                daemon_cmd, health_failures, h.failure_threshold
                            );
                            if health_failures >= h.failure_threshold {
                                tracing::error!(
                                    "exec daemon '{}' failed {} health probes; killing + respawning",
                                    daemon_cmd, h.failure_threshold
                                );
                                if let Some(child) = daemon.take() {
                                    *self.child.lock().await = Some(child);
                                }
                                self.kill_process().await;
                                daemon = self.respawn(&daemon_cmd).await;
                                health_failures = 0;
                            }
                        }
                    }
                }
                // 3. File change (watch) — restart the whole pipeline, as before.
                event = rx.recv(), if self.glob_exec.is_some() => {
                    if let Some(event) = event
                        && self.should_restart(&event)
                    {
                        tracing::info!(
                            "File change detected {} → restarting pipeline",
                            event
                                .paths
                                .iter()
                                .map(|p| {
                                    p.strip_prefix(cwd)
                                        .map(|s| s.to_string_lossy().to_string())
                                        .unwrap_or_else(|_| p.to_string_lossy().to_string())
                                })
                                .collect::<Vec<_>>()
                                .join(", ")
                                .purple()
                        );
                        if let Some(child) = daemon.take() {
                            *self.child.lock().await = Some(child);
                        }
                        self.kill_process().await;
                        if let Err(e) = self.run_pipeline().await {
                            tracing::error!("pipeline restart failed: {e}");
                        }
                        daemon = self.child.lock().await.take();
                        backoff = init_backoff;
                        health_failures = 0;
                    }
                }
                // 4. Shutdown.
                _ = shutdown_rx.changed() => {
                    if let Some(child) = daemon.take() {
                        *self.child.lock().await = Some(child);
                    }
                    self.stop_process().await;
                    return;
                }
            }
        }
    }

    /// Respawn the daemon command. Returns the new child, or None on spawn
    /// failure (loudly logged — supervision keeps running; a configured health
    /// probe will retry it on the next interval).
    async fn respawn(&self, command: &str) -> Option<Child> {
        match self.spawn_single(command) {
            Ok(child) => {
                tracing::info!("exec daemon respawned: {}", command.purple());
                Some(child)
            }
            Err(e) => {
                tracing::error!("failed to respawn exec daemon '{}': {e}", command);
                None
            }
        }
    }

    async fn run_pipeline(&self) -> Result<()> {
        if self.exec.is_empty() {
            return Ok(());
        }

        let mut shutdown_rx = self.shutdown_rx.clone();

        let last_idx = self.exec.len() - 1;
        for (idx, cmd) in self.exec.iter().enumerate() {
            let spawned = self.spawn_single(cmd)?;
            *self.child.lock().await = Some(spawned);

            if idx < last_idx {
                // Take child out of the mutex so we don't hold the lock during wait.
                // This allows stop_process()/shutdown() to proceed if called concurrently.
                let mut child = self.child.lock().await.take().unwrap();

                // Wait for command to finish OR shutdown signal
                let status = tokio::select! {
                    status = child.wait() => status?,
                    _ = shutdown_rx.changed() => {
                        tracing::info!("Pipeline interrupted by shutdown signal");
                        // Put child back and kill it — shutdown() may have already
                        // called stop_process() while the child was extracted, finding
                        // None. We must kill it ourselves to avoid orphaning.
                        *self.child.lock().await = Some(child);
                        self.stop_process().await;
                        return Ok(());
                    }
                };

                if !status.success() {
                    tracing::error!("Pipeline step failed, aborting pipeline");
                    break;
                }
            }
        }

        Ok(())
    }

    fn spawn_single(&self, command: &str) -> Result<Child> {
        tracing::info!("Starting process: {}", command.purple());

        #[cfg(not(windows))]
        let mut cmd = {
            let mut c = Command::new("sh");
            c.arg("-c").arg(command);
            c.stdout(Stdio::inherit()).stderr(Stdio::inherit());

            // We need to detach from the current process
            // To coordinate process termination properly
            unsafe {
                c.pre_exec(|| {
                    nix::unistd::setsid()
                        .map_err(|e| std::io::Error::other(format!("setsid failed: {e}")))?;
                    Ok(())
                });
            }
            c
        };

        #[cfg(windows)]
        let mut cmd = {
            let mut c = Command::new("cmd");
            c.arg("/C").arg(command);
            c.stdout(Stdio::inherit()).stderr(Stdio::inherit());

            // On Windows, create a new process group for easier termination of child process
            c.creation_flags(winapi::um::winbase::CREATE_NEW_PROCESS_GROUP);

            c
        };

        cmd.stdout(Stdio::inherit()).stderr(Stdio::inherit());

        Ok(cmd.spawn()?)
    }

    fn should_restart(&self, event: &Event) -> bool {
        let cwd = std::env::current_dir().unwrap_or_default();
        let is_valid_event = matches!(
            event.kind,
            EventKind::Create(CreateKind::File)
                | EventKind::Modify(ModifyKind::Data(DataChange::Content))
                | EventKind::Modify(ModifyKind::Name(RenameMode::Any))
                | EventKind::Remove(RemoveKind::File)
        );

        if !is_valid_event {
            return false;
        }

        if let Some(ref glob_exec) = self.glob_exec {
            return event
                .paths
                .iter()
                .any(|path| glob_exec.should_trigger(path.strip_prefix(&cwd).unwrap_or(path)));
        }

        false
    }

    pub async fn stop_process(&self) {
        if let Some(mut child) = self.child.lock().await.take() {
            #[cfg(not(windows))]
            let pgid = child.id().map(|id| nix::unistd::Pid::from_raw(id as i32));

            #[cfg(not(windows))]
            if let Some(pgid) = pgid {
                // 1️⃣ Ask the whole process group politely
                let _ = nix::sys::signal::killpg(pgid, nix::sys::signal::Signal::SIGTERM);
            }

            #[cfg(windows)]
            {
                use winapi::{
                    shared::minwindef::{FALSE, TRUE},
                    um::{
                        consoleapi::SetConsoleCtrlHandler,
                        wincon::{
                            AttachConsole, CTRL_BREAK_EVENT, FreeConsole, GenerateConsoleCtrlEvent,
                        },
                    },
                };

                if let Some(pid) = child.id() {
                    unsafe {
                        if AttachConsole(pid) != 0 {
                            SetConsoleCtrlHandler(None, TRUE);
                            GenerateConsoleCtrlEvent(CTRL_BREAK_EVENT, 0);
                            SetConsoleCtrlHandler(None, FALSE);
                            FreeConsole();
                        }
                    }
                }
            }

            // 2️⃣ Wait a bit
            let exited = timeout(Duration::from_secs(3), child.wait()).await;

            if exited.is_err() {
                // 3️⃣ Force kill the entire process group
                tracing::warn!("Process did not exit gracefully, killing");
                #[cfg(not(windows))]
                if let Some(pgid) = pgid {
                    let _ = nix::sys::signal::killpg(pgid, nix::sys::signal::Signal::SIGKILL);
                }
                #[cfg(windows)]
                {
                    let _ = child.kill().await;
                }

                // Reap the child to avoid zombie processes
                let _ = child.wait().await;
            }
        }
    }

    async fn kill_process(&self) {
        if let Some(mut proc) = self.child.lock().await.take() {
            #[cfg(not(windows))]
            {
                if let Some(id) = proc.id() {
                    let pgid = nix::unistd::Pid::from_raw(id as i32);
                    if let Err(err) =
                        nix::sys::signal::killpg(pgid, nix::sys::signal::Signal::SIGKILL)
                    {
                        tracing::error!("Failed to kill process group: {:?}", err);
                    } else {
                        tracing::debug!("Process group killed");
                    }
                }
            }

            #[cfg(windows)]
            {
                if let Err(err) = proc.kill().await {
                    tracing::error!("Failed to kill process: {:?}", err);
                } else {
                    tracing::debug!("Process killed");
                }
            }

            // Reap the direct child to avoid zombies
            let _ = proc.wait().await;
        }
    }
}

/// Await the child's exit when present; otherwise never resolve, so the
/// `select!` arm is effectively disabled while no daemon is live.
async fn wait_opt(daemon: &mut Option<Child>) -> Option<std::process::ExitStatus> {
    match daemon {
        Some(child) => child.wait().await.ok(),
        None => std::future::pending().await,
    }
}

/// Tick the interval when present; otherwise never resolve.
async fn tick_opt(timer: &mut Option<tokio::time::Interval>) {
    match timer {
        Some(iv) => {
            iv.tick().await;
        }
        None => std::future::pending().await,
    }
}

/// Next exponential backoff: double the current delay, clamped to
/// `max_backoff` (0 = no cap). A zero current value falls back to `initial`
/// so a `backoff_secs: 0` config keeps respawning promptly.
fn next_backoff(current: u64, initial: u64, max: u64) -> u64 {
    let base = if current == 0 { initial } else { current };
    let doubled = base.saturating_mul(2);
    if max == 0 { doubled } else { doubled.min(max) }
}

/// Liveness probe: `command` (exit 0 = healthy) takes precedence over `url`
/// (HTTP 2xx = healthy). No probe configured → considered healthy.
async fn probe_health(health: &HealthCheck) -> bool {
    let budget = Duration::from_secs(health.timeout_secs.max(1));
    if let Some(cmd) = &health.command {
        let fut = Command::new("sh").arg("-c").arg(cmd).status();
        return matches!(timeout(budget, fut).await, Ok(Ok(status)) if status.success());
    }
    if let Some(url) = &health.url {
        let fut = reqwest::get(url.as_str());
        return matches!(timeout(budget, fut).await, Ok(Ok(resp)) if resp.status().is_success());
    }
    true
}

#[cfg(test)]
#[cfg(not(windows))]
mod tests {
    use std::{fs, path::PathBuf};

    use super::*;

    fn temp_repo_dir(name: &str) -> PathBuf {
        std::env::current_dir()
            .expect("current dir")
            .join(format!(".tmp-{name}-{}", uuid::Uuid::new_v4()))
    }

    /// Spawns `sh -c "sleep 300 & sleep 300 & wait"` via Exec,
    /// calls stop_process(), and asserts all processes in the group are dead.
    #[tokio::test]
    async fn stop_process_kills_entire_process_group() {
        let exec = Exec::new(ExecConfig {
            watch: None,
            exec: vec!["sleep 300 & sleep 300 & wait".to_string()],
            ..Default::default()
        });

        // Spawn the process (sh -c "sleep 300 & sleep 300 & wait")
        let child = exec.spawn_single(&exec.exec[0]).unwrap();
        let child_pid = child.id().unwrap() as i32;
        *exec.child.lock().await = Some(child);

        // Give children time to spawn
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Collect all PIDs in the process group (PGID = child_pid due to setsid)
        let pids_before: Vec<i32> = get_pids_in_group(child_pid);
        assert!(
            pids_before.len() >= 2,
            "expected at least 2 processes in group, got {:?}",
            pids_before
        );

        // Kill via stop_process
        exec.stop_process().await;

        // Give OS time to reap
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Verify all processes in the group are dead
        let pids_after: Vec<i32> = get_pids_in_group(child_pid);
        assert!(
            pids_after.is_empty(),
            "orphaned processes remain in group {}: {:?}",
            child_pid,
            pids_after
        );
    }

    /// Same test but for kill_process() (the file-change restart path).
    #[tokio::test]
    async fn kill_process_kills_entire_process_group() {
        let exec = Exec::new(ExecConfig {
            watch: None,
            exec: vec!["sleep 300 & sleep 300 & wait".to_string()],
            ..Default::default()
        });

        let child = exec.spawn_single(&exec.exec[0]).unwrap();
        let child_pid = child.id().unwrap() as i32;
        *exec.child.lock().await = Some(child);

        tokio::time::sleep(Duration::from_millis(200)).await;

        let pids_before: Vec<i32> = get_pids_in_group(child_pid);
        assert!(
            pids_before.len() >= 2,
            "expected at least 2 processes in group, got {:?}",
            pids_before
        );

        exec.kill_process().await;

        tokio::time::sleep(Duration::from_millis(500)).await;

        let pids_after: Vec<i32> = get_pids_in_group(child_pid);
        assert!(
            pids_after.is_empty(),
            "orphaned processes remain in group {}: {:?}",
            child_pid,
            pids_after
        );
    }

    /// Verifies that stop_process() reaps the child after SIGKILL (no zombie left).
    /// Uses a process that traps SIGTERM so the graceful path always times out.
    #[tokio::test]
    async fn stop_process_reaps_child_after_sigkill() {
        let exec = Exec::new(ExecConfig {
            watch: None,
            exec: vec!["trap '' TERM; sleep 300".to_string()],
            ..Default::default()
        });

        let child = exec.spawn_single(&exec.exec[0]).unwrap();
        let child_pid = child.id().unwrap() as i32;
        *exec.child.lock().await = Some(child);

        tokio::time::sleep(Duration::from_millis(200)).await;

        // stop_process sends SIGTERM, waits 3s, then SIGKILL
        exec.stop_process().await;

        // After stop_process returns, the child should be fully reaped (no zombie)
        tokio::time::sleep(Duration::from_millis(200)).await;

        let pids_after: Vec<i32> = get_pids_in_group(child_pid);
        assert!(
            pids_after.is_empty(),
            "zombie or orphaned processes remain in group {}: {:?}",
            child_pid,
            pids_after
        );
    }

    /// Calling shutdown() twice must not panic or error.
    #[tokio::test]
    async fn shutdown_is_idempotent() {
        let exec = Exec::new(ExecConfig {
            watch: None,
            exec: vec!["sleep 300".to_string()],
            ..Default::default()
        });

        let child = exec.spawn_single(&exec.exec[0]).unwrap();
        *exec.child.lock().await = Some(child);

        tokio::time::sleep(Duration::from_millis(200)).await;

        // First shutdown
        exec.shutdown().await;
        // Second shutdown — must not panic
        exec.shutdown().await;
    }

    /// restart.on_crash respawns the long-lived command when it exits. The
    /// daemon appends a line on each spawn and exits immediately; after a brief
    /// window the marker must show multiple spawns.
    #[tokio::test]
    async fn restart_on_crash_respawns_the_daemon() {
        let dir = temp_repo_dir("shell-exec-respawn");
        fs::create_dir_all(&dir).expect("create temp dir");
        let marker = dir.join("spawns.log");

        let exec = Exec::new(ExecConfig {
            watch: None,
            exec: vec![format!("echo x >> {}; exit 1", marker.display())],
            restart: Some(RestartPolicy {
                on_crash: true,
                backoff_secs: 0,
                max_backoff_secs: 0,
            }),
            health: None,
        });

        let runner = tokio::spawn({
            let exec = exec.clone();
            async move { exec.run().await }
        });

        tokio::time::sleep(Duration::from_millis(300)).await;
        exec.shutdown().await;
        let _ = runner.await.expect("join run task");

        let spawns = fs::read_to_string(&marker)
            .unwrap_or_default()
            .lines()
            .count();
        assert!(
            spawns >= 2,
            "daemon should have respawned, saw {spawns} spawn(s)"
        );

        fs::remove_dir_all(dir).ok();
    }

    /// Back-compat: with NO restart/health, the long-lived command is spawned
    /// exactly once and never respawned (today's behavior, byte-identical).
    #[tokio::test]
    async fn without_restart_daemon_is_spawned_once() {
        let dir = temp_repo_dir("shell-exec-norestart");
        fs::create_dir_all(&dir).expect("create temp dir");
        let marker = dir.join("spawns.log");

        let exec = Exec::new(ExecConfig {
            watch: None,
            exec: vec![format!("echo x >> {}", marker.display())],
            ..Default::default()
        });

        let runner = tokio::spawn({
            let exec = exec.clone();
            async move { exec.run().await }
        });

        tokio::time::sleep(Duration::from_millis(300)).await;
        exec.shutdown().await;
        let _ = runner.await.expect("join run task");

        let spawns = fs::read_to_string(&marker)
            .unwrap_or_default()
            .lines()
            .count();
        assert_eq!(
            spawns, 1,
            "without restart the daemon must spawn exactly once, saw {spawns}"
        );

        fs::remove_dir_all(dir).ok();
    }

    /// A failing health probe kills and respawns the daemon after the failure
    /// threshold. The daemon stays up (sleep) and appends on each spawn; an
    /// always-failing `command` probe must force at least one respawn.
    #[tokio::test]
    async fn health_probe_failure_kills_and_respawns() {
        let dir = temp_repo_dir("shell-exec-health");
        fs::create_dir_all(&dir).expect("create temp dir");
        let marker = dir.join("spawns.log");

        let exec = Exec::new(ExecConfig {
            watch: None,
            exec: vec![format!("echo x >> {}; sleep 300", marker.display())],
            restart: None,
            health: Some(HealthCheck {
                url: None,
                command: Some("false".to_string()),
                interval_secs: 1,
                timeout_secs: 1,
                failure_threshold: 1,
            }),
        });

        let runner = tokio::spawn({
            let exec = exec.clone();
            async move { exec.run().await }
        });

        // Initial tick is consumed; ~1s later the first probe fails → respawn.
        tokio::time::sleep(Duration::from_millis(2500)).await;
        exec.shutdown().await;
        let _ = runner.await.expect("join run task");

        let spawns = fs::read_to_string(&marker)
            .unwrap_or_default()
            .lines()
            .count();
        assert!(
            spawns >= 2,
            "failing health probe should have respawned the daemon, saw {spawns} spawn(s)"
        );

        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn should_restart_uses_notify_event_kind_and_watch_patterns() {
        let root = temp_repo_dir("shell-exec-should-restart");
        fs::create_dir_all(root.join("nested")).expect("create temp directory");

        let exec = Exec::new(ExecConfig {
            watch: Some(vec![format!(
                "{}/**/*.txt",
                root.file_name().expect("temp dir name").to_string_lossy()
            )]),
            exec: vec![],
            ..Default::default()
        });

        let matching = Event::new(EventKind::Modify(ModifyKind::Data(DataChange::Content)))
            .add_path(root.join("nested").join("file.txt"));
        assert!(exec.should_restart(&matching));

        let wrong_extension = Event::new(EventKind::Create(CreateKind::File))
            .add_path(root.join("nested").join("file.rs"));
        assert!(!exec.should_restart(&wrong_extension));

        let wrong_kind =
            Event::new(EventKind::Create(CreateKind::Folder)).add_path(root.join("nested"));
        assert!(!exec.should_restart(&wrong_kind));

        fs::remove_dir_all(root).expect("remove temp directory");
    }

    #[tokio::test]
    async fn run_returns_after_shutdown_without_watchers() {
        let exec = Exec::new(ExecConfig {
            watch: None,
            exec: vec![],
            ..Default::default()
        });

        let runner = tokio::spawn({
            let exec = exec.clone();
            async move { exec.run().await }
        });

        tokio::time::sleep(Duration::from_millis(100)).await;
        exec.shutdown().await;

        let result = runner.await.expect("join run task");
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn run_pipeline_stops_when_shutdown_interrupts_intermediate_step() {
        let output_dir = temp_repo_dir("shell-exec-pipeline");
        fs::create_dir_all(&output_dir).expect("create temp directory");
        let output_file = output_dir.join("should-not-exist.txt");

        let exec = Exec::new(ExecConfig {
            watch: None,
            exec: vec![
                "sleep 30".to_string(),
                format!("printf done > {}", output_file.display()),
            ],
            ..Default::default()
        });

        let pipeline = tokio::spawn({
            let exec = exec.clone();
            async move { exec.run_pipeline().await }
        });

        tokio::time::sleep(Duration::from_millis(100)).await;
        exec.shutdown().await;

        let result = pipeline.await.expect("join pipeline task");
        assert!(result.is_ok());
        assert!(
            !output_file.exists(),
            "shutdown should abort later pipeline steps"
        );

        fs::remove_dir_all(output_dir).expect("remove temp directory");
    }

    #[tokio::test]
    async fn run_pipeline_aborts_after_failed_step() {
        let output_dir = temp_repo_dir("shell-exec-failure");
        fs::create_dir_all(&output_dir).expect("create temp directory");
        let output_file = output_dir.join("should-not-exist.txt");

        let exec = Exec::new(ExecConfig {
            watch: None,
            exec: vec![
                "false".to_string(),
                format!("printf done > {}", output_file.display()),
            ],
            ..Default::default()
        });

        exec.run_pipeline().await.expect("run pipeline");
        assert!(
            !output_file.exists(),
            "pipeline should stop after a failed step"
        );

        fs::remove_dir_all(output_dir).expect("remove temp directory");
    }

    #[tokio::test]
    async fn run_restarts_pipeline_when_watched_file_changes() {
        let root = temp_repo_dir("shell-exec-run");
        let watch_dir = root.join("watch");
        fs::create_dir_all(&watch_dir).expect("create watch directory");
        let watched_file = watch_dir.join("input.txt");
        let output_file = root.join("output.txt");
        fs::write(&watched_file, "initial").expect("seed watched file");

        let pattern = format!(
            "{}/**/*.txt",
            root.file_name().expect("temp dir name").to_string_lossy()
        );
        let exec = Exec::new(ExecConfig {
            watch: Some(vec![pattern]),
            exec: vec![format!("printf x >> {}; sleep 5", output_file.display())],
            ..Default::default()
        });

        let runner = tokio::spawn({
            let exec = exec.clone();
            async move { exec.run().await }
        });

        tokio::time::sleep(Duration::from_millis(400)).await;
        fs::write(&watched_file, "changed").expect("update watched file");

        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            let len = fs::read_to_string(&output_file)
                .unwrap_or_default()
                .chars()
                .count();
            if len >= 2 {
                break;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "pipeline did not restart after file change"
            );
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        exec.shutdown().await;
        let result = runner.await.expect("join run task");
        assert!(result.is_ok());

        fs::remove_dir_all(root).expect("remove temp directory");
    }

    /// Returns PIDs of all alive processes whose PGID matches the given group id.
    fn get_pids_in_group(pgid: i32) -> Vec<i32> {
        let output = std::process::Command::new("ps")
            .args(["-eo", "pid,pgid"])
            .output()
            .expect("failed to run ps");
        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout
            .lines()
            .skip(1) // header
            .filter_map(|line| {
                let mut cols = line.split_whitespace();
                let pid: i32 = cols.next()?.parse().ok()?;
                let group: i32 = cols.next()?.parse().ok()?;
                if group == pgid { Some(pid) } else { None }
            })
            .collect()
    }
}
