use image::{AnimationDecoder, DynamicImage, ImageDecoder};
use memedock_core::ResourceLimits;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{SyncSender, sync_channel};
use std::thread;
use std::time::{Duration, Instant};

pub const EDGE: u32 = 1280;

#[derive(Clone, Copy)]
pub enum Clip {
    Gif,
    WebP,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayFault {
    Broken,
    Limit,
}

pub enum Event {
    Frame {
        width: i32,
        height: i32,
        bytes: Vec<u8>,
    },
    Failed(PlayFault),
}

pub struct Playback {
    stop: Arc<AtomicBool>,
}

impl Playback {
    pub fn start(path: PathBuf, clip: Clip) -> (Self, std::sync::mpsc::Receiver<Event>) {
        let stop = Arc::new(AtomicBool::new(false));
        let (sender, receiver) = sync_channel(1);
        let worker_stop = Arc::clone(&stop);
        let _thread = thread::Builder::new()
            .name("memedock-playback".to_owned())
            .spawn(move || decode_loop(path, clip, sender, worker_stop));
        (Self { stop }, receiver)
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl Drop for Playback {
    fn drop(&mut self) {
        self.stop();
    }
}

pub fn frame_wait(numer: u32, denom: u32) -> Duration {
    if numer == 0 || denom == 0 {
        return Duration::from_millis(100);
    }
    let millis = u64::from(numer) / u64::from(denom);
    if millis == 0 {
        Duration::from_millis(100)
    } else {
        Duration::from_millis(millis)
    }
}

pub fn fitted_edge(width: u32, height: u32) -> Option<(u32, u32)> {
    if width == 0 || height == 0 {
        return None;
    }
    let long = width.max(height);
    if long <= EDGE {
        return Some((width, height));
    }
    let width = u64::from(width);
    let height = u64::from(height);
    let short = width.min(height) * u64::from(EDGE) / u64::from(long);
    let short = u32::try_from(short).unwrap_or(1).max(1);
    if width >= height {
        Some((EDGE, short))
    } else {
        Some((short, EDGE))
    }
}

enum Cycle {
    Restart,
    Stop,
}

enum Take {
    Ended,
    Failed(PlayFault),
}

struct Shown {
    width: i32,
    height: i32,
    bytes: Vec<u8>,
    wait: Duration,
}

fn decode_loop(path: PathBuf, clip: Clip, sender: SyncSender<Event>, stop: Arc<AtomicBool>) {
    let limits = ResourceLimits::default();
    loop {
        if stop.load(Ordering::Relaxed) {
            return;
        }
        let file = match std::fs::File::open(&path) {
            Ok(file) => file,
            Err(_) => {
                let _ = sender.send(Event::Failed(PlayFault::Broken));
                return;
            }
        };
        let cycle = match clip {
            Clip::Gif => play_gif(file, &limits, &sender, &stop),
            Clip::WebP => play_webp(file, &limits, &sender, &stop),
        };
        match cycle {
            Cycle::Restart => continue,
            Cycle::Stop => return,
        }
    }
}

fn play_gif(
    file: std::fs::File,
    limits: &ResourceLimits,
    sender: &SyncSender<Event>,
    stop: &AtomicBool,
) -> Cycle {
    let decoder = match image::codecs::gif::GifDecoder::new(BufReader::new(file)) {
        Ok(decoder) => decoder,
        Err(_) => {
            let _ = sender.send(Event::Failed(PlayFault::Broken));
            return Cycle::Stop;
        }
    };
    if exceeds(limits, decoder.dimensions()) {
        let _ = sender.send(Event::Failed(PlayFault::Limit));
        return Cycle::Stop;
    }
    let mut frames = decoder.into_frames();
    drive(&mut frames, sender, stop)
}

fn play_webp(
    file: std::fs::File,
    limits: &ResourceLimits,
    sender: &SyncSender<Event>,
    stop: &AtomicBool,
) -> Cycle {
    let decoder = match image::codecs::webp::WebPDecoder::new(BufReader::new(file)) {
        Ok(decoder) => decoder,
        Err(_) => {
            let _ = sender.send(Event::Failed(PlayFault::Broken));
            return Cycle::Stop;
        }
    };
    if !decoder.has_animation() || exceeds(limits, decoder.dimensions()) {
        let _ = sender.send(Event::Failed(if decoder.has_animation() {
            PlayFault::Limit
        } else {
            PlayFault::Broken
        }));
        return Cycle::Stop;
    }
    let mut frames = decoder.into_frames();
    drive(&mut frames, sender, stop)
}

fn exceeds(limits: &ResourceLimits, dimensions: (u32, u32)) -> bool {
    let pixels = u64::from(dimensions.0).saturating_mul(u64::from(dimensions.1));
    pixels == 0
        || pixels > limits.max_frame_pixels
        || pixels.saturating_mul(16) > limits.max_decode_bytes
}

fn drive(
    frames: &mut dyn Iterator<Item = image::ImageResult<image::Frame>>,
    sender: &SyncSender<Event>,
    stop: &AtomicBool,
) -> Cycle {
    let mut upcoming = match take(frames) {
        Ok(frame) => frame,
        Err(Take::Failed(message)) => {
            let _ = sender.send(Event::Failed(message));
            return Cycle::Stop;
        }
        Err(Take::Ended) => {
            let _ = sender.send(Event::Failed(PlayFault::Broken));
            return Cycle::Stop;
        }
    };
    loop {
        if stop.load(Ordering::Relaxed) {
            return Cycle::Stop;
        }
        if sender
            .send(Event::Frame {
                width: upcoming.width,
                height: upcoming.height,
                bytes: upcoming.bytes,
            })
            .is_err()
        {
            return Cycle::Stop;
        }
        let started = Instant::now();
        let following = take(frames);
        if !sleep_remaining(upcoming.wait, started, stop) {
            return Cycle::Stop;
        }
        match following {
            Ok(frame) => upcoming = frame,
            Err(Take::Ended) => return Cycle::Restart,
            Err(Take::Failed(message)) => {
                let _ = sender.send(Event::Failed(message));
                return Cycle::Stop;
            }
        }
    }
}

fn take(frames: &mut dyn Iterator<Item = image::ImageResult<image::Frame>>) -> Result<Shown, Take> {
    let frame = match frames.next() {
        Some(Ok(frame)) => frame,
        Some(Err(_)) => return Err(Take::Failed(PlayFault::Broken)),
        None => return Err(Take::Ended),
    };
    let (numer, denom) = frame.delay().numer_denom_ms();
    let wait = frame_wait(numer, denom);
    let image = DynamicImage::ImageRgba8(frame.into_buffer());
    let Some((width, height)) = fitted_edge(image.width(), image.height()) else {
        return Err(Take::Failed(PlayFault::Broken));
    };
    let image = if width == image.width() && height == image.height() {
        image
    } else {
        image.thumbnail(width, height)
    };
    let rgba = image.into_rgba8();
    let width = i32::try_from(rgba.width()).map_err(|_| Take::Failed(PlayFault::Broken))?;
    let height = i32::try_from(rgba.height()).map_err(|_| Take::Failed(PlayFault::Broken))?;
    Ok(Shown {
        width,
        height,
        bytes: rgba.into_raw(),
        wait,
    })
}

fn sleep_remaining(wait: Duration, started: Instant, stop: &AtomicBool) -> bool {
    let Some(mut left) = wait.checked_sub(started.elapsed()) else {
        return !stop.load(Ordering::Relaxed);
    };
    let slice = Duration::from_millis(50);
    while !left.is_zero() {
        if stop.load(Ordering::Relaxed) {
            return false;
        }
        let step = left.min(slice);
        thread::sleep(step);
        left = left.saturating_sub(step);
    }
    !stop.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::{fitted_edge, frame_wait};
    use std::time::Duration;

    #[test]
    fn a_zero_frame_delay_waits_one_hundred_milliseconds() {
        assert_eq!(frame_wait(0, 1), Duration::from_millis(100));
        assert_eq!(frame_wait(10, 0), Duration::from_millis(100));
        assert_eq!(frame_wait(100, 1), Duration::from_millis(100));
    }

    #[test]
    fn frames_shrink_only_when_the_long_edge_exceeds_1280() {
        assert_eq!(fitted_edge(0, 10), None);
        assert_eq!(fitted_edge(100, 50), Some((100, 50)));
        assert_eq!(fitted_edge(2000, 1000), Some((1280, 640)));
        assert_eq!(fitted_edge(1000, 2000), Some((640, 1280)));
    }
}
