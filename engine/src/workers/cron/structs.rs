// Copyright Motia LLC and/or licensed to Motia LLC under one or more
// contributor license agreements. Licensed under the Elastic License 2.0;
// you may not use this file except in compliance with the Elastic License 2.0.
// This software is patent protected. We welcome discussions - reach out at team@iii.dev
// See LICENSE and PATENTS files for details.

use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use colored::Colorize;
use cron::Schedule;
use tokio::{task::JoinHandle, time::sleep};
use tracing::Instrument;

use crate::condition::check_condition;
use crate::engine::{Engine, EngineTrait};

/// Trait for cron scheduling operations
#[async_trait]
pub trait CronSchedulerAdapter: Send + Sync + 'static {
    /// Try to acquire a distributed lock for a cron job
    async fn try_acquire_lock(&self, job_id: &str) -> bool;

    /// Release the distributed lock for a cron job
    async fn release_lock(&self, job_id: &str);
}

/// Wall-clock and sleep source for the cron loop.
///
/// Injected (instead of calling `Utc::now()` / `tokio::time::sleep` inline) so
/// tests can drive the scheduler deterministically and reproduce the clock
/// skew described on [`next_fire`].
#[async_trait]
pub(crate) trait CronClock: Send + Sync + 'static {
    fn now(&self) -> DateTime<Utc>;
    async fn sleep(&self, duration: Duration);
}

/// Production clock: `chrono::Utc::now()` for the wall clock, `tokio::time::sleep`
/// (monotonic clock) for waiting.
pub(crate) struct SystemClock;

#[async_trait]
impl CronClock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }

    async fn sleep(&self, duration: Duration) {
        sleep(duration).await
    }
}

/// The next slot to fire: strictly after `now` AND strictly after the slot
/// fired last, so a slot can never fire twice.
///
/// Why `last_fired` matters (hex incident, 2026-08-28 to 2026-09-23): the job
/// loop sleeps on the monotonic clock but schedules on the wall clock. Over a
/// 24h sleep the two drifted ~250ms, so the loop woke at 04:59:59.78 for a
/// 05:00:00 slot. Computing "next" from that pre-boundary `now` returned the
/// same 05:00:00 slot, and every daily job whose handler finished before the
/// wall clock reached 05:00:00 fired twice (04:59:59.8 and 05:00:00.0).
pub(crate) fn next_fire(
    schedule: &Schedule,
    now: DateTime<Utc>,
    last_fired: Option<DateTime<Utc>>,
) -> Option<DateTime<Utc>> {
    let anchor = match last_fired {
        Some(last) if last > now => last,
        _ => now,
    };
    schedule.after(&anchor).next()
}

pub(crate) struct CronJobInfo {
    #[allow(dead_code)]
    pub id: String,
    #[allow(dead_code)]
    pub schedule: Schedule,
    pub function_id: String,
    #[allow(dead_code)]
    pub condition_function_id: Option<String>,
    pub task_handle: JoinHandle<()>,
}

pub struct CronAdapter {
    adapter: Arc<dyn CronSchedulerAdapter>,
    jobs: Arc<tokio::sync::RwLock<HashMap<String, CronJobInfo>>>,
    engine: Arc<Engine>,
    clock: Arc<dyn CronClock>,
    shutdown_tx: tokio::sync::watch::Sender<bool>,
    shutdown_rx: tokio::sync::watch::Receiver<bool>,
    shutdown_called: AtomicBool,
}

impl CronAdapter {
    pub fn new(scheduler: Arc<dyn CronSchedulerAdapter>, engine: Arc<Engine>) -> Self {
        Self::new_with_clock(scheduler, engine, Arc::new(SystemClock))
    }

    pub(crate) fn new_with_clock(
        scheduler: Arc<dyn CronSchedulerAdapter>,
        engine: Arc<Engine>,
        clock: Arc<dyn CronClock>,
    ) -> Self {
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        Self {
            adapter: scheduler,
            jobs: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            engine,
            clock,
            shutdown_tx,
            shutdown_rx,
            shutdown_called: AtomicBool::new(false),
        }
    }

