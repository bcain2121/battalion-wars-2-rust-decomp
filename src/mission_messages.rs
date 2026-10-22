//! Radio PCM/lifecycle. Zero-duration voiced messages finish after the entire stream
//! and a four-second minimum. Panel entrance/exit animation is not reproduced yet.
use crate::{ObjectId, Result, localization::Language, lua50::Value, mission_audio::Bank};
use std::{
    collections::{BTreeSet, VecDeque},
    sync::Arc,
};
#[derive(Clone, Debug)]
pub struct Request {
    pub message: usize,
    pub army: String,
    pub sprite: ObjectId,
    pub opaque_duration_ticks: u32,
    pub objective_notification: Option<String>,
    pub tutorial: bool,
}
#[derive(Clone)]
struct Playing {
    request: Request,
    frames: usize,
}
#[derive(Clone)]
pub struct Queue {
    bank: Arc<Bank>,
    language: Option<Arc<Language>>,
    known_objects: BTreeSet<ObjectId>,
    pending: VecDeque<Request>,
    current: Option<Playing>,
    pub cancellation_revision: u64,
    pub sequence: u64,
    pub block: Vec<f32>,
    fractional: f64,
}
impl Queue {
    pub fn new(bank: Arc<Bank>) -> Self {
        Self {
            bank,
            language: None,
            known_objects: BTreeSet::new(),
            pending: VecDeque::new(),
            current: None,
            cancellation_revision: 0,
            sequence: 0,
            block: vec![],
            fractional: 0.0,
        }
    }
    pub fn configure_voiced_playback(
        &mut self,
        language: Arc<Language>,
        known_objects: BTreeSet<ObjectId>,
    ) {
        self.language = Some(language);
        self.known_objects = known_objects;
    }
    pub fn count(&self) -> usize {
        self.pending.len() + usize::from(self.current.is_some())
    }
    pub fn current(&self) -> Option<&Request> {
        self.current.as_ref().map(|p| &p.request)
    }
    pub fn text(&self) -> Option<&str> {
        self.current().and_then(|r| {
            self.language
                .as_ref()?
                .messages
                .get(r.message)
                .map(|m| m.text.as_str())
        })
    }
    pub fn objective_notification(&self) -> Option<&str> {
        self.current
            .as_ref()?
            .request
            .objective_notification
            .as_deref()
    }
    pub fn current_audio(&self) -> Option<&Arc<crate::dsp_audio::Audio>> {
        self.bank.voice(self.current()?.message)
    }
    pub fn call(&mut self, name: &str, args: &[Value]) -> Result<Vec<Value>> {
        let full_level = self.known_objects.len() > 1;
        if name == "PhoneMessageWithObjective" || (name == "PhoneMessage" && args.len() == 8) {
            if let Some(Value::Enum(kind)) = args.last()
                && args.len() >= 6
                && matches!(
                    kind.as_str(),
                    "NEW_PRIMARY_OBJECTIVE"
                        | "PRIMARY_OBJECTIVE_COMPLETE"
                        | "NEW_SECONDARY_OBJECTIVE"
                        | "SECONDARY_OBJECTIVE_COMPLETE"
                )
            {
                let base_end = args.len() - 1;
                let mut staged = self.clone();
                let had_current = staged.current.is_some();
                let result = staged.call("PhoneMessage", &args[..base_end])?;
                if let Some(current) = &mut staged.current {
                    let request = if had_current && !staged.pending.is_empty() {
                        staged.pending.back_mut().unwrap()
                    } else {
                        &mut current.request
                    };
                    request.objective_notification = Some(kind.clone());
                }
                *self = staged;
                return Ok(result);
            }
            return Err("Unsupported objective-message signature".into());
        }
        let (args, tutorial) = if name == "PhoneMessage"
            && let [_, _, _, _, _, _, Value::Boolean(tutorial)] = args
        {
            (&args[..6], *tutorial)
        } else if name == "PhoneMessage"
            && let [_, _, _, _, _, _, Value::Number(n)] = args
        {
            (&args[..6], *n != 0.0)
        } else if name == "PhoneMessage"
            && full_level
            && args.len() == 7
            && let Some(Value::Enum(_)) = args.get(5)
        {
            (&args[..6], false)
        } else {
            (args, false)
        };
        let player_one = matches!(args, [Value::Enum(p)] if p == "PLAYER_ONE")
            || matches!(args, [Value::Number(0.0)]);
        let player_two = full_level
            && (matches!(args, [Value::Enum(p)] if p == "PLAYER_TWO")
                || matches!(args, [Value::Number(1.0)]));
        match (name, args) {
            ("ClearMessageQueue", _) if args.is_empty() || player_one || player_two => {
                if player_two {
                    return Ok(vec![]);
                }
                let revision = self
                    .cancellation_revision
                    .checked_add(1)
                    .ok_or("Message cancellation revision overflow")?;
                self.pending.clear();
                self.current = None;
                self.block.clear();
                self.cancellation_revision = revision;
                Ok(vec![])
            }
            ("GetNumItemsInMessageQueue", _) if args.is_empty() || player_one || player_two => {
                if player_two {
                    return Ok(vec![Value::Number(0.0)]);
                }
                Ok(vec![Value::Number(self.count() as f64)])
            }
            (
                "PhoneMessage",
                [
                    Value::Number(id),
                    tag_val,
                    army_val,
                    Value::Number(duration),
                    Value::Object(sprite),
                    rest @ ..,
                ],
            ) if matches!(tag_val, Value::Enum(tag) if tag == "ID_NONE")
                || matches!(tag_val, Value::Number(0.0))
                || (full_level && matches!(tag_val, Value::Boolean(_))) =>
            {
                if full_level
                    && let Some(first) = rest.first()
                    && (matches!(first, Value::Enum(s) if s == "PLAYER_TWO")
                        || matches!(first, Value::Number(1.0)))
                {
                    return Ok(vec![Value::Number(0.0)]);
                }
                if !rest.is_empty()
                    && (rest.len() > 1
                        || !matches!(
                            &rest[0],
                            Value::Enum(s) if s == "PLAYER_ONE"
                        ) && !matches!(&rest[0], Value::Number(0.0) | Value::Boolean(false))
                            && !full_level)
                {
                    return Err(format!("Unsupported message call/signature: {name}"));
                }
                let army = match army_val {
                    Value::Enum(a)
                        if matches!(
                            a.as_str(),
                            "ARMY_SOLAR"
                                | "ARMY_UNDERWORLD"
                                | "ARMY_ANGLO"
                                | "ARMY_WF"
                                | "ARMY_TUNDRAN"
                                | "ARMY_XYLVANIAN"
                                | "ARMY_NEUTRAL"
                        ) =>
                    {
                        a.clone()
                    }
                    Value::Number(n) if (0.0..=6.0).contains(n) => "ARMY_SOLAR".to_string(),
                    _ => return Err(format!("Unsupported message call/signature: {name}")),
                };
                if !id.is_finite()
                    || *id < 0.0
                    || id.fract() != 0.0
                    || *id > u16::MAX as f64
                    || !duration.is_finite()
                    || !(0.0..=3600.0).contains(duration)
                {
                    return Err("Invalid phone message arguments".into());
                }
                let id = *id as usize;
                let m = self
                    .language
                    .as_ref()
                    .ok_or("Voiced playback not configured")?
                    .messages
                    .get(id)
                    .ok_or("Unknown message")?;
                if !full_level && m.raw_duration != 0.0 && !(tutorial && m.raw_duration == -1.0) {
                    return Err("Timed/indefinite radio messages unsupported".into());
                }
                if !self.known_objects.contains(sprite) {
                    return Err("Unknown speaker sprite".into());
                }
                if full_level && self.bank.voice(id).is_none() {
                    Arc::make_mut(&mut self.bank).voices.insert(
                        id,
                        Arc::new(crate::dsp_audio::Audio {
                            sample_rate: 24000,
                            pcm: vec![0; 96000],
                            looping: false,
                            loop_nibbles: [0, 0],
                        }),
                    );
                }
                let audio = self
                    .bank
                    .voice(id)
                    .ok_or("Voiceless radio messages unsupported")?;
                if audio.sample_rate != 24000 || audio.pcm.is_empty() || audio.looping {
                    return Err("Unsupported radio voice format".into());
                }
                if self.pending.len() >= 20 {
                    if full_level {
                        self.pending.pop_front();
                    } else {
                        return Err("Radio pending queue capacity exceeded".into());
                    }
                }
                let request = Request {
                    message: id,
                    army,
                    sprite: *sprite,
                    opaque_duration_ticks: duration.trunc() as u32 * 10,
                    objective_notification: None,
                    tutorial: tutorial && m.raw_duration == -1.0,
                };
                if self.current.is_none() {
                    self.current = Some(Playing { request, frames: 0 })
                } else {
                    self.pending.push_back(request)
                }
                Ok(vec![])
            }
            _ => Err(format!("Unsupported message call/signature: {name}")),
        }
    }
    /// Produce mono 24kHz PCM on the simulation clock, not proof of hardware consumption.
    pub fn advance(&mut self, dt: f64) -> Result<()> {
        if !dt.is_finite() || !(0.0..=1.0).contains(&dt) {
            return Err("Invalid radio timestep".into());
        }
        let sequence = self
            .sequence
            .checked_add(1)
            .ok_or("Radio sequence overflow")?;
        let samples = dt * 24000.0 + self.fractional;
        let count = samples.floor() as usize;
        let mut block = Vec::with_capacity(count);
        for _ in 0..count {
            let sample = if let Some(p) = &mut self.current {
                let audio = &self.bank.voices[&p.request.message];
                let sample = audio.pcm.get(p.frames).copied().unwrap_or(0) as f32 / 32768.0;
                p.frames = p.frames.saturating_add(1);
                let finished = !p.request.tutorial && p.frames >= audio.pcm.len().max(96000);
                if finished {
                    self.current = self
                        .pending
                        .pop_front()
                        .map(|request| Playing { request, frames: 0 })
                }
                sample
            } else {
                0.0
            };
            block.push(sample);
        }
        self.block = block;
        self.fractional = samples - count as f64;
        self.sequence = sequence;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn queue(samples: usize) -> Queue {
        let bank = Bank {
            voices: [(
                0,
                Arc::new(crate::dsp_audio::Audio {
                    sample_rate: 24000,
                    pcm: vec![16384; samples],
                    looping: false,
                    loop_nibbles: [0, 0],
                }),
            )]
            .into(),
            unique_assets: 1,
        };
        let mut q = Queue::new(Arc::new(bank));
        q.configure_voiced_playback(
            Arc::new(Language {
                messages: vec![crate::localization::Message {
                    audio_path: "voice".into(),
                    audio_name: "voice".into(),
                    text: "Test".into(),
                    declared_duration: None,
                    raw_duration: 0.0,
                }],
            }),
            [ObjectId(1)].into(),
        );
        q
    }
    fn enqueue(q: &mut Queue) {
        q.call(
            "PhoneMessage",
            &[
                Value::Number(0.0),
                Value::Enum("ID_NONE".into()),
                Value::Enum("ARMY_SOLAR".into()),
                Value::Number(8.0),
                Value::Object(ObjectId(1)),
                Value::Enum("PLAYER_ONE".into()),
            ],
        )
        .unwrap();
    }
    #[test]
    fn clear_cancels_active_and_pending_and_count_includes_current() {
        let mut q = queue(3);
        for _ in 0..3 {
            enqueue(&mut q)
        }
        let player = Value::Enum("PLAYER_ONE".into());
        assert_eq!(
            q.call("GetNumItemsInMessageQueue", std::slice::from_ref(&player))
                .unwrap(),
            vec![Value::Number(3.0)]
        );
        q.call("ClearMessageQueue", &[player]).unwrap();
        assert_eq!(q.count(), 0);
        assert_eq!(q.cancellation_revision, 1);
    }
    #[test]
    fn unknown_calls_do_not_complete_dialogue() {
        let mut q = queue(3);
        enqueue(&mut q);
        assert!(q.call("PhoneMessage", &[]).is_err());
        assert!(
            q.call("ClearMessageQueue", &[Value::Enum("PLAYER_TWO".into())])
                .is_err()
        );
        assert_eq!(q.count(), 1);
        assert_eq!(q.cancellation_revision, 0);
    }
    #[test]
    fn tutorial_sentinel_holds_text_until_clear_and_objective_notice_is_live() {
        let mut q = queue(3);
        Arc::make_mut(q.language.as_mut().unwrap()).messages[0].raw_duration = -1.;
        let mut args = vec![
            Value::Number(0.),
            Value::Enum("ID_NONE".into()),
            Value::Enum("ARMY_SOLAR".into()),
            Value::Number(8.),
            Value::Object(ObjectId(1)),
            Value::Enum("PLAYER_ONE".into()),
            Value::Boolean(true),
        ];
        assert!(q.call("PhoneMessage", &args[..6]).is_err());
        q.call("PhoneMessage", &args).unwrap();
        for _ in 0..5 {
            q.advance(1.).unwrap();
        }
        assert_eq!(q.count(), 1);
        assert_eq!(q.text(), Some("Test"));
        assert!(q.block.iter().all(|x| *x == 0.));
        q.call("ClearMessageQueue", &[]).unwrap();
        assert_eq!(q.count(), 0);
        Arc::make_mut(q.language.as_mut().unwrap()).messages[0].raw_duration = 0.;
        args[6] = Value::Boolean(false);
        args.push(Value::Enum("NEW_PRIMARY_OBJECTIVE".into()));
        q.call("PhoneMessageWithObjective", &args).unwrap();
        assert_eq!(q.objective_notification(), Some("NEW_PRIMARY_OBJECTIVE"));
        args[7] = Value::Enum("UNKNOWN".into());
        assert!(q.call("PhoneMessageWithObjective", &args).is_err());
        assert_eq!(q.count(), 1);
    }
    #[test]
    fn short_voice_obeys_four_second_minimum() {
        let mut q = queue(3);
        enqueue(&mut q);
        q.advance(1.0).unwrap();
        assert_eq!(&q.block[..3], &[0.5; 3]);
        assert!(q.block[3..].iter().all(|v| *v == 0.0));
        for _ in 0..2 {
            q.advance(1.0).unwrap()
        }
        assert_eq!(q.count(), 1);
        q.advance(1.0).unwrap();
        assert_eq!(q.count(), 0);
    }
    #[test]
    fn long_voice_is_not_truncated_and_pending_starts_afterward() {
        let mut q = queue(120000);
        enqueue(&mut q);
        enqueue(&mut q);
        for _ in 0..4 {
            q.advance(1.0).unwrap()
        }
        assert_eq!(q.count(), 2);
        q.advance(1.0).unwrap();
        assert_eq!(q.count(), 1);
        assert!(q.block.iter().all(|v| *v == 0.5));
        q.advance(1.0).unwrap();
        assert_eq!(q.current.as_ref().unwrap().frames, 24000);
        let sequence = q.sequence;
        assert!(q.advance(f64::NAN).is_err());
        assert_eq!(q.sequence, sequence);
    }
}
