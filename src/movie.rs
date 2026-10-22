//! Local THP cinematic decoding through FFmpeg, not emulation or substituted footage.
//! Video is streamed through a bounded queue; the owned decoder is stopped on drop.
use crate::Result;
use serde::Deserialize;
use std::{
    io::{ErrorKind, Read},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver, TryRecvError},
    thread::{self, JoinHandle},
};

#[derive(Debug, Clone)]
pub struct Info {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub frame_rate: f64,
    pub frames: u64,
    pub audio_rate: u32,
    pub audio_channels: u16,
    pub audio_duration: f64,
}
#[derive(Deserialize)]
struct Probe {
    streams: Vec<Stream>,
}
#[derive(Deserialize)]
struct Stream {
    codec_name: String,
    width: Option<u32>,
    height: Option<u32>,
    avg_frame_rate: Option<String>,
    nb_frames: Option<String>,
    sample_rate: Option<String>,
    channels: Option<u16>,
    duration: Option<String>,
}
impl Info {
    pub fn probe(path: &Path) -> Result<Self> {
        let path = path
            .canonicalize()
            .map_err(|e| format!("Movie path: {e}"))?;
        if !path.is_file()
            || !path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("thp"))
        {
            return Err("Only local regular THP movie files supported".into());
        }
        let out = Command::new("ffprobe").args(["-v", "error", "-protocol_whitelist", "file,pipe", "-f", "thp", "-show_entries", "stream=codec_name,width,height,avg_frame_rate,nb_frames,sample_rate,channels,duration", "-of", "json"]).arg(&path).output().map_err(|e| format!("Local ffprobe required: {e}"))?;
        if !out.status.success() || out.stdout.len() > 65536 {
            return Err("THP metadata probe failed".into());
        }
        let probe: Probe =
            serde_json::from_slice(&out.stdout).map_err(|e| format!("Movie metadata: {e}"))?;
        let [v, a] = probe.streams.as_slice() else {
            return Err("Expected one THP video and one audio stream".into());
        };
        if v.codec_name != "thp" || a.codec_name != "adpcm_thp" {
            return Err("Unexpected THP codecs".into());
        }
        let rate = v
            .avg_frame_rate
            .as_deref()
            .ok_or("Missing movie frame rate")?;
        let (n, d) = rate.split_once('/').ok_or("Invalid movie frame rate")?;
        let frame_rate = n.parse::<f64>().map_err(|_| "Invalid rate numerator")?
            / d.parse::<f64>().map_err(|_| "Invalid rate denominator")?;
        let info = Self {
            path,
            width: v.width.ok_or("Missing movie width")?,
            height: v.height.ok_or("Missing movie height")?,
            frame_rate,
            frames: v
                .nb_frames
                .as_deref()
                .ok_or("Missing movie frame count")?
                .parse()
                .map_err(|_| "Invalid frame count")?,
            audio_rate: a
                .sample_rate
                .as_deref()
                .ok_or("Missing audio rate")?
                .parse()
                .map_err(|_| "Invalid audio rate")?,
            audio_channels: a.channels.ok_or("Missing audio channels")?,
            audio_duration: a
                .duration
                .as_deref()
                .ok_or("Missing audio duration")?
                .parse()
                .map_err(|_| "Invalid audio duration")?,
        };
        info.validate()?;
        Ok(info)
    }
    fn validate(&self) -> Result<()> {
        if !self.path.is_file()
            || !self
                .path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("thp"))
        {
            return Err("Only local regular THP files supported".into());
        }
        if self.width == 0
            || self.height == 0
            || self.width > 1920
            || self.height > 1080
            || !self.frame_rate.is_finite()
            || !(1.0..=120.0).contains(&self.frame_rate)
            || self.frames == 0
            || self.frames > 72000
            || self.frames as f64 / self.frame_rate > 600.0
            || !(1..=2).contains(&self.audio_channels)
            || !(8000..=96000).contains(&self.audio_rate)
            || !self.audio_duration.is_finite()
            || !(0.0..=601.0).contains(&self.audio_duration)
        {
            return Err("Unsupported movie dimensions, rate, duration, or audio format".into());
        }
        Ok(())
    }
    pub fn duration(&self) -> f64 {
        (self.frames as f64 / self.frame_rate).max(self.audio_duration)
    }
    pub fn audio_wav(&self) -> Result<Vec<u8>> {
        let pcm = self.decode_audio()?;
        let size = u32::try_from(pcm.len() * 2).map_err(|_| "Movie audio too large")?;
        let mut bytes = Vec::with_capacity(size as usize + 44);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(size + 36).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&self.audio_channels.to_le_bytes());
        bytes.extend_from_slice(&self.audio_rate.to_le_bytes());
        bytes.extend_from_slice(&(self.audio_rate * self.audio_channels as u32 * 2).to_le_bytes());
        bytes.extend_from_slice(&(self.audio_channels * 2).to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&size.to_le_bytes());
        for sample in pcm {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        Ok(bytes)
    }
    /// Native soundtrack, interleaved little-endian PCM. No replacement music.
    pub fn decode_audio(&self) -> Result<Vec<i16>> {
        self.validate()?;
        let mut child = Command::new("ffmpeg")
            .args([
                "-nostdin",
                "-v",
                "error",
                "-threads",
                "1",
                "-protocol_whitelist",
                "file,pipe",
                "-f",
                "thp",
                "-i",
            ])
            .arg(&self.path)
            .args([
                "-map",
                "0:a:0",
                "-vn",
                "-sn",
                "-dn",
                "-f",
                "s16le",
                "-acodec",
                "pcm_s16le",
                "pipe:1",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Local ffmpeg required: {e}"))?;
        let limit = ((self.audio_duration + 1.0)
            * self.audio_rate as f64
            * self.audio_channels as f64
            * 2.0)
            .ceil() as u64;
        let mut bytes = Vec::new();
        let read = child
            .stdout
            .take()
            .ok_or("Missing audio pipe")?
            .take(limit + 1)
            .read_to_end(&mut bytes);
        if read.is_err() || bytes.len() as u64 > limit {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Movie audio read failed or exceeded declared duration".into());
        }
        let status = child.wait().map_err(|e| e.to_string())?;
        if !status.success()
            || bytes.is_empty()
            || !bytes.len().is_multiple_of(2 * self.audio_channels as usize)
        {
            return Err("Movie audio decode failed/truncated".into());
        }
        Ok(bytes
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]))
            .collect())
    }
}
#[derive(Debug)]
pub struct Frame {
    pub index: u64,
    pub rgba: Vec<u8>,
}
enum Packet {
    Frame(Frame),
    End,
    Error(String),
}
pub struct Decoder {
    info: Info,
    child: Child,
    receiver: Option<Receiver<Packet>>,
    reader: Option<JoinHandle<()>>,
    received: u64,
    finished: bool,
}
fn read_frame(reader: &mut impl Read, size: usize) -> Result<Option<Vec<u8>>> {
    let mut bytes = vec![0; size];
    loop {
        match reader.read(&mut bytes[..1]) {
            Ok(0) => return Ok(None),
            Ok(_) => break,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => return Err(format!("Movie frame read failed: {e}")),
        }
    }
    reader
        .read_exact(&mut bytes[1..])
        .map_err(|e| format!("Truncated movie frame: {e}"))?;
    Ok(Some(bytes))
}
impl Decoder {
    pub fn start(info: Info) -> Result<Self> {
        info.validate()?;
        let mut child = Command::new("ffmpeg")
            .args([
                "-nostdin",
                "-v",
                "error",
                "-threads",
                "1",
                "-protocol_whitelist",
                "file,pipe",
                "-f",
                "thp",
                "-i",
            ])
            .arg(&info.path)
            .args([
                "-map",
                "0:v:0",
                "-an",
                "-sn",
                "-dn",
                "-fps_mode",
                "passthrough",
                "-pix_fmt",
                "rgba",
                "-f",
                "rawvideo",
                "pipe:1",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Local ffmpeg required: {e}"))?;
        let mut pipe = child.stdout.take().ok_or("Missing movie video pipe")?;
        let size = info.width as usize * info.height as usize * 4;
        let (tx, rx) = mpsc::sync_channel(2);
        let reader = thread::spawn(move || {
            let mut index = 0;
            loop {
                let packet = match read_frame(&mut pipe, size) {
                    Ok(Some(rgba)) => Packet::Frame(Frame { index, rgba }),
                    Ok(None) => Packet::End,
                    Err(e) => Packet::Error(e),
                };
                let terminal = !matches!(packet, Packet::Frame(_));
                if tx.send(packet).is_err() || terminal {
                    break;
                }
                index += 1;
            }
        });
        Ok(Self {
            info,
            child,
            receiver: Some(rx),
            reader: Some(reader),
            received: 0,
            finished: false,
        })
    }
    pub fn info(&self) -> &Info {
        &self.info
    }
    pub fn finished(&self) -> bool {
        self.finished
    }
    /// Nonblocking; completion requires both exact frame count and decoder success.
    pub fn try_frame(&mut self) -> Result<Option<Frame>> {
        if self.finished {
            return Ok(None);
        }
        match self
            .receiver
            .as_ref()
            .ok_or("Movie decoder closed")?
            .try_recv()
        {
            Ok(Packet::Frame(frame)) => {
                if frame.index != self.received || self.received >= self.info.frames {
                    return Err("Movie frame count/order mismatch".into());
                }
                self.received += 1;
                Ok(Some(frame))
            }
            Ok(Packet::End) => {
                let status = self.child.wait().map_err(|e| e.to_string())?;
                if !status.success() || self.received != self.info.frames {
                    return Err("Movie decode failed or ended early".into());
                }
                self.finished = true;
                Ok(None)
            }
            Ok(Packet::Error(e)) => Err(e),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => {
                Err("Movie decoder disconnected before completion".into())
            }
        }
    }
}
impl Drop for Decoder {
    fn drop(&mut self) {
        self.receiver.take(); // Unblock a reader waiting on the bounded queue first.
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn distinguishes_clean_eof_from_partial_frame() {
        assert!(read_frame(&mut &[][..], 4).unwrap().is_none());
        assert!(read_frame(&mut &[1, 2][..], 4).is_err());
        assert_eq!(
            read_frame(&mut &[1, 2, 3, 4][..], 4).unwrap(),
            Some(vec![1, 2, 3, 4])
        );
    }
}
