// Per-test-case module for the `pty_e2e` integration test crate.
#[allow(unused_imports)]
use super::common::*;

// Default (unset): send pins the new prompt at the viewport top.
// `[ui] page_flip_on_send = false`: send does not move the viewport.

const TAIL_SENTINEL: &str = "TAILSENTINEL_T1";
const SECOND_PROMPT: &str = "second-prompt-marker";

fn tall_first_response() -> String {
    let mut s = String::from("```\n");
    for i in 0..80 {
        s.push_str(&format!("line {i} payload\n"));
    }
    s.push_str(TAIL_SENTINEL);
    s.push_str("\n```\n");
    s
}

/// Welcome → tall turn 1 → submit turn 2 while holding turn 2 open.
<<<<<<< HEAD
async fn drive_to_second_send(content: &ContentController) -> PtyHarness {
    content.set_response(tall_first_response());
=======
async fn drive_to_second_send(content: &ContentController) -> (PtyHarness, AgentTurnExpectation) {
    let mut first_turn =
        content.expect_agent_turn("page-flip tall first turn", tall_first_response());
    let mut second_turn = content.expect_agent_turn_blocked(
        "page-flip held second turn",
        format!("{MOCK_RESPONSE_SENTINEL} second turn."),
    );
>>>>>>> 6e386420825bd44ae648c63e7c8cba12fcec9401

    let binary = pager_binary().expect("resolve pager binary");
    let mut harness =
        PtyHarness::spawn_with_content(&binary, DEFAULT_ROWS, DEFAULT_COLS, content, &[])
            .expect("spawn pager");
    harness
        .wait_for_text(WELCOME_SCREEN_SENTINEL, WELCOME_TIMEOUT)
        .expect("welcome text");
    harness
        .inject_keys(format!("{PROMPT}\r").as_bytes())
        .expect("submit first prompt");
    harness
        .wait_for_text(TAIL_SENTINEL, Duration::from_secs(30))
        .expect("turn 1 tail visible");
<<<<<<< HEAD

    content.hold_agent_completions();
    content.set_response(format!("{MOCK_RESPONSE_SENTINEL} second turn."));
=======
    tokio::time::timeout(Duration::from_secs(10), first_turn.wait_satisfied())
        .await
        .expect("first turn completes before second send");

>>>>>>> 6e386420825bd44ae648c63e7c8cba12fcec9401
    harness
        .inject_keys(format!("{SECOND_PROMPT}\r").as_bytes())
        .expect("submit second prompt");
    harness
        .wait_for_text(SECOND_PROMPT, Duration::from_secs(15))
        .expect("second prompt rendered");
<<<<<<< HEAD
    harness.update(Duration::from_millis(600));
    harness
=======
    tokio::time::timeout(Duration::from_secs(10), second_turn.wait_blocked())
        .await
        .expect("second turn reaches completion barrier");
    harness.update(Duration::from_millis(600));
    (harness, second_turn)
>>>>>>> 6e386420825bd44ae648c63e7c8cba12fcec9401
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore]
async fn send_page_flips_by_default() {
    let content = ContentController::start().await.expect("start content");
<<<<<<< HEAD
    let mut harness = drive_to_second_send(&content).await;
=======
    let (mut harness, second_turn) = drive_to_second_send(&content).await;
>>>>>>> 6e386420825bd44ae648c63e7c8cba12fcec9401

    assert!(
        !harness.contains_text(TAIL_SENTINEL),
        "default send should page-flip turn 1's tail off screen\nscreen:\n{}",
        harness.screen_contents()
    );
    let screen = harness.screen_contents();
    let prompt_row = screen
        .lines()
        .position(|l| l.contains(SECOND_PROMPT))
        .expect("second prompt visible");
    assert!(
        prompt_row < (DEFAULT_ROWS as usize) / 2,
        "flipped prompt should be in the top half (row {prompt_row})\nscreen:\n{screen}"
    );

<<<<<<< HEAD
    content.release_agent_completions();
=======
    second_turn.release();
>>>>>>> 6e386420825bd44ae648c63e7c8cba12fcec9401
    harness.quit().expect("clean quit");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore]
async fn send_keeps_viewport_when_page_flip_disabled() {
    let content = ContentController::start().await.expect("start content");
    seed_ui_config(&content, "page_flip_on_send = false");
<<<<<<< HEAD
    let mut harness = drive_to_second_send(&content).await;
=======
    let (mut harness, second_turn) = drive_to_second_send(&content).await;
>>>>>>> 6e386420825bd44ae648c63e7c8cba12fcec9401

    assert!(
        harness.contains_text(TAIL_SENTINEL),
        "page_flip_on_send=false must leave turn 1's tail on screen\nscreen:\n{}",
        harness.screen_contents()
    );

<<<<<<< HEAD
    content.release_agent_completions();
=======
    second_turn.release();
>>>>>>> 6e386420825bd44ae648c63e7c8cba12fcec9401
    harness.quit().expect("clean quit");
}