    /// Parse a cron expression string into a Schedule
    fn parse_cron_expression(expression: &str) -> anyhow::Result<Schedule> {
        expression
            .parse::<Schedule>()
            .map_err(|e| anyhow::anyhow!("Invalid cron expression '{}': {}", expression, e))
    }

    /// Start a cron job that will trigger at the specified schedule
    async fn start_cron_job(
        &self,
        id: String,
        schedule: Schedule,
        function_id: String,
        condition_function_id: Option<String>,
    ) -> JoinHandle<()> {
        let scheduler = Arc::clone(&self.adapter);
        let engine = Arc::clone(&self.engine);
        let clock = Arc::clone(&self.clock);
        let job_id = id.clone();
        let function_id = function_id.clone();
        let mut shutdown_rx = self.shutdown_rx.clone();

        tokio::spawn(async move {
            tracing::debug!(job_id = %job_id, function_id = %function_id, "Starting cron job loop");

            // The slot fired most recently. `next_fire` anchors on it so the
            // same slot is never scheduled twice, whatever the wall clock says.
            let mut last_fired: Option<DateTime<Utc>> = None;

            'job: loop {
                // Calculate time until next execution
                let now = clock.now();
                let next: DateTime<Utc> = match next_fire(&schedule, now, last_fired) {
                    Some(next) => next,
                    None => {
                        tracing::warn!(job_id = %job_id, "No upcoming schedule found for cron job");
                        break;
                    }
                };

                tracing::debug!(
                    job_id = %job_id,
                    next_run = %next,
                    duration_secs = (next - now).num_seconds(),
                    "Waiting for next cron execution"
                );

                // Wait until the WALL clock reaches `next`, or shutdown. The
                // sleep runs on the monotonic clock and can return before the
                // wall clock gets there (observed: ~250ms over a 24h sleep), so
                // re-check after every wake and sleep the remainder instead of
                // firing early.
                loop {
                    let now = clock.now();
                    if now >= next {
                        break;
                    }
                    let remaining = (next - now).to_std().unwrap_or(Duration::ZERO);
                    tokio::select! {
                        _ = clock.sleep(remaining) => {}
                        _ = shutdown_rx.changed() => {
                            tracing::info!(job_id = %job_id, "Cron job received shutdown signal");
                            break 'job;
                        }
                    }
                    if clock.now() < next {
                        tracing::debug!(
                            job_id = %job_id,
                            next_run = %next,
                            early_by_ms = (next - clock.now()).num_milliseconds(),
                            "Sleep returned before the wall clock reached the slot; sleeping the remainder"
                        );
                    }
                }

                // Check shutdown flag after waking
                if *shutdown_rx.borrow() {
                    tracing::info!(job_id = %job_id, "Cron job shutting down");
                    break;
                }

                last_fired = Some(next);

                // Try to acquire the distributed lock
                if scheduler.try_acquire_lock(&job_id).await {
                    let cron_span = tracing::info_span!(
                        "cron_trigger",
                        otel.name = %format!("call {}", function_id),
                        otel.kind = "producer",
                        otel.status_code = tracing::field::Empty,
                        job_id = %job_id,
                        function_id = %function_id,
                        "faas.trigger" = "timer",
                    );

                    async {
                        tracing::info!(
                            "{} Cron job {} → {}",
                            "[TRIGGERED]".green(),
                            job_id.purple(),
                            function_id.cyan()
                        );

                        // Create the cron event payload
                        let event_data = serde_json::json!({
                            "trigger": "cron",
                            "job_id": job_id,
                            "scheduled_time": next.to_rfc3339(),
                            "actual_time": clock.now().to_rfc3339(),
                        });

                        if let Some(ref condition_id) = condition_function_id {
                            tracing::debug!(
                                condition_function_id = %condition_id,
                                "Checking trigger conditions"
                            );
                            match check_condition(engine.as_ref(), condition_id, event_data.clone())
                                .await
                            {
                                Ok(true) => {}
                                Ok(false) => {
                                    tracing::debug!(
                                        function_id = %function_id,
                                        "Condition check failed, skipping handler"
                                    );
                                    tracing::Span::current().record("otel.status_code", "OK");
                                    scheduler.release_lock(&job_id).await;
                                    return;
                                }
                                Err(err) => {
                                    tracing::error!(
                                        condition_function_id = %condition_id,
                                        error = ?err,
                                        "Error invoking condition function"
                                    );
                                    tracing::Span::current().record("otel.status_code", "ERROR");
                                    scheduler.release_lock(&job_id).await;
                                    return;
                                }
                            }
                        }

                        match engine.call(&function_id, event_data).await {
                            Ok(_) => {
                                crate::workers::telemetry::collector::track_cron_execution();
                                tracing::Span::current().record("otel.status_code", "OK");
                            }
                            Err(e) => {
                                tracing::error!(
                                    job_id = %job_id,
                                    function_id = %function_id,
                                    error = ?e,
                                    "Cron job execution failed"
                                );
                                tracing::Span::current().record("otel.status_code", "ERROR");
                            }
                        }

                        // Release the lock regardless of success or error
                        scheduler.release_lock(&job_id).await;
                    }
                    .instrument(cron_span)
                    .await;
                } else {
                    tracing::debug!(
                        job_id = %job_id,
                        "Skipping cron execution - another instance is handling it"
                    );
                }
            }

            tracing::debug!(job_id = %job_id, "Cron job loop ended");
        })
    }

    /// Register a new cron trigger
    pub async fn register(
        &self,
        id: &str,
        cron_expression: &str,
        function_id: &str,
        condition_function_id: Option<String>,
    ) -> anyhow::Result<()> {
        // Check if already registered
        {
            let jobs = self.jobs.read().await;
            if jobs.contains_key(id) {
                return Err(anyhow::anyhow!("Cron job '{}' is already registered", id));
            }
        }

        // Parse the cron expression
        let schedule = Self::parse_cron_expression(cron_expression)?;

        tracing::info!(
            "{} Cron job {} ({}) → {}",
            "[REGISTERED]".green(),
            id.purple(),
            cron_expression.yellow(),
            function_id.cyan()
        );

        // Start the cron job
        let task_handle = self
            .start_cron_job(
                id.to_string(),
                schedule.clone(),
                function_id.to_string(),
                condition_function_id.clone(),
            )
            .await;

        // Store the job info
        let mut jobs = self.jobs.write().await;
        jobs.insert(
            id.to_string(),
            CronJobInfo {
                id: id.to_string(),
                schedule,
                function_id: function_id.to_string(),
                condition_function_id,
                task_handle,
            },
        );

        Ok(())
    }

    /// Unregister a cron trigger
    pub async fn unregister(&self, id: &str) -> anyhow::Result<()> {
        let mut jobs = self.jobs.write().await;

        if let Some(job_info) = jobs.remove(id) {
            tracing::info!(
                "{} Cron job {} → {}",
                "[UNREGISTERED]".yellow(),
                id.purple(),
                job_info.function_id.cyan()
            );

            // Abort the task
            job_info.task_handle.abort();

            // Release any held lock
            self.adapter.release_lock(id).await;

            Ok(())
        } else {
            Err(anyhow::anyhow!("Cron job '{}' not found", id))
        }
    }

    /// Shutdown all cron jobs by signaling them and aborting any that don't stop
    pub async fn shutdown(&self) {
        if self.shutdown_called.swap(true, Ordering::SeqCst) {
            return;
        }
        tracing::info!("Shutting down all cron jobs");
        let _ = self.shutdown_tx.send(true);
        tokio::time::sleep(Duration::from_millis(50)).await;
        let mut jobs = self.jobs.write().await;
        for (id, job_info) in jobs.drain() {
            if !job_info.task_handle.is_finished() {
                tracing::debug!(job_id = %id, "Force-aborting cron job task");
                job_info.task_handle.abort();
            }
            self.adapter.release_lock(&id).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use serde_json::Value;

    use crate::{
        engine::{EngineTrait, Handler, RegisterFunctionRequest},
        function::FunctionResult,
    };

    use super::*;

    struct NoopSchedulerAdapter;

    #[async_trait]
    impl CronSchedulerAdapter for NoopSchedulerAdapter {
        async fn try_acquire_lock(&self, _job_id: &str) -> bool {
            false
        }

        async fn release_lock(&self, _job_id: &str) {}
    }

    fn test_engine() -> Arc<Engine> {
        crate::workers::observability::metrics::ensure_default_meter();
        Arc::new(Engine::new())
    }

    #[tokio::test]
    async fn shutdown_signal_stops_cron_jobs() {
        let engine = test_engine();
        let scheduler: Arc<dyn CronSchedulerAdapter> = Arc::new(NoopSchedulerAdapter);
        let adapter = CronAdapter::new(scheduler, engine);

        // Register a cron job with an hourly schedule (won't fire during the test)
        adapter
            .register("test-job-1", "0 0 * * * *", "test-function", None)
            .await
            .expect("Failed to register cron job");

        // Verify the job is running
        {
            let jobs = adapter.jobs.read().await;
            assert_eq!(jobs.len(), 1, "Expected one registered job");
            let job = jobs.get("test-job-1").unwrap();
            assert!(
                !job.task_handle.is_finished(),
                "Job task should still be running"
            );
        }

        // Shutdown and wait a bit for tasks to stop
        adapter.shutdown().await;
        tokio::time::sleep(Duration::from_millis(100)).await;

        // After shutdown, the jobs map should be drained
        let jobs = adapter.jobs.read().await;
        assert!(jobs.is_empty(), "All jobs should be drained after shutdown");
    }

    /// Calling shutdown() twice must not panic or error.
    #[tokio::test]
    async fn shutdown_is_idempotent() {
        let engine = test_engine();
        let adapter = CronAdapter::new(Arc::new(NoopSchedulerAdapter), engine);

        adapter
            .register("job-1", "0 0 * * * *", "fn-1", None)
            .await
            .unwrap();

        adapter.shutdown().await;
        // Second call must not panic or deadlock
        adapter.shutdown().await;
    }

    /// Calling shutdown() directly (as destroy would) should abort all tasks
    /// even if no external shutdown signal was sent first.
    #[tokio::test]
    async fn shutdown_aborts_all_task_handles() {
        let engine = test_engine();
        let adapter = CronAdapter::new(Arc::new(NoopSchedulerAdapter), engine);

        // Register multiple cron jobs
        adapter
            .register("job-1", "0 0 * * * *", "fn-1", None)
            .await
            .unwrap();
        adapter
            .register("job-2", "0 30 * * * *", "fn-2", None)
            .await
            .unwrap();

        // Verify both jobs are running
        {
            let jobs = adapter.jobs.read().await;
            assert_eq!(jobs.len(), 2);
        }

        // Call shutdown directly (simulating destroy path)
        adapter.shutdown().await;

        // Verify all tasks are finished and map is drained
        let jobs = adapter.jobs.read().await;
        assert!(jobs.is_empty(), "All jobs should be drained after shutdown");
    }

    struct CountingSchedulerAdapter {
        acquire_calls: Arc<AtomicUsize>,
        release_calls: Arc<AtomicUsize>,
        allow_lock: bool,
    }

    #[async_trait]
    impl CronSchedulerAdapter for CountingSchedulerAdapter {
        async fn try_acquire_lock(&self, _job_id: &str) -> bool {
            self.acquire_calls.fetch_add(1, Ordering::SeqCst);
            self.allow_lock
        }

        async fn release_lock(&self, _job_id: &str) {
            self.release_calls.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn parse_cron_expression_rejects_invalid_input() {
        let error = CronAdapter::parse_cron_expression("not a cron")
            .expect_err("invalid cron expression should fail");
        assert!(error.to_string().contains("Invalid cron expression"));
    }

    #[tokio::test]
    async fn register_rejects_duplicates_and_unregister_missing_job() {
        let engine = test_engine();
        let adapter = CronAdapter::new(Arc::new(NoopSchedulerAdapter), engine);

        adapter
            .register("dup-job", "0 0 * * * *", "fn-1", None)
            .await
            .expect("first registration");

        let duplicate = adapter
            .register("dup-job", "0 0 * * * *", "fn-1", None)
            .await
            .expect_err("duplicate registration should fail");
        assert!(duplicate.to_string().contains("already registered"));

        let missing = adapter
            .unregister("missing-job")
            .await
            .expect_err("missing job should fail");
        assert!(missing.to_string().contains("not found"));

        adapter.shutdown().await;
    }

    /// Incident numbers: the loop woke at 04:59:59.780 for the 05:00:00 slot,
    /// fired, and recomputed from that pre-boundary `now`. With nothing fired
    /// yet the slot is today's; once that slot has fired, the same pre-boundary
    /// `now` must yield tomorrow's slot, never today's again.
    #[test]
    fn next_fire_never_returns_the_slot_just_fired() {
        use chrono::TimeZone;

        let schedule: Schedule = "0 0 5 * * * *".parse().unwrap();
        let slot = Utc.with_ymd_and_hms(2026, 9, 23, 5, 0, 0).unwrap();
        let early_wake = slot - chrono::Duration::milliseconds(220);

        assert_eq!(next_fire(&schedule, early_wake, None), Some(slot));
        assert_eq!(
            next_fire(&schedule, early_wake, Some(slot)),
            Some(slot + chrono::Duration::days(1))
        );
        // A normal (late) wake still moves on to the next slot.
        assert_eq!(
            next_fire(
                &schedule,
                slot + chrono::Duration::milliseconds(9),
                Some(slot)
            ),
            Some(slot + chrono::Duration::days(1))
        );
        // Wall clock jumped ahead past last_fired: anchor on now.
        let later = slot + chrono::Duration::hours(30);
        assert_eq!(
            next_fire(&schedule, later, Some(slot)),
            Some(slot + chrono::Duration::days(2))
        );
    }

    /// Wall clock that gains slightly less than each monotonic sleep, so a
    /// sleep returns BEFORE the wall clock reaches the target. This is the
    /// skew the hex harness sees on macOS: `tokio::time::sleep` runs on the
    /// monotonic clock, `Utc::now()` is NTP-disciplined, and over a 24h sleep
    /// they diverge by ~250ms.
    struct SkewedClock {
        now: std::sync::Mutex<DateTime<Utc>>,
        slow_ppm: f64,
    }

    #[async_trait]
    impl CronClock for SkewedClock {
        fn now(&self) -> DateTime<Utc> {
            *self.now.lock().unwrap()
        }

        async fn sleep(&self, duration: Duration) {
            // Paused tokio time auto-advances this instantly in tests.
            tokio::time::sleep(duration).await;
            let advance = duration.mul_f64(1.0 - self.slow_ppm * 1e-6);
            let mut now = self.now.lock().unwrap();
            *now += chrono::Duration::from_std(advance).expect("advance fits");
        }
    }

    /// Incident: hex-applier::daily-run-0500 (`0 0 5 * * * *`) landed two
    /// telemetry rows on 19 of 23 days, e.g. 2026-09-23T04:59:59.815 and
    /// 2026-09-23T05:00:00.183, with the same for every other daily cron whose
    /// handler finishes in under a second. The loop woke ~220ms before the wall
    /// clock reached 05:00:00, fired, recomputed "next" from that pre-boundary
    /// `now`, got 05:00:00 back again and fired it a second time.
    ///
    /// With a 3ppm-slow wall clock a 24h sleep wakes 259ms early. Over three
    /// mock days the job must fire exactly three times, each at or after its
    /// slot, never the same slot twice.
    #[tokio::test(start_paused = true)]
    async fn daily_job_fires_each_slot_once_when_sleep_wakes_before_wall_clock() {
        use chrono::TimeZone;

        let engine = test_engine();
        let fires: Arc<std::sync::Mutex<Vec<(String, String)>>> =
            Arc::new(std::sync::Mutex::new(Vec::new()));
        let fires_clone = fires.clone();
        engine.register_function_handler(
            RegisterFunctionRequest {
                function_id: "cron.handler.daily".to_string(),
                description: None,
                request_format: None,
                response_format: None,
                metadata: None,
            },
            Handler::new(move |input: Value| {
                let fires_clone = fires_clone.clone();
                async move {
                    fires_clone.lock().unwrap().push((
                        input["scheduled_time"].as_str().unwrap().to_string(),
                        input["actual_time"].as_str().unwrap().to_string(),
                    ));
                    FunctionResult::Success(Some(serde_json::json!({ "ok": true })))
                }
            }),
        );

        let start = Utc.with_ymd_and_hms(2026, 9, 22, 5, 0, 0).unwrap()
            + chrono::Duration::milliseconds(500);
        let clock = Arc::new(SkewedClock {
            now: std::sync::Mutex::new(start),
            slow_ppm: 3.0,
        });
        let adapter = CronAdapter::new_with_clock(
            Arc::new(CountingSchedulerAdapter {
                acquire_calls: Arc::new(AtomicUsize::new(0)),
                release_calls: Arc::new(AtomicUsize::new(0)),
                allow_lock: true,
            }),
            engine,
            clock.clone(),
        );

        adapter
            .register("daily-0500", "0 0 5 * * * *", "cron.handler.daily", None)
            .await
            .expect("register daily cron job");

        // Three slots: 09-23, 09-24, 09-25 at 05:00. Paused time auto-advances
        // through the job's 24h sleeps before this one completes.
        tokio::time::sleep(Duration::from_secs(3 * 86_400 + 3_600)).await;
        adapter.shutdown().await;

        let fires = fires.lock().unwrap().clone();
        let scheduled: Vec<&str> = fires.iter().map(|(s, _)| s.as_str()).collect();
        assert_eq!(
            scheduled,
            vec![
                "2026-09-23T05:00:00+00:00",
                "2026-09-24T05:00:00+00:00",
                "2026-09-25T05:00:00+00:00",
            ],
            "each slot fires exactly once; got {fires:?}"
        );
        for (scheduled, actual) in &fires {
            let s = DateTime::parse_from_rfc3339(scheduled).unwrap();
            let a = DateTime::parse_from_rfc3339(actual).unwrap();
            assert!(
                a >= s,
                "fired before the wall clock reached the slot: scheduled {scheduled} actual {actual}"
            );
        }
    }

    #[tokio::test]
    async fn cron_job_executes_handler_and_releases_lock() {
        let acquire_calls = Arc::new(AtomicUsize::new(0));
        let release_calls = Arc::new(AtomicUsize::new(0));
        let executions = Arc::new(AtomicUsize::new(0));

        let engine = test_engine();
        let executions_clone = executions.clone();
        engine.register_function_handler(
            RegisterFunctionRequest {
                function_id: "cron.handler".to_string(),
                description: None,
                request_format: None,
                response_format: None,
                metadata: None,
            },
            Handler::new(move |input: Value| {
                let executions_clone = executions_clone.clone();
                async move {
                    assert_eq!(input["trigger"], "cron");
                    executions_clone.fetch_add(1, Ordering::SeqCst);
                    FunctionResult::Success(Some(serde_json::json!({ "ok": true })))
                }
            }),
        );

        let adapter = CronAdapter::new(
            Arc::new(CountingSchedulerAdapter {
                acquire_calls: acquire_calls.clone(),
                release_calls: release_calls.clone(),
                allow_lock: true,
            }),
            engine,
        );

        adapter
            .register("tick-job", "* * * * * *", "cron.handler", None)
            .await
            .expect("register cron job");

        tokio::time::sleep(Duration::from_millis(1200)).await;
        adapter.shutdown().await;

        assert!(acquire_calls.load(Ordering::SeqCst) >= 1);
        assert!(release_calls.load(Ordering::SeqCst) >= 1);
        assert!(executions.load(Ordering::SeqCst) >= 1);
    }

    #[tokio::test]
    async fn cron_job_skips_handler_when_condition_returns_false() {
        let release_calls = Arc::new(AtomicUsize::new(0));
        let executions = Arc::new(AtomicUsize::new(0));

        let engine = test_engine();
        engine.register_function_handler(
            RegisterFunctionRequest {
                function_id: "cron.condition".to_string(),
                description: None,
                request_format: None,
                response_format: None,
                metadata: None,
            },
            Handler::new(|_input: Value| async move {
                FunctionResult::Success(Some(serde_json::json!(false)))
            }),
        );

        let executions_clone = executions.clone();
        engine.register_function_handler(
            RegisterFunctionRequest {
                function_id: "cron.handler.false".to_string(),
                description: None,
                request_format: None,
                response_format: None,
                metadata: None,
            },
            Handler::new(move |_input: Value| {
                let executions_clone = executions_clone.clone();
                async move {
                    executions_clone.fetch_add(1, Ordering::SeqCst);
                    FunctionResult::Success(Some(serde_json::json!({ "ok": true })))
                }
            }),
        );

        let adapter = CronAdapter::new(
            Arc::new(CountingSchedulerAdapter {
                acquire_calls: Arc::new(AtomicUsize::new(0)),
                release_calls: release_calls.clone(),
                allow_lock: true,
            }),
            engine,
        );

        adapter
            .register(
                "cond-job",
                "* * * * * *",
                "cron.handler.false",
                Some("cron.condition".to_string()),
            )
            .await
            .expect("register conditional cron job");

        tokio::time::sleep(Duration::from_millis(1200)).await;
        adapter.shutdown().await;

        assert_eq!(executions.load(Ordering::SeqCst), 0);
        assert!(release_calls.load(Ordering::SeqCst) >= 1);
    }

    #[tokio::test]
    async fn cron_job_runs_handler_when_condition_returns_none() {
        let release_calls = Arc::new(AtomicUsize::new(0));
        let executions = Arc::new(AtomicUsize::new(0));

        let engine = test_engine();
        engine.register_function_handler(
            RegisterFunctionRequest {
                function_id: "cron.condition.none".to_string(),
                description: None,
                request_format: None,
                response_format: None,
                metadata: None,
            },
            Handler::new(|_input: Value| async move { FunctionResult::Success(None) }),
        );

        let executions_clone = executions.clone();
        engine.register_function_handler(
            RegisterFunctionRequest {
                function_id: "cron.handler.none".to_string(),
                description: None,
                request_format: None,
                response_format: None,
                metadata: None,
            },
            Handler::new(move |_input: Value| {
                let executions_clone = executions_clone.clone();
                async move {
                    executions_clone.fetch_add(1, Ordering::SeqCst);
                    FunctionResult::Success(Some(serde_json::json!({ "ok": true })))
                }
            }),
        );

        let adapter = CronAdapter::new(
            Arc::new(CountingSchedulerAdapter {
                acquire_calls: Arc::new(AtomicUsize::new(0)),
                release_calls: release_calls.clone(),
                allow_lock: true,
            }),
            engine,
        );

        adapter
            .register(
                "cond-none-job",
                "* * * * * *",
                "cron.handler.none",
                Some("cron.condition.none".to_string()),
            )
            .await
            .expect("register conditional cron job");

        tokio::time::sleep(Duration::from_millis(1200)).await;
        adapter.shutdown().await;

        assert!(executions.load(Ordering::SeqCst) >= 1);
        assert!(release_calls.load(Ordering::SeqCst) >= 1);
    }

    #[tokio::test]
    async fn cron_job_releases_lock_when_condition_errors() {
        let release_calls = Arc::new(AtomicUsize::new(0));
        let executions = Arc::new(AtomicUsize::new(0));

        let engine = test_engine();
        engine.register_function_handler(
            RegisterFunctionRequest {
                function_id: "cron.condition.err".to_string(),
                description: None,
                request_format: None,
                response_format: None,
                metadata: None,
            },
            Handler::new(|_input: Value| async move {
                FunctionResult::Failure(crate::protocol::ErrorBody {
                    code: "COND".to_string(),
                    message: "condition failed".to_string(),
                    stacktrace: None,
                })
            }),
        );

        let executions_clone = executions.clone();
        engine.register_function_handler(
            RegisterFunctionRequest {
                function_id: "cron.handler.err".to_string(),
                description: None,
                request_format: None,
                response_format: None,
                metadata: None,
            },
            Handler::new(move |_input: Value| {
                let executions_clone = executions_clone.clone();
                async move {
                    executions_clone.fetch_add(1, Ordering::SeqCst);
                    FunctionResult::Success(Some(serde_json::json!({ "ok": true })))
                }
            }),
        );

        let adapter = CronAdapter::new(
            Arc::new(CountingSchedulerAdapter {
                acquire_calls: Arc::new(AtomicUsize::new(0)),
                release_calls: release_calls.clone(),
                allow_lock: true,
            }),
            engine,
        );

        adapter
            .register(
                "cond-err-job",
                "* * * * * *",
                "cron.handler.err",
                Some("cron.condition.err".to_string()),
            )
            .await
            .expect("register conditional cron job");

        tokio::time::sleep(Duration::from_millis(1200)).await;
        adapter.shutdown().await;

        assert_eq!(executions.load(Ordering::SeqCst), 0);
        assert!(release_calls.load(Ordering::SeqCst) >= 1);
    }

    #[tokio::test]
    async fn cron_job_releases_lock_when_handler_errors() {
        let release_calls = Arc::new(AtomicUsize::new(0));
        let attempts = Arc::new(AtomicUsize::new(0));

        let engine = test_engine();
        let attempts_clone = attempts.clone();
        engine.register_function_handler(
            RegisterFunctionRequest {
                function_id: "cron.handler.failure".to_string(),
                description: None,
                request_format: None,
                response_format: None,
                metadata: None,
            },
            Handler::new(move |_input: Value| {
                let attempts_clone = attempts_clone.clone();
                async move {
                    attempts_clone.fetch_add(1, Ordering::SeqCst);
                    FunctionResult::Failure(crate::protocol::ErrorBody {
                        code: "HANDLER".to_string(),
                        message: "handler failed".to_string(),
                        stacktrace: None,
                    })
                }
            }),
        );

        let adapter = CronAdapter::new(
            Arc::new(CountingSchedulerAdapter {
                acquire_calls: Arc::new(AtomicUsize::new(0)),
                release_calls: release_calls.clone(),
                allow_lock: true,
            }),
            engine,
        );

        adapter
            .register(
                "handler-err-job",
                "* * * * * *",
                "cron.handler.failure",
                None,
            )
            .await
            .expect("register cron job");

        tokio::time::sleep(Duration::from_millis(1200)).await;
        adapter.shutdown().await;

        assert!(attempts.load(Ordering::SeqCst) >= 1);
        assert!(release_calls.load(Ordering::SeqCst) >= 1);
    }

    #[tokio::test]
    async fn cron_job_skips_execution_when_lock_is_not_acquired() {
        let acquire_calls = Arc::new(AtomicUsize::new(0));
        let executions = Arc::new(AtomicUsize::new(0));

        let engine = test_engine();
        let executions_clone = executions.clone();
        engine.register_function_handler(
            RegisterFunctionRequest {
                function_id: "cron.handler.locked".to_string(),
                description: None,
                request_format: None,
                response_format: None,
                metadata: None,
            },
            Handler::new(move |_input: Value| {
                let executions_clone = executions_clone.clone();
                async move {
                    executions_clone.fetch_add(1, Ordering::SeqCst);
                    FunctionResult::Success(Some(serde_json::json!({ "ok": true })))
                }
            }),
        );

        let adapter = CronAdapter::new(
            Arc::new(CountingSchedulerAdapter {
                acquire_calls: acquire_calls.clone(),
                release_calls: Arc::new(AtomicUsize::new(0)),
                allow_lock: false,
            }),
            engine,
        );

        adapter
            .register("locked-job", "* * * * * *", "cron.handler.locked", None)
            .await
            .expect("register cron job");

        tokio::time::sleep(Duration::from_millis(1200)).await;
        adapter.shutdown().await;

        assert!(acquire_calls.load(Ordering::SeqCst) >= 1);
        assert_eq!(executions.load(Ordering::SeqCst), 0);
    }
}
