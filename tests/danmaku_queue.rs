use mutsumi::{Color, Danmaku, DanmakuMode, DanmakuQueue};

fn danmaku(start: f64, content: &str) -> Danmaku {
    Danmaku {
        content: content.to_string(),
        start,
        color: Color::default(),
        mode: DanmakuMode::Scroll,
    }
}

fn contents(items: Vec<Danmaku>) -> Vec<String> {
    items.into_iter().map(|item| item.content).collect()
}

fn borrowed_contents<'a>(items: impl Iterator<Item = &'a Danmaku>) -> Vec<String> {
    items.map(|item| item.content.clone()).collect()
}

#[test]
fn init_sorts_and_skips_initial_time() {
    let mut queue = DanmakuQueue::new();
    queue.init(
        vec![
            danmaku(20.0, "third"),
            danmaku(0.0, "first"),
            danmaku(10.0, "second"),
        ],
        5.0,
    );

    assert_eq!(
        contents(queue.pop_to_time(15.0)),
        vec!["second".to_string()]
    );
    assert_eq!(contents(queue.pop_to_time(25.0)), vec!["third".to_string()]);
}

#[test]
fn pop_to_time_does_not_replay_items() {
    let mut queue = DanmakuQueue::new();
    queue.init(vec![danmaku(10.0, "first"), danmaku(20.0, "second")], 0.0);

    assert_eq!(contents(queue.pop_to_time(10.0)), vec!["first".to_string()]);
    assert!(queue.pop_to_time(10.0).is_empty());
    assert_eq!(
        contents(queue.pop_to_time(20.0)),
        vec!["second".to_string()]
    );
}

#[test]
fn pop_to_time_iter_advances_cursor() {
    let mut queue = DanmakuQueue::new();
    queue.init(vec![danmaku(10.0, "first"), danmaku(20.0, "second")], 0.0);

    assert_eq!(
        borrowed_contents(queue.pop_to_time_iter(10.0)),
        vec!["first".to_string()]
    );
    assert!(queue.pop_to_time_iter(10.0).next().is_none());
    assert_eq!(
        borrowed_contents(queue.pop_to_time_iter(20.0)),
        vec!["second".to_string()]
    );
}

#[test]
fn reset_time_rewinds_without_replaying_skipped_items() {
    let mut queue = DanmakuQueue::new();
    queue.init(
        vec![
            danmaku(0.0, "first"),
            danmaku(10.0, "second"),
            danmaku(20.0, "third"),
        ],
        0.0,
    );

    assert_eq!(
        contents(queue.pop_to_time(25.0)),
        vec!["second".to_string(), "third".to_string()]
    );

    queue.reset_time(5.0);

    assert_eq!(
        contents(queue.pop_to_time(25.0)),
        vec!["second".to_string(), "third".to_string()]
    );
}

#[test]
fn reset_time_can_skip_forward() {
    let mut queue = DanmakuQueue::new();
    queue.init(vec![danmaku(10.0, "first"), danmaku(20.0, "second")], 0.0);

    queue.reset_time(15.0);

    assert_eq!(
        contents(queue.pop_to_time(25.0)),
        vec!["second".to_string()]
    );
}
