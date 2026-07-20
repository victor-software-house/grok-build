//! Heap-leak test for the session lifecycle: create and remove many sessions,
//! then fail if heap memory grows per session. Run:
<<<<<<< HEAD
//!     leader_session_lifecycle_heap_steady_state -- --ignored --nocapture
use super::*;
use xai_grok_workspace::permission::PermissionEvent;
const MAX_BLOCKS_PER_SESSION: f64 = 0.5;
const MAX_BYTES_PER_SESSION: f64 = 1024.0;
=======
//!   cargo test -p xai-grok-shell --features dhat-heap \
//!     leader_session_lifecycle_heap_steady_state -- --ignored --nocapture
use super::*;
use xai_grok_workspace::permission::PermissionEvent;

// Chosen between a healthy build (about zero retained allocations per
// session) and the smallest deliberately introduced leak (one per session);
// re-tune if healthy runs drift toward the limits.
const MAX_BLOCKS_PER_SESSION: f64 = 0.5;
const MAX_BYTES_PER_SESSION: f64 = 1024.0;

>>>>>>> ba76b0a683fa52e4e60685017b85905451be17bc
/// Creates the per-session state that `remove_session` must clean up, then
/// removes the session. A full `SessionHandle` would allocate so much
/// unrelated memory that a small leak would be lost in the noise.
fn populate_and_evict(agent: &MvpAgent, i: usize) {
    let sid = acp::SessionId::new(format!("soak-{i}"));
<<<<<<< HEAD
=======

    // The same workspace binding `spawn_session_actor` creates; if
    // `remove_session` does not release it, the session map holds every
    // toolset for the life of the process.
>>>>>>> ba76b0a683fa52e4e60685017b85905451be17bc
    {
        let ops = agent.workspace_ops.borrow();
        let ops = ops.as_ref().expect("test installs workspace ops");
        let toolset = std::sync::Arc::new(
            xai_grok_tools::registry::types::FinalizedToolset::empty_for_test(),
        );
        ops.bind_local_session(
            sid.0.as_ref(),
            std::env::temp_dir(),
            xai_hunk_tracker::HunkTrackerHandle::noop(),
            toolset,
            None,
        )
        .expect("bind_local_session must succeed");
    }
<<<<<<< HEAD
=======

>>>>>>> ba76b0a683fa52e4e60685017b85905451be17bc
    let (_ptx, prx) = tokio::sync::mpsc::unbounded_channel::<PermissionEvent>();
    agent
        .permission_event_receivers
        .borrow_mut()
        .insert(sid.clone(), prx);
    agent
        .session_turn_numbers
        .borrow_mut()
        .insert(sid.clone(), i as u64);
    agent.model_unavailable_sessions.borrow_mut().insert(
        sid.0.to_string(),
        acp::ModelId::new(std::sync::Arc::from("gone-model")),
    );
<<<<<<< HEAD
    agent.remove_session(&sid);
}
=======

    agent.remove_session(&sid);
}

>>>>>>> ba76b0a683fa52e4e60685017b85905451be17bc
/// Waits for background tasks to finish before reading heap stats.
async fn quiesce() {
    const YIELD_ROUNDS: usize = 50;
    const SETTLE: std::time::Duration = std::time::Duration::from_millis(50);
    for _ in 0..YIELD_ROUNDS {
        tokio::task::yield_now().await;
    }
    tokio::time::sleep(SETTLE).await;
    for _ in 0..YIELD_ROUNDS {
        tokio::task::yield_now().await;
    }
}
<<<<<<< HEAD
=======

