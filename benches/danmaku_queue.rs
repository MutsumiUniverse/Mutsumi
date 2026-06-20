use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use mutsumi::{Color, Danmaku, DanmakuMode, DanmakuQueue};

const DANMAKU_COUNT: usize = 200_000;
const DURATION_MS: f64 = 24.0 * 60.0 * 1000.0;
const FRAME_MS: f64 = 1000.0 / 60.0;

struct DrainDanmakuQueue {
    now_queue: Vec<Danmaku>,
    all_queue: Vec<Danmaku>,
}

impl DrainDanmakuQueue {
    fn new() -> Self {
        Self {
            now_queue: Vec::new(),
            all_queue: Vec::new(),
        }
    }

    fn init(&mut self, danmaku: Vec<Danmaku>, time: f64) {
        self.all_queue = danmaku;
        self.all_queue.sort_by(|a, b| a.start.total_cmp(&b.start));
        self.now_queue = self.all_queue.clone();
        self.pop_to_time(time);
    }

    fn pop_to_time(&mut self, time: f64) -> Vec<Danmaku> {
        let split_index = self
            .now_queue
            .partition_point(|danmaku| danmaku.start <= time);

        self.now_queue.drain(..split_index).collect()
    }
}

struct VecDequeDanmakuQueue {
    now_queue: VecDeque<Danmaku>,
    all_queue: Vec<Danmaku>,
}

impl VecDequeDanmakuQueue {
    fn new() -> Self {
        Self {
            now_queue: VecDeque::new(),
            all_queue: Vec::new(),
        }
    }

    fn init(&mut self, danmaku: Vec<Danmaku>, time: f64) {
        self.all_queue = danmaku;
        self.all_queue.sort_by(|a, b| a.start.total_cmp(&b.start));
        let next_index = self
            .all_queue
            .partition_point(|danmaku| danmaku.start <= time);
        self.now_queue = self.all_queue[next_index..].iter().cloned().collect();
    }

    fn pop_to_time(&mut self, time: f64) -> Vec<Danmaku> {
        let mut danmaku = Vec::new();

        while self
            .now_queue
            .front()
            .is_some_and(|danmaku| danmaku.start <= time)
        {
            if let Some(item) = self.now_queue.pop_front() {
                danmaku.push(item);
            }
        }

        danmaku
    }
}

fn make_danmaku() -> Vec<Danmaku> {
    (0..DANMAKU_COUNT)
        .map(|index| Danmaku {
            content: format!("comment-{index}"),
            start: (index as f64 + 1.0) * DURATION_MS / (DANMAKU_COUNT as f64 + 1.0),
            color: Color::default(),
            mode: DanmakuMode::Scroll,
        })
        .collect()
}

fn run_drain_queue(items: Vec<Danmaku>) -> (usize, Duration) {
    let mut queue = DrainDanmakuQueue::new();
    queue.init(items, 0.0);
    run_frames(|time| count_owned(queue.pop_to_time(time)))
}

fn run_vec_deque_queue(items: Vec<Danmaku>) -> (usize, Duration) {
    let mut queue = VecDequeDanmakuQueue::new();
    queue.init(items, 0.0);
    run_frames(|time| count_owned(queue.pop_to_time(time)))
}

fn run_cursor_queue(items: Vec<Danmaku>) -> (usize, Duration) {
    let mut queue = DanmakuQueue::new();
    queue.init(items, 0.0);
    run_frames(|time| count_ref(queue.pop_to_time_iter(time)))
}

fn count_owned(items: Vec<Danmaku>) -> usize {
    let count = items.len();
    for item in items {
        std::hint::black_box(item.start);
    }
    count
}

fn count_ref<'a>(items: impl Iterator<Item = &'a Danmaku>) -> usize {
    let mut count = 0;
    for item in items {
        std::hint::black_box(item.start);
        count += 1;
    }
    count
}

fn run_frames(mut pop_to_time: impl FnMut(f64) -> usize) -> (usize, Duration) {
    let frames = (DURATION_MS / FRAME_MS).ceil() as usize;
    let started = Instant::now();
    let mut popped = 0;

    for frame in 0..=frames {
        popped += pop_to_time(frame as f64 * FRAME_MS);
    }

    (popped, started.elapsed())
}

fn main() {
    let items = make_danmaku();
    let (drain_popped, drain_elapsed) = run_drain_queue(items.clone());
    let (vec_deque_popped, vec_deque_elapsed) = run_vec_deque_queue(items.clone());
    let (cursor_popped, cursor_elapsed) = run_cursor_queue(items);

    assert_eq!(drain_popped, cursor_popped);
    assert_eq!(drain_popped, vec_deque_popped);

    println!("danmaku_count: {DANMAKU_COUNT}");
    println!("duration_ms: {DURATION_MS}");
    println!("frame_ms: {FRAME_MS}");
    println!("drain_queue: popped={drain_popped} elapsed={drain_elapsed:?}");
    println!("vec_deque_queue: popped={vec_deque_popped} elapsed={vec_deque_elapsed:?}");
    println!("cursor_iter_queue: popped={cursor_popped} elapsed={cursor_elapsed:?}");

    if cursor_elapsed.as_nanos() > 0 {
        println!(
            "cursor_speedup_vs_drain: {:.2}x",
            drain_elapsed.as_secs_f64() / cursor_elapsed.as_secs_f64()
        );
    }
    if vec_deque_elapsed.as_nanos() > 0 {
        println!(
            "vec_deque_speedup_vs_drain: {:.2}x",
            drain_elapsed.as_secs_f64() / vec_deque_elapsed.as_secs_f64()
        );
    }
    if cursor_elapsed.as_nanos() > 0 {
        println!(
            "cursor_speedup_vs_vec_deque: {:.2}x",
            vec_deque_elapsed.as_secs_f64() / cursor_elapsed.as_secs_f64()
        );
    }
}
