//! Original STR -> DSP voice bank. Loading media is not a completed dialogue/cinematic.
use crate::{
    Result,
    dsp_audio::Audio,
    localization::{Language, Message},
};
use std::{
    collections::BTreeMap,
    path::{Component, Path, PathBuf},
    sync::Arc,
};
#[derive(Clone)]
pub struct Bank {
    pub voices: BTreeMap<usize, Arc<Audio>>,
    pub unique_assets: usize,
}
fn voice_path(message: &Message) -> Result<Option<PathBuf>> {
    if message.audio_name.is_empty() {
        return Ok(None);
    }
    let path = Path::new(&message.audio_path);
    if message.audio_path.contains('\\')
        || !path.starts_with("Data/Audio/Voiceover")
        || path
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
        || message.audio_name.contains(['/', '\\'])
        || matches!(message.audio_name.as_str(), "." | "..")
    {
        return Err("Unsafe/unsupported STR voice path".into());
    }
    Ok(Some(path.join(format!(
        "{}.adp",
        message.audio_name.to_ascii_lowercase()
    ))))
}
impl Bank {
    pub fn load(root: &Path, language: &Language) -> Result<Self> {
        let root = root.canonicalize().map_err(|e| e.to_string())?;
        let mut cache: BTreeMap<PathBuf, Arc<Audio>> = BTreeMap::new();
        let mut voices = BTreeMap::new();
        for (id, message) in language.messages.iter().enumerate() {
            let Some(relative) = voice_path(message)? else {
                continue;
            };
            let Ok(path) = root.join(relative).canonicalize() else {
                continue;
            };
            if !path.starts_with(&root) {
                return Err("Voice path escapes original asset root".into());
            }
            let audio = if let Some(a) = cache.get(&path) {
                a.clone()
            } else {
                let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
                let a = Arc::new(Audio::decode(&bytes)?);
                if a.looping {
                    return Err("Looping mission dialogue unsupported".into());
                }
                cache.insert(path, a.clone());
                a
            };
            voices.insert(id, audio);
        }
        Ok(Self {
            voices,
            unique_assets: cache.len(),
        })
    }
    pub fn voice(&self, id: usize) -> Option<&Arc<Audio>> {
        self.voices.get(&id)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn voice_paths_are_relative_and_portable() {
        let mut m = Message {
            audio_path: "Data/Audio/Voiceover/CO/Qalen".into(),
            audio_name: "SP_0.1_SE_QALEN_001".into(),
            text: String::new(),
            declared_duration: None,
            raw_duration: 0.0,
        };
        assert_eq!(
            voice_path(&m).unwrap().unwrap(),
            PathBuf::from("Data/Audio/Voiceover/CO/Qalen/sp_0.1_se_qalen_001.adp")
        );
        for p in [
            "/Data/Audio/Voiceover/CO/Qalen",
            "Data/Audio/Voiceover/../secret",
            "Data\\Audio\\Voiceover",
        ] {
            m.audio_path = p.into();
            assert!(voice_path(&m).is_err());
        }
        m.audio_path = "Data/Audio/Voiceover/CO/Qalen".into();
        m.audio_name = "../bad".into();
        assert!(voice_path(&m).is_err());
        m.audio_name.clear();
        assert!(voice_path(&m).unwrap().is_none());
    }
}
