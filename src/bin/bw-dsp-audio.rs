use std::io::Write;
fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    assert!(
        (1..=2).contains(&args.len()),
        "Usage: bw-dsp-audio input.adp [new-output.wav]"
    );
    let audio = bw_rust::dsp_audio::Audio::decode(&std::fs::read(&args[0]).expect("DSP input"))
        .expect("DSP decode");
    println!(
        "DSP_AUDIO sample_rate={} samples={} duration={} looping={} peak={}",
        audio.sample_rate,
        audio.pcm.len(),
        audio.duration(),
        audio.looping,
        audio
            .pcm
            .iter()
            .map(|v| (*v as i32).abs())
            .max()
            .unwrap_or(0)
    );
    if let Some(path) = args.get(1) {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .expect("New local WAV destination (never overwrite)");
        file.write_all(&audio.wav()).expect("Write WAV");
    }
}