>>>>>>> ba76b0a683fa52e4e60685017b85905451be17bc
/// Creating and removing N sessions must not grow the heap.
///
/// Only one `dhat::Profiler` can exist at a time, and the test harness runs
/// tests in parallel, so keep this the only test that creates one.
#[test]
<<<<<<< HEAD
=======
#[ignore = "heap soak; nightly only, needs --features dhat-heap"]
>>>>>>> ba76b0a683fa52e4e60685017b85905451be17bc
fn leader_session_lifecycle_heap_steady_state() {
    run_local_for_bridge_test(|| async {
        let agent = build_minimal_agent_for_tests();
        *agent.workspace_ops.borrow_mut() = Some(xai_grok_workspace::WorkspaceOps::for_test());
        let _profiler = dhat::Profiler::builder().testing().build();
<<<<<<< HEAD
        const WARMUP: usize = 16;
        const MEASURE: usize = 256;
=======

        const WARMUP: usize = 16;
        const MEASURE: usize = 256;

        // The first runs fill caches and one-time allocations; do them before
        // the measured window so they do not count as growth.
>>>>>>> ba76b0a683fa52e4e60685017b85905451be17bc
        for i in 0..WARMUP {
            populate_and_evict(&agent, i);
        }
        quiesce().await;
        let before = dhat::HeapStats::get();
<<<<<<< HEAD
=======

>>>>>>> ba76b0a683fa52e4e60685017b85905451be17bc
        for i in WARMUP..(WARMUP + MEASURE) {
            populate_and_evict(&agent, i);
        }
        quiesce().await;
        let after = dhat::HeapStats::get();
<<<<<<< HEAD
=======

>>>>>>> ba76b0a683fa52e4e60685017b85905451be17bc
        let d_blocks = after.curr_blocks as i64 - before.curr_blocks as i64;
        let d_bytes = after.curr_bytes as i64 - before.curr_bytes as i64;
        let blocks_per = d_blocks as f64 / MEASURE as f64;
        let bytes_per = d_bytes as f64 / MEASURE as f64;
<<<<<<< HEAD
        eprintln!(
            "DHAT_SOAK_SUMMARY {}",
            serde_json::json!({ "warmup_sessions" : WARMUP,
            "measured_sessions" : MEASURE, "before_blocks" : before.curr_blocks,
            "before_bytes" : before.curr_bytes, "after_blocks" : after.curr_blocks,
            "after_bytes" : after.curr_bytes, "blocks_per_session" : blocks_per,
            "bytes_per_session" : bytes_per, "max_blocks_per_session" :
            MAX_BLOCKS_PER_SESSION, "max_bytes_per_session" : MAX_BYTES_PER_SESSION,
            "pass" : blocks_per < MAX_BLOCKS_PER_SESSION && bytes_per <
            MAX_BYTES_PER_SESSION })
        );
=======
        // Printed before the asserts so failing runs still show the numbers.
        eprintln!(
            "DHAT_SOAK_SUMMARY {}",
            serde_json::json!({
                "warmup_sessions": WARMUP,
                "measured_sessions": MEASURE,
                "before_blocks": before.curr_blocks,
                "before_bytes": before.curr_bytes,
                "after_blocks": after.curr_blocks,
                "after_bytes": after.curr_bytes,
                "blocks_per_session": blocks_per,
                "bytes_per_session": bytes_per,
                "max_blocks_per_session": MAX_BLOCKS_PER_SESSION,
                "max_bytes_per_session": MAX_BYTES_PER_SESSION,
                "pass": blocks_per < MAX_BLOCKS_PER_SESSION && bytes_per < MAX_BYTES_PER_SESSION
            })
        );

>>>>>>> ba76b0a683fa52e4e60685017b85905451be17bc
        assert!(
            blocks_per < MAX_BLOCKS_PER_SESSION,
            "block-count leak: {blocks_per:.3} blocks/session retained ({d_blocks} over {MEASURE} cycles) exceeds the {MAX_BLOCKS_PER_SESSION} gate"
        );
        assert!(
            bytes_per < MAX_BYTES_PER_SESSION,
            "byte leak: {bytes_per:.1} bytes/session retained ({d_bytes} over {MEASURE} cycles) exceeds the {MAX_BYTES_PER_SESSION} gate"
        );
    });
}
