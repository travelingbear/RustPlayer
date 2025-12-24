use rodio::{Decoder, OutputStream, Sink};
use std::fs::File;
use std::io::BufReader;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

pub struct AudioEngine {
    _stream: OutputStream,
    sink: Arc<Mutex<Sink>>,
    start_time: Arc<Mutex<Option<Instant>>>,
    duration: Arc<Mutex<Option<Duration>>>,
    paused_elapsed: Arc<Mutex<Duration>>,
}

impl AudioEngine {
    pub fn new() -> Result<Self, String> {
        let (stream, handle) = OutputStream::try_default()
            .map_err(|e| format!("Failed to create audio stream: {}", e))?;
        let sink = Sink::try_new(&handle)
            .map_err(|e| format!("Failed to create sink: {}", e))?;
        
        Ok(Self {
            _stream: stream,
            sink: Arc::new(Mutex::new(sink)),
            start_time: Arc::new(Mutex::new(None)),
            duration: Arc::new(Mutex::new(None)),
            paused_elapsed: Arc::new(Mutex::new(Duration::ZERO)),
        })
    }

    pub fn play(&self, path: &str) -> Result<(), String> {
        let duration = Self::get_file_duration(path);
        
        let file = File::open(path)
            .map_err(|e| format!("Failed to open file: {}", e))?;
        let source = Decoder::new(BufReader::new(file))
            .map_err(|e| format!("Failed to decode audio: {}", e))?;
        
        let sink = self.sink.lock().unwrap();
        sink.append(source);
        sink.play();
        drop(sink);
        
        *self.start_time.lock().unwrap() = Some(Instant::now());
        *self.duration.lock().unwrap() = duration;
        *self.paused_elapsed.lock().unwrap() = Duration::ZERO;
        Ok(())
    }

    fn get_file_duration(path: &str) -> Option<Duration> {
        let file = File::open(path).ok()?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        
        let mut hint = Hint::new();
        if let Some(ext) = std::path::Path::new(path).extension() {
            hint.with_extension(ext.to_str()?);
        }
        
        let format_opts = FormatOptions::default();
        let metadata_opts = MetadataOptions::default();
        
        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &format_opts, &metadata_opts)
            .ok()?;
        
        let track = probed.format.default_track()?;
        let time_base = track.codec_params.time_base?;
        let n_frames = track.codec_params.n_frames?;
        
        let seconds = time_base.calc_time(n_frames).seconds;
        Some(Duration::from_secs(seconds))
    }

    pub fn pause(&self) {
        let elapsed = self.get_position();
        *self.paused_elapsed.lock().unwrap() = elapsed;
        *self.start_time.lock().unwrap() = None;
        self.sink.lock().unwrap().pause();
    }

    pub fn resume(&self) {
        *self.start_time.lock().unwrap() = Some(Instant::now());
        self.sink.lock().unwrap().play();
    }

    pub fn is_paused(&self) -> bool {
        self.sink.lock().unwrap().is_paused()
    }

    pub fn stop(&self) {
        self.sink.lock().unwrap().stop();
        *self.start_time.lock().unwrap() = None;
        *self.duration.lock().unwrap() = None;
        *self.paused_elapsed.lock().unwrap() = Duration::ZERO;
    }

    pub fn set_volume(&self, volume: f32) {
        self.sink.lock().unwrap().set_volume(volume);
    }

    pub fn get_position(&self) -> Duration {
        if let Some(start) = *self.start_time.lock().unwrap() {
            *self.paused_elapsed.lock().unwrap() + start.elapsed()
        } else {
            *self.paused_elapsed.lock().unwrap()
        }
    }

    pub fn get_duration(&self) -> Option<Duration> {
        *self.duration.lock().unwrap()
    }

    pub fn is_finished(&self) -> bool {
        self.sink.lock().unwrap().empty()
    }
}
