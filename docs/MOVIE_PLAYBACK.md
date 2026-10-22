# Original cinematic playback backend

Implemented in `src/movie.rs` and exercised by `bw-movie`. This is a **standalone backend check, not the menu launcher or mission flow**.

- Uses installed local `ffprobe`/`ffmpeg` for THP/ADPCM-THP decoding; no emulator or substituted footage.
- Accepts local regular THP files, forces the THP demuxer and restricts protocols to file/pipe. Checks codecs, dimensions, frame rate/count, audio format and duration.
- Streams RGBA frames through a two-frame bounded queue. Exact frame order/count and successful decoder exit are required for completion. Partial frames and early EOF fail.
- Dropping the decoder disconnects the queue, kills/reaps its owned child and joins its reader. It does not inspect or kill unrelated processes.
- Decodes the original stereo soundtrack to bounded PCM/WAV. The Bevy viewer uses AudioSink playback position as its headed clock and fails if audio output does not become ready.
- `BW_OFFSCREEN=1` uses an explicit verification clock and **does not play or verify hardware audio**. Offscreen playback may coalesce due frames; complete per-frame decoding is checked separately.
- The viewer is currently a fixed-layout diagnostic. Frontend mode/asset selection, subtitles, page transitions, resizing and game lifecycle are not integrated.

## Verified original assets

Read-only selected extraction, with H0 verification for accessed clusters:

| Asset | Video frames | Video format | Audio |
|---|---:|---|---|
| BWii_Intro.thp | 323 | 640×480, 30000/1001 fps | stereo 32 kHz |
| 0.1_Prologue_1.THP | 2777 | 640×480, 30000/1001 fps | stereo 32 kHz |

Both decoded through every frame and soundtrack. Both reached their final frame through the Bevy/Metal offscreen viewer. Captures were visually checked. Neither establishes menu-to-prologue sequencing, authored subtitles, hardware listening acceptance, or Windows/Linux execution.

Logs/captures are in `local/analysis/frontend-movie-*` and `local/analysis/prologue-movie-*`; original media stays ignored. `BWii_Intro_w.thp` was also extracted but is not included in the playback acceptance above.

## Run locally

```sh
cargo run --locked --features viewer --bin bw-movie
cargo run --locked --features viewer --bin bw-movie -- local/extracted/files/Data/Video/0.1_Prologue_1.THP
cargo test --locked --test movie -- --include-ignored
```

Requires FFmpeg binaries on PATH. Headed playback plays the original soundtrack and exits at completion; Escape cancels, rather than reporting successful completion. The normal game launcher remains unchanged and still opens the replacement battle.
