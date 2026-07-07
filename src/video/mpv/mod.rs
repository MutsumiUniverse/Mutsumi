mod actor;
mod area;
mod contexted;
mod paintable;
mod proxy;

pub use actor::*;
pub use area::*;
pub use contexted::*;
pub use paintable::*;
pub use proxy::*;

use flume::{Receiver, Sender, unbounded};
use once_cell::sync::Lazy;

type TimeMillis = f64;

pub enum ListenEvent {
    Seek(TimeMillis),
    PlaybackRestart(TimeMillis),
    Eof(u32),
    StartFile,
    Duration(f64),
    Pause(bool),
    CacheSpeed(i64),
    Error(String),
    TrackList(MpvTracks),
    Volume(i64),
    Speed(f64),
    Shutdown,
    DemuxerCacheTime(i64),
    TimePos(i64),
    PausedForCache(bool, TimeMillis),
    ChapterList(ChapterList),
    Playlist(Playlist),
}

pub struct MPVEventChannel {
    pub tx: Sender<ListenEvent>,
    pub rx: Receiver<ListenEvent>,
}

pub static MPV_EVENT_CHANNEL: Lazy<MPVEventChannel> = Lazy::new(|| {
    let (tx, rx) = unbounded::<ListenEvent>();

    MPVEventChannel { tx, rx }
});

pub struct RenderUpdate {
    pub tx: Sender<bool>,
    pub rx: Receiver<bool>,
}

// Give render update a unique channel
pub static RENDER_UPDATE: Lazy<RenderUpdate> = Lazy::new(|| {
    let (tx, rx) = unbounded::<bool>();

    RenderUpdate { tx, rx }
});
