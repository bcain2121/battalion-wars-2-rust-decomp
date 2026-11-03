//! Explicit frontend engine inputs and PowerPC-verified math/globe/video/VO/mission state helpers.
use crate::{
    Result, level_catalog,
    lua50::{Bootstrap, Value},
};
use std::collections::BTreeMap;

const DEG_TO_RAD: f64 = 0.01745329238474369;
const RAD_TO_DEG: f32 = 57.295776;

#[derive(Debug, Clone)]
pub struct Environment {
    counts: BTreeMap<String, usize>,
    level_game_types: Vec<i32>,
    frame_rate: f32,
    controller_errors_disabled: bool,
    pub video_mode: i32,
    pub video_filename: Option<String>,
    pub video_frame_num: u32,
    pub video_total_frames: u32,
    pub vo_directory: Option<String>,
    pub vo_filename: Option<String>,
    pub vo_playing: bool,
    pub current_mission_attempted: bool,
    pub current_mission_completed: bool,
    pub current_fmv_out_seen: bool,
    pub num_global_strings: usize,
    pub global_strings: Vec<String>,
    pub frontend_strings: Vec<String>,
    pub font_kern_factors: BTreeMap<i32, f32>,
    pub screen_ratio: i32,
    pub cursor_position: (f32, f32),
    pub cursor_roll: f32,
    pub cursor_state: i32,
    pub elapsed_time: f32,
    pub rng_state: u32,
    pub levels_done: u32,
    pub levels_attempted_or_done: u32,
    pub selected_level: i32,
    pub selected_slot: u32,
    pub current_level: i32,
    pub current_mp_profile: u32,
    pub profile_names: [String; 4],
    pub profile_undo_buffer: String,
    pub slot_times: [f32; 4],
    pub card_error: i32,
    pub sp_save_valid: bool,
    pub mp_save_valid: bool,
    pub general_purpose_flags: u32,
    pub unlock_flags: u32,
    pub unlock_seen_flags: u32,
    pub mission_status: u32,
    pub mission_stats: BTreeMap<i32, (f32, f32, f32, i32)>,
    pub current_mission_stats: (f32, f32, f32, i32),
    pub music_volume: f32,
    pub sfx_volume: f32,
    pub co_voice_volume: f32,
    pub co_subtitles: bool,
    pub movie_subtitles: bool,
    pub field_toggle: i32,
    pub follow_mode: i32,
    pub tutorial_mode: bool,
    pub homebutton_blocked: bool,
    pub is_client_or_server: u32,
    pub network_status: i32,
    pub tmp_store: f64,
    pub active_widgets: BTreeMap<u64, bool>,
    pub widget_values: BTreeMap<(u64, String), Value>,
    pub launched_level: Option<i32>,
}

impl Environment {
    /// The caller supplies the simulation's frontend rate, not a measured GPU FPS.
    pub fn new(catalog: &level_catalog::Catalog, frame_rate: f32) -> Result<Self> {
        if !frame_rate.is_finite() || !(1.0..4294967296.0).contains(&frame_rate) {
            return Err("Unsupported frontend frame rate".into());
        }
        let mut counts = BTreeMap::new();
        let mut level_game_types = Vec::with_capacity(catalog.entries.len());
        for entry in &catalog.entries {
            *counts.entry(entry.gametype.clone()).or_insert(0) += 1;
            let gt = match entry.gametype.as_str() {
                "frontend" => 0,
                "frontend2" => 1,
                "singleplayer" => 2,
                "skirmish" => 3,
                "assault" => 4,
                "co-op" => 5,
                "mini-game" => 6,
                _ => 7,
            };
            level_game_types.push(gt);
        }
        // Lazy singleton constructor 0x8024c870 writes 1 to the disabled byte.
        // This is a fresh frontend environment, not imported mid-session state.
        Ok(Self {
            counts,
            level_game_types,
            frame_rate,
            controller_errors_disabled: true,
            video_mode: 0,
            video_filename: None,
            video_frame_num: 0,
            video_total_frames: 0,
            vo_directory: None,
            vo_filename: None,
            vo_playing: false,
            current_mission_attempted: false,
            current_mission_completed: false,
            current_fmv_out_seen: false,
            num_global_strings: 778,
            global_strings: Vec::new(),
            frontend_strings: Vec::new(),
            font_kern_factors: BTreeMap::new(),
            screen_ratio: 0,
            cursor_position: (0.0, 0.0),
            cursor_roll: 0.0,
            cursor_state: 0,
            elapsed_time: 0.0,
            rng_state: 0x1234_5678,
            levels_done: 20,
            levels_attempted_or_done: 20,
            selected_level: 1,
            selected_slot: 1,
            current_level: 1,
            current_mp_profile: 1,
            profile_names: [
                String::new(),
                "COMMANDER".to_string(),
                "BATTLER".to_string(),
                String::new(),
            ],
            profile_undo_buffer: "COMMANDER".to_string(),
            slot_times: [0.0, 3600.0, 1800.0, 0.0],
            card_error: 0,
            sp_save_valid: true,
            mp_save_valid: true,
            general_purpose_flags: 0,
            unlock_flags: 0,
            unlock_seen_flags: 0,
            mission_status: 1, // GAMESTATUS.FRONTEND
            mission_stats: BTreeMap::new(),
            current_mission_stats: (95.0, 95.0, 95.0, 5), // GRADE.S = 5
            music_volume: 1.0,
            sfx_volume: 1.0,
            co_voice_volume: 1.0,
            co_subtitles: true,
            movie_subtitles: true,
            field_toggle: 1,
            follow_mode: 0,
            tutorial_mode: true,
            homebutton_blocked: false,
            is_client_or_server: 0,
            network_status: 6, // NETWORK.OK
            tmp_store: 0.0,
            active_widgets: BTreeMap::new(),
            widget_values: BTreeMap::new(),
            launched_level: None,
        })
    }

    pub fn controller_errors_enabled(&self) -> bool {
        !self.controller_errors_disabled
    }

    pub fn set_string_tables(
        &mut self,
        global_strings: Vec<String>,
        frontend_strings: Vec<String>,
    ) {
        if !global_strings.is_empty() {
            self.num_global_strings = global_strings.len();
        }
        self.global_strings = global_strings;
        self.frontend_strings = frontend_strings;
    }

    pub fn lookup_string(&self, id: usize) -> String {
        if id < self.num_global_strings {
            self.global_strings
                .get(id)
                .cloned()
                .unwrap_or_else(|| format!("GSTR_{id}"))
        } else {
            let local = id - self.num_global_strings;
            self.frontend_strings
                .get(local)
                .cloned()
                .unwrap_or_else(|| format!("FESTR_{local}"))
        }
    }

    fn next_random(&mut self) -> f32 {
        self.rng_state = self
            .rng_state
            .wrapping_mul(1664525)
            .wrapping_add(1013904223);
        (self.rng_state >> 8) as f32 / ((1u32 << 24) as f32)
    }

    /// Reads a 4x4 row-major matrix from a 1-indexed 4x4 Lua table (`0x800e4c6c`).
    pub fn read_matrix4(vm: &Bootstrap, table_id: usize) -> Result<[[f32; 4]; 4]> {
        let mut out = [[0.0f32; 4]; 4];
        for (r, row_out) in out.iter_mut().enumerate() {
            let row = vm.table_get(table_id, &Value::Number((r + 1) as f64))?;
            let Value::Table(row_id) = row else {
                return Err(format!("Matrix row {} is not a table", r + 1));
            };
            for (c, cell_out) in row_out.iter_mut().enumerate() {
                let cell = vm.table_get(row_id, &Value::Number((c + 1) as f64))?;
                let Value::Number(val) = cell else {
                    return Err(format!(
                        "Matrix cell [{}, {}] is not a number",
                        r + 1,
                        c + 1
                    ));
                };
                *cell_out = val as f32;
            }
        }
        Ok(out)
    }

    /// Writes a 4x4 row-major matrix into an existing 1-indexed 4x4 Lua table (`0x800e4d78`).
    pub fn write_matrix4(vm: &mut Bootstrap, table_id: usize, mtx: &[[f32; 4]; 4]) -> Result<()> {
        for (r, row_vals) in mtx.iter().enumerate() {
            let row = vm.table_get(table_id, &Value::Number((r + 1) as f64))?;
            let Value::Table(row_id) = row else {
                return Err(format!("Matrix row {} is not a table", r + 1));
            };
            for (c, &val) in row_vals.iter().enumerate() {
                vm.table_set(
                    row_id,
                    Value::Number((c + 1) as f64),
                    Value::Number(val as f64),
                )?;
            }
        }
        Ok(())
    }

    /// PowerPC `0x800e544c`: `GlobeLatLonToOrientationMatrix(mtx_table, lat_deg, lon_deg)`.
    pub fn globe_lat_lon_to_orientation_matrix(lat_deg: f32, lon_deg: f32) -> [[f32; 4]; 4] {
        let lat_rad = (lat_deg as f64 * DEG_TO_RAD) as f32;
        let lon_rad = ((90.0 - lon_deg) as f64 * DEG_TO_RAD) as f32;
        let (s1, c1) = lat_rad.sin_cos();
        let (s2, c2) = lon_rad.sin_cos();
        [
            [c2, s1 * s2, -c1 * s2, 0.0],
            [0.0, c1, s1, 0.0],
            [s2, -s1 * c2, c1 * c2, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]
    }

    /// PowerPC `0x800e6fa8`: `GlobeCalcLatLonFromLocationMtx(mtx)`.
    pub fn globe_calc_lat_lon_from_location_mtx(mtx: &[[f32; 4]; 4]) -> (f32, f32) {
        let lat = -mtx[2][1].clamp(-1.0, 1.0).asin() * RAD_TO_DEG;
        let lon = mtx[2][0].atan2(mtx[2][2]) * RAD_TO_DEG;
        (lat, lon)
    }

    /// PowerPC `0x800e70d0`: `GlobeCalcLatLonFromOrientationMtx(mtx)`.
    pub fn globe_calc_lat_lon_from_orientation_mtx(mtx: &[[f32; 4]; 4]) -> (f32, f32) {
        let lat = -mtx[1][2].clamp(-1.0, 1.0).asin() * RAD_TO_DEG;
        let lon = mtx[0][2].atan2(mtx[2][2]) * RAD_TO_DEG;
        (lat, lon)
    }

    /// PowerPC `0x800e5758`: builds an orthonormal 4x4 matrix from forward vector `f`.
    fn orthonormal_basis_from_forward(f: [f32; 3]) -> [[f32; 4]; 4] {
        let f_len = (f[0] * f[0] + f[1] * f[1] + f[2] * f[2]).sqrt();
        let f = if f_len > 1.0e-5 {
            [f[0] / f_len, f[1] / f_len, f[2] / f_len]
        } else {
            [0.0, 0.0, 1.0]
        };
        // Cross with world up (0, 1, 0): right = up x f = (f.z, 0, -f.x)
        let rx = f[2];
        let rz = -f[0];
        let r_len = (rx * rx + rz * rz).sqrt();
        let right = if r_len > 1.0e-5 {
            [rx / r_len, 0.0, rz / r_len]
        } else {
            [1.0, 0.0, 0.0]
        };
        // up = f x right
        let up = [
            f[1] * right[2] - f[2] * right[1],
            f[2] * right[0] - f[0] * right[2],
            f[0] * right[1] - f[1] * right[0],
        ];
        [
            [right[0], right[1], right[2], 0.0],
            [up[0], up[1], up[2], 0.0],
            [f[0], f[1], f[2], 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]
    }

    fn widget_handle(val: &Value) -> u64 {
        match val {
            Value::Object(id) => id.0,
            Value::Number(n) => *n as u64,
            Value::LightUserdata(h) => *h,
            _ => 0,
        }
    }

    pub fn call(&mut self, name: &str, args: &[Value]) -> Result<Vec<Value>> {
        match (name, args) {
            ("EnableControllerErrorMessages", [value]) => {
                let previous = self.controller_errors_enabled();
                // Lua truthiness: numeric zero and empty strings are true.
                self.controller_errors_disabled =
                    matches!(value, Value::Nil | Value::Boolean(false));
                Ok(vec![Value::Boolean(previous)])
            }
            // Native 0x800df1cc converts the f32 input to an unsigned integer.
            ("GetFramesPerSecond", []) => Ok(vec![Value::Number(self.frame_rate as u32 as f64)]),
            ("GetNumOfGameType", [Value::Number(kind)]) => {
                let xml = match *kind {
                    0.0 => "frontend",
                    1.0 => "frontend2",
                    2.0 => "singleplayer",
                    3.0 => "skirmish",
                    4.0 => "assault",
                    5.0 => "co-op",
                    6.0 => "mini-game",
                    7.0 => "unknown",
                    _ => return Err(format!("Unsupported frontend game type {kind}")),
                };
                Ok(vec![Value::Number(
                    self.counts.get(xml).copied().unwrap_or(0) as f64,
                )])
            }
            // 0x800da8dc: BuildType() -> 2.0 (final release build)
            ("BuildType", []) => Ok(vec![Value::Number(2.0)]),
            // 0x800da910..0x800dab88: degree trig and math helpers
            ("sin", [Value::Number(deg)]) => Ok(vec![Value::Number(
                ((*deg * DEG_TO_RAD) as f32).sin() as f64,
            )]),
            ("cos", [Value::Number(deg)]) => Ok(vec![Value::Number(
                ((*deg * DEG_TO_RAD) as f32).cos() as f64,
            )]),
            ("tan", [Value::Number(deg)]) => Ok(vec![Value::Number(
                ((*deg * DEG_TO_RAD) as f32).tan() as f64,
            )]),
            ("mod", [Value::Number(x), Value::Number(y)]) => Ok(vec![Value::Number(*x % *y)]),
            ("sqrt", [Value::Number(x)]) => Ok(vec![Value::Number((*x as f32).sqrt() as f64)]),
            ("abs", [Value::Number(x)]) => Ok(vec![Value::Number(x.abs())]),
            ("ceil", [Value::Number(x)]) => Ok(vec![Value::Number(x.ceil())]),
            ("floor", [Value::Number(x)]) => Ok(vec![Value::Number(x.floor())]),
            ("rint", [Value::Number(x)]) => Ok(vec![Value::Number((*x as i32) as f64)]),
            // 0x800ddecc: GetNumGlobalStrings()
            ("GetNumGlobalStrings", []) => Ok(vec![Value::Number(self.num_global_strings as f64)]),
            ("GetString" | "GetStringFromTable", [Value::Number(id)]) => {
                let idx = (*id as isize).max(0) as usize;
                Ok(vec![Value::String(self.lookup_string(idx))])
            }
            ("GetString" | "GetStringFromTable", [Value::String(s)]) => {
                Ok(vec![Value::String(s.clone())])
            }
            ("FormatString", [fmt, rest @ ..]) => {
                let mut out = match fmt {
                    Value::String(s) => s.clone(),
                    Value::Number(id) => self.lookup_string((*id as isize).max(0) as usize),
                    _ => String::new(),
                };
                for arg in rest {
                    let rep = match arg {
                        Value::String(s) => s.clone(),
                        Value::Number(n) => {
                            if n.fract() == 0.0 {
                                format!("{}", *n as i64)
                            } else {
                                format!("{n}")
                            }
                        }
                        Value::Boolean(b) => b.to_string(),
                        _ => String::new(),
                    };
                    if let Some(pos) = out.find("%s").or_else(|| out.find("%d")) {
                        out.replace_range(pos..pos + 2, &rep);
                    } else {
                        out.push_str(&rep);
                    }
                }
                Ok(vec![Value::String(out)])
            }
            ("GetCharFromString", [Value::String(s), Value::Number(idx)]) => {
                let i = (*idx as isize).max(0) as usize;
                let ch = s
                    .chars()
                    .nth(i)
                    .or_else(|| i.checked_sub(1).and_then(|j| s.chars().nth(j)))
                    .map(|c| c.to_string())
                    .unwrap_or_default();
                Ok(vec![Value::String(ch)])
            }
            ("GetLanguageStr", []) => Ok(vec![Value::String("English".into())]),
            ("GetRegionStr", []) => Ok(vec![Value::String("US".into())]),
            ("GetVersionStr", []) => Ok(vec![Value::String("1.0".into())]),
            // 0x800de298: SetFontKernFactor(font_id, factor)
            ("SetFontKernFactor", [Value::Number(id), Value::Number(factor)]) => {
                self.font_kern_factors.insert(*id as i32, *factor as f32);
                Ok(vec![])
            }
            // 0x800dd6a8: GetScreenRatio()
            ("GetScreenRatio", []) => Ok(vec![Value::Number(self.screen_ratio as f64)]),
            ("GetCursorPosition", []) => Ok(vec![
                Value::Number(self.cursor_position.0 as f64),
                Value::Number(self.cursor_position.1 as f64),
            ]),
            ("GetCursorRoll", []) => Ok(vec![Value::Number(self.cursor_roll as f64)]),
            ("SetCursorState", [Value::Number(state)]) => {
                self.cursor_state = *state as i32;
                Ok(vec![])
            }
            ("GetTime", []) => {
                self.elapsed_time += 1.0 / self.frame_rate.max(1.0);
                Ok(vec![Value::Number(self.elapsed_time as f64)])
            }
            ("GetRandom", [Value::Number(max)]) => {
                let r = (self.next_random() * (*max as f32)).floor();
                Ok(vec![Value::Number(r as f64)])
            }
            ("GetRandom", [Value::Number(min), Value::Number(max)]) => {
                let span = (*max - *min).max(0.0) as f32;
                let r = *min as f32 + (self.next_random() * span).floor();
                Ok(vec![Value::Number(r as f64)])
            }
            // 0x800d5654..0x800d6d94: THP Video playback
            ("PlayVideo", [Value::Number(mode), rest @ ..]) => {
                self.video_mode = *mode as i32;
                self.video_frame_num = 0;
                if self.video_mode == 0 {
                    self.video_filename = None;
                    self.video_total_frames = 0;
                } else if let Some(Value::String(name)) = rest.first() {
                    self.video_filename = Some(name.clone());
                    if self.video_total_frames == 0 {
                        self.video_total_frames = 60;
                    }
                }
                Ok(vec![])
            }
            ("IsVideoFinished", []) => Ok(vec![Value::Boolean(self.video_mode == 0)]),
            ("GetTotalFrames", []) => Ok(vec![Value::Number(self.video_total_frames as f64)]),
            ("GetFrameNum", []) => Ok(vec![Value::Number(self.video_frame_num as f64)]),
            // 0x800eadf0..0x800d6f84: Briefing voiceover (ADP) playback
            (
                "StartVO",
                [
                    Value::Number(_chan),
                    Value::String(dir),
                    Value::String(file),
                    ..,
                ],
            ) => {
                self.vo_directory = Some(dir.clone());
                self.vo_filename = Some(file.clone());
                self.vo_playing = true;
                Ok(vec![Value::Boolean(true)])
            }
            ("StopVO", []) => {
                self.vo_playing = false;
                Ok(vec![])
            }
            ("CheckVOFinished", []) => Ok(vec![Value::Boolean(!self.vo_playing)]),
            // 0x800def60..0x800df160: Mission attempted/completed/FMV flags
            ("SetCurrentMissionAttempted", [val]) => {
                self.current_mission_attempted = !matches!(val, Value::Nil | Value::Boolean(false));
                Ok(vec![])
            }
            ("GetCurrentMissionAttempted", []) => {
                Ok(vec![Value::Boolean(self.current_mission_attempted)])
            }
            ("SetCurrentMissionCompleted", [val]) => {
                self.current_mission_completed = !matches!(val, Value::Nil | Value::Boolean(false));
                Ok(vec![])
            }
            ("GetCurrentMissionCompleted", []) => {
                Ok(vec![Value::Boolean(self.current_mission_completed)])
            }
            ("SetCurrentFMVOutSeen", []) => {
                self.current_fmv_out_seen = true;
                Ok(vec![])
            }
            ("SetCurrentFMVOutSeen", [val]) => {
                self.current_fmv_out_seen = !matches!(val, Value::Nil | Value::Boolean(false));
                Ok(vec![])
            }
            ("GetCurrentFMVOutSeen", []) => Ok(vec![Value::Boolean(self.current_fmv_out_seen)]),
            // Persistent campaign & save slot state
            ("GetPersistentData", []) => Ok(vec![
                Value::Number(self.levels_done as f64),
                Value::Number(self.selected_level as f64),
                Value::Number(self.selected_slot as f64),
            ]),
            ("GetLevelsDone", []) => Ok(vec![Value::Number(self.levels_done as f64)]),
            ("SetLevelsDone", [Value::Number(n)]) => {
                self.levels_done = (*n as i32).max(0) as u32;
                self.levels_attempted_or_done = self.levels_attempted_or_done.max(self.levels_done);
                Ok(vec![])
            }
            ("GetLevelsAttemptedOrDone", []) => {
                Ok(vec![Value::Number(self.levels_attempted_or_done as f64)])
            }
            ("RegisterMissionAttempt", [Value::Number(level)]) => {
                let lvl = (*level as i32).max(0) as u32;
                self.levels_attempted_or_done = self.levels_attempted_or_done.max(lvl);
                self.current_mission_attempted = true;
                Ok(vec![])
            }
            ("SetSelectedSlot", [Value::Number(slot)]) => {
                self.selected_slot = (*slot as i32).clamp(1, 3) as u32;
                Ok(vec![])
            }
            ("StartNewSlot", []) => {
                let idx = (self.selected_slot as usize).min(3);
                self.profile_names[idx] = "NEW COMMANDER".to_string();
                self.levels_done = 0;
                self.levels_attempted_or_done = 0;
                Ok(vec![])
            }
            ("GetProfileName", [Value::Number(slot)]) => {
                let idx = (*slot as isize).clamp(0, 3) as usize;
                Ok(vec![Value::String(self.profile_names[idx].clone())])
            }
            ("SetProfileName", [Value::String(name)]) => {
                let idx = (self.selected_slot as usize).min(3);
                self.profile_names[idx] = name.clone();
                Ok(vec![])
            }
            ("ClearProfile", [Value::Number(slot)]) => {
                let idx = (*slot as isize).clamp(0, 3) as usize;
                self.profile_names[idx].clear();
                self.slot_times[idx] = 0.0;
                Ok(vec![])
            }
            ("SaveProfileToUndoBuffer", []) => {
                let idx = (self.selected_slot as usize).min(3);
                self.profile_undo_buffer = self.profile_names[idx].clone();
                Ok(vec![])
            }
            ("RestoreProfileFromUndoBuffer", []) => {
                let idx = (self.selected_slot as usize).min(3);
                self.profile_names[idx] = self.profile_undo_buffer.clone();
                Ok(vec![])
            }
            ("GetProfileNameCheckResult", []) => Ok(vec![Value::Number(0.0)]),
            ("GetSlotTime", [Value::Number(slot_zero_based)]) => {
                let idx = (*slot_zero_based as isize + 1).clamp(0, 3) as usize;
                Ok(vec![Value::Number(self.slot_times[idx] as f64)])
            }
            ("GetCardError", []) => Ok(vec![Value::Number(self.card_error as f64)]),
            ("ClearCardError", []) => {
                self.card_error = 0;
                Ok(vec![])
            }
            ("CheckSaveFile" | "CreateSaveFile" | "WriteSaveFile" | "DeleteSaveFile", []) => {
                self.card_error = 0;
                Ok(vec![])
            }
            ("CheckNANDResources", [_, _]) => {
                self.card_error = 0;
                Ok(vec![])
            }
            ("GetGameSaveValid", []) => Ok(vec![Value::Boolean(self.sp_save_valid)]),
            ("GetGameSaveValid", [kind]) => {
                let valid = if matches!(kind, Value::Number(n) if (*n as i32) == 2) {
                    self.mp_save_valid
                } else {
                    self.sp_save_valid
                };
                Ok(vec![Value::Boolean(valid)])
            }
            ("SetGameSaveValid", [val]) => {
                self.sp_save_valid = !matches!(val, Value::Nil | Value::Boolean(false));
                Ok(vec![])
            }
            ("SetGameSaveValid", [kind, val]) => {
                let valid = !matches!(val, Value::Nil | Value::Boolean(false));
                if matches!(kind, Value::Number(n) if (*n as i32) == 2) {
                    self.mp_save_valid = valid;
                } else {
                    self.sp_save_valid = valid;
                }
                Ok(vec![])
            }
            ("GetGeneralPurposeFlags", []) => {
                Ok(vec![Value::Number(self.general_purpose_flags as f64)])
            }
            ("SetGeneralPurposeFlags", [Value::Number(flags)]) => {
                self.general_purpose_flags = *flags as u32;
                Ok(vec![])
            }
            ("GetUnlockFlags", []) => Ok(vec![Value::Number(self.unlock_flags as f64)]),
            ("SetUnlockFlags", [Value::Number(flags)]) => {
                self.unlock_flags = *flags as u32;
                Ok(vec![])
            }
            ("GetUnlockSeenFlags", []) => Ok(vec![Value::Number(self.unlock_seen_flags as f64)]),
            ("SetUnlockSeenFlags", [Value::Number(flags)]) => {
                self.unlock_seen_flags = *flags as u32;
                Ok(vec![])
            }
            ("AddUnlockFlag", [Value::Number(flags), Value::Number(bit)]) => {
                let bit_idx = (*bit as u32).min(31);
                let next = (*flags as u32) | (1u32 << bit_idx);
                Ok(vec![Value::Number(next as f64)])
            }
            ("RemoveUnlockFlag", [Value::Number(flags), Value::Number(bit)]) => {
                let bit_idx = (*bit as u32).min(31);
                let next = (*flags as u32) & !(1u32 << bit_idx);
                Ok(vec![Value::Number(next as f64)])
            }
            ("TestUnlockFlag", [Value::Number(flags), Value::Number(bit)]) => {
                let bit_idx = (*bit as u32).min(31);
                let set = ((*flags as u32) & (1u32 << bit_idx)) != 0;
                Ok(vec![Value::Boolean(set)])
            }
            ("TestAnyUnlockFlag", [Value::Number(flags)]) => {
                Ok(vec![Value::Boolean((*flags as u32) != 0)])
            }
            ("GetCurrentLevel", []) => Ok(vec![Value::Number(self.current_level as f64)]),
            ("SetCurrentLevel", [Value::Number(level)]) => {
                self.current_level = *level as i32;
                Ok(vec![])
            }
            ("GetLevelGameType", [Value::Number(level)]) => {
                let idx = (*level as isize).max(0) as usize;
                let gt = self.level_game_types.get(idx).copied().unwrap_or(2);
                Ok(vec![Value::Number(gt as f64)])
            }
            ("GetMissionStatus", []) => Ok(vec![Value::Number(self.mission_status as f64)]),
            ("SetMissionStatus", [Value::Number(status)]) => {
                self.mission_status = *status as u32;
                Ok(vec![])
            }
            ("TestMissionStatus", [Value::Number(mask)]) => {
                let m = *mask as u32;
                let matched = if m == 0 {
                    self.mission_status == 0
                } else {
                    (self.mission_status & m) != 0
                };
                Ok(vec![Value::Boolean(matched)])
            }
            ("GetMissionData", []) => {
                let (s, p, t, g) = self.current_mission_stats;
                Ok(vec![
                    Value::Number(s as f64),
                    Value::Number(p as f64),
                    Value::Number(t as f64),
                    Value::Number(g as f64),
                ])
            }
            ("GetStoredMissionData", [Value::Number(level)]) => {
                let (s, p, t, g) = self
                    .mission_stats
                    .get(&(*level as i32))
                    .copied()
                    .unwrap_or((90.0, 90.0, 90.0, 4));
                Ok(vec![
                    Value::Number(s as f64),
                    Value::Number(p as f64),
                    Value::Number(t as f64),
                    Value::Number(g as f64),
                ])
            }
            ("SetCurrentMissionStats", [Value::Number(grade)]) => {
                self.current_mission_stats.3 = *grade as i32;
                self.mission_stats
                    .insert(self.selected_level, self.current_mission_stats);
                Ok(vec![])
            }
            ("LoadCurrentMissionStats", [Value::Number(level)]) => {
                self.selected_level = *level as i32;
                Ok(vec![])
            }
            (
                "SetMissionComplete",
                [
                    Value::Number(level),
                    Value::Number(speed),
                    Value::Number(power),
                    Value::Number(tech),
                    Value::Number(grade),
                ],
            ) => {
                let entry = (*speed as f32, *power as f32, *tech as f32, *grade as i32);
                self.current_mission_stats = entry;
                self.mission_stats.insert(*level as i32, entry);
                Ok(vec![])
            }
            ("GetObjectivesResult", []) => Ok(vec![
                Value::Number(1.0),
                Value::Number(1.0),
                Value::Number(1.0),
            ]),
            ("TriggerFrameSave" | "BlockHomebutton" | "UnblockHomebutton" | "ResetToIPL", []) => {
                self.homebutton_blocked = name == "BlockHomebutton";
                Ok(vec![])
            }
            // Audio & settings natives
            ("StartMusic", [..]) => Ok(vec![Value::Boolean(true)]),
            ("StopMusic", []) => Ok(vec![]),
            ("CheckMusicFinished", []) => Ok(vec![Value::Boolean(false)]),
            ("SetMusicVolume" | "SetMusicVolumeS", [Value::Number(v), ..]) => {
                self.music_volume = *v as f32;
                Ok(vec![])
            }
            ("GetMusicVolume", []) => Ok(vec![Value::Number(self.music_volume as f64)]),
            ("SetSFXVolume", [Value::Number(v)]) => {
                self.sfx_volume = *v as f32;
                Ok(vec![])
            }
            ("GetSFXVolume", []) => Ok(vec![Value::Number(self.sfx_volume as f64)]),
            ("SetCOVoiceVolume", [Value::Number(v)]) => {
                self.co_voice_volume = *v as f32;
                Ok(vec![])
            }
            ("GetCOVoiceVolume", []) => Ok(vec![Value::Number(self.co_voice_volume as f64)]),
            ("SetCOSubtitles", [val]) => {
                self.co_subtitles =
                    !matches!(val, Value::Nil | Value::Boolean(false) | Value::Number(0.0));
                Ok(vec![])
            }
            ("GetCOSubtitles", []) => Ok(vec![Value::Number(u8::from(self.co_subtitles) as f64)]),
            ("SetMovieSubtitles", [val]) => {
                self.movie_subtitles =
                    !matches!(val, Value::Nil | Value::Boolean(false) | Value::Number(0.0));
                Ok(vec![])
            }
            ("GetMovieSubtitles", []) => {
                Ok(vec![Value::Number(u8::from(self.movie_subtitles) as f64)])
            }
            ("SetFieldToggle", [Value::Number(v)]) => {
                self.field_toggle = *v as i32;
                Ok(vec![])
            }
            ("GetFieldToggle", []) => Ok(vec![Value::Number(self.field_toggle as f64)]),
            ("SetFollowMode", [Value::Number(v)]) => {
                self.follow_mode = *v as i32;
                Ok(vec![])
            }
            ("GetFollowMode", []) => Ok(vec![Value::Number(self.follow_mode as f64)]),
            ("SetTutorialMode", [val]) => {
                self.tutorial_mode =
                    !matches!(val, Value::Nil | Value::Boolean(false) | Value::Number(0.0));
                Ok(vec![])
            }
            ("GetTutorialMode", []) => Ok(vec![Value::Number(u8::from(self.tutorial_mode) as f64)]),
            ("InitSettings" | "ApplySettings" | "EndSettings", []) => Ok(vec![]),
            ("PlaySound", [_]) => Ok(vec![]),
            ("PlayEffect", [_, _, _, _, _]) => Ok(vec![]),
            // Widget state helpers
            ("StartWidget", [_, widget, _]) => {
                let h = Self::widget_handle(widget);
                if h != 0 {
                    self.active_widgets.insert(h, true);
                }
                Ok(vec![])
            }
            ("StopWidget", [widget]) => {
                let h = Self::widget_handle(widget);
                if h != 0 {
                    self.active_widgets.insert(h, false);
                }
                Ok(vec![])
            }
            ("GetWidgetValue", [widget, Value::String(field)]) => {
                let h = Self::widget_handle(widget);
                let v = self
                    .widget_values
                    .get(&(h, field.clone()))
                    .cloned()
                    .unwrap_or(Value::Number(1.0));
                Ok(vec![v])
            }
            ("SetWidgetValue", [widget, Value::String(field), val]) => {
                let h = Self::widget_handle(widget);
                self.widget_values.insert((h, field.clone()), val.clone());
                Ok(vec![])
            }
            ("SetListBoxData", [..]) => Ok(vec![]),
            // Multiplayer & network state helpers
            ("GetCurrentMPProfile", []) => Ok(vec![Value::Number(self.current_mp_profile as f64)]),
            ("SetCurrentMPProfile", [Value::Number(p)]) => {
                self.current_mp_profile = (*p as i32).max(1) as u32;
                Ok(vec![])
            }
            ("GetIsClientOrServer", []) => Ok(vec![Value::Number(self.is_client_or_server as f64)]),
            ("SetIsClientOrServer", [Value::Number(v)]) => {
                self.is_client_or_server = *v as u32;
                Ok(vec![])
            }
            ("GetNetworkStatus", []) => Ok(vec![Value::Number(self.network_status as f64)]),
            ("GetNetworkErrorCode", []) => Ok(vec![Value::Number(0.0)]),
            ("RequestNetworkOpen", [_]) => {
                self.network_status = 6; // NETWORK.OK
                Ok(vec![])
            }
            ("RequestNetworkClose", []) => Ok(vec![]),
            (
                "RequestLoadMultiPlayer"
                | "RequestCreateMultiPlayer"
                | "RequestSaveMultiPlayer"
                | "RequestDeleteMultiPlayer"
                | "DontSaveMultiPlayer",
                [],
            ) => {
                self.card_error = 0;
                Ok(vec![])
            }
            ("MultiPlayerNeedsSaving", []) => Ok(vec![Value::Boolean(false)]),
            ("GetMPPageFromCode", []) => Ok(vec![Value::String("Play".into())]),
            ("GetMPStatistics", [_, _, _]) => Ok(vec![Value::Number(0.0)]),
            ("UpdateMPStatistics", []) => Ok(vec![]),
            ("UpdateMPUnlockStatus", [_]) => Ok(vec![Value::Boolean(false)]),
            ("GetOwnRank", [..]) => Ok(vec![Value::Number(1.0)]),
            ("GetRankIncreased" | "GetRankDecreased", [_]) => Ok(vec![Value::Boolean(false)]),
            ("ResetRankChange", [_]) => Ok(vec![]),
            ("GetRankFromDiff", [_]) => Ok(vec![Value::Number(1.0)]),
            ("GetSkill", []) => Ok(vec![Value::Number(1000.0)]),
            ("GetSkillUncertainty", []) => Ok(vec![Value::Number(100.0)]),
            ("GetFriendKey", [..]) => Ok(vec![Value::String("0000-0000-0000".into())]),
            ("GetFriendName", [_]) => Ok(vec![Value::String(String::new())]),
            ("SetFriendName" | "SetFriendRank", [_, _]) => Ok(vec![]),
            ("GetFriendRank", [_]) => Ok(vec![Value::Number(1.0)]),
            ("GetFriendStatusCode", [..]) => Ok(vec![Value::Number(0.0)]),
            ("GetFriendStatusStringTag", [..]) => Ok(vec![Value::String(String::new())]),
            ("SetOwnStatusStringTag" | "RemoveOwnStatusStringTag", [..]) => Ok(vec![]),
            ("AddFriend", [..]) => Ok(vec![Value::Boolean(true)]),
            ("DeleteFriend", [_]) => Ok(vec![]),
            ("IsOkToChangeFriends", []) => Ok(vec![Value::Boolean(true)]),
            ("StartMatchMaking", [..]) => Ok(vec![Value::Boolean(true)]),
            ("StopMatchMaking" | "startSearching" | "LeaveMPGame", []) => Ok(vec![]),
            ("CheckMatchMaking", []) => Ok(vec![Value::Number(0.0)]),
            ("IsNetworkPlayer", [Value::Number(idx)]) => Ok(vec![Value::Boolean(*idx == 0.0)]),
            ("GetPlayerInformationReady", [_]) => Ok(vec![Value::Boolean(true)]),
            ("GetPlayerInformation", [Value::Number(idx)]) => {
                let p = *idx as i32;
                let name = if p == 1 { "COMMANDER" } else { "OPPONENT" };
                let army = if p == 1 { 1.0 } else { 2.0 };
                Ok(vec![
                    Value::String(name.into()),
                    Value::Number(army),
                    Value::Number(1.0),
                    Value::Number(0.0),
                    Value::Number(0.0),
                    Value::Number(0.0),
                    Value::Number(0.0),
                    Value::Number(1000.0),
                    Value::Number(100.0),
                ])
            }
            ("GetFriendlyArmy", []) => Ok(vec![Value::Number(1.0)]),
            ("GetEnemyArmy", []) => Ok(vec![Value::Number(2.0)]),
            ("GetFriendlyScoreTotals", []) => Ok(vec![Value::Number(100.0)]),
            ("GetEnemyScoreTotals", []) => Ok(vec![Value::Number(0.0)]),
            ("GetTmpStore", []) => Ok(vec![Value::Number(self.tmp_store)]),
            ("SetTmpStore", [Value::Number(v)]) => {
                self.tmp_store = *v;
                Ok(vec![])
            }
            ("UpdateRematch", [_]) => Ok(vec![Value::Number(1.0)]),
            ("IQuit" | "OtherPlayerQuit", []) => Ok(vec![Value::Boolean(false)]),
            ("WaitFor", [_]) => Ok(vec![]),
            // 0x800e50ec: RotationToVector(rx_deg, ry_deg, rz_deg)
            ("RotationToVector", [Value::Number(rx), Value::Number(ry), Value::Number(rz)]) => {
                let (sx, cx) = ((*rx * DEG_TO_RAD) as f32).sin_cos();
                let (sy, cy) = ((*ry * DEG_TO_RAD) as f32).sin_cos();
                let (sz, cz) = ((*rz * DEG_TO_RAD) as f32).sin_cos();
                let vx = sx * sy * sz + cx * cz;
                let vy = cx * sy * sz - sx * cz;
                let vz = cy * sz;
                Ok(vec![
                    Value::Number(vx as f64),
                    Value::Number(vy as f64),
                    Value::Number(vz as f64),
                ])
            }
            // 0x800e58b4: CalcGlobeInterection(sx, sy, gx, gy, gz, radius)
            (
                "CalcGlobeInterection",
                [
                    Value::Number(sx),
                    Value::Number(sy),
                    Value::Number(gx),
                    Value::Number(gy),
                    Value::Number(gz),
                    Value::Number(radius),
                ],
            ) => {
                let (sx, sy, gx, gy, gz, r) = (
                    *sx as f32,
                    *sy as f32,
                    *gx as f32,
                    *gy as f32,
                    *gz as f32,
                    *radius as f32,
                );
                let dx = sx - gx;
                let dy = sy - gy;
                let dist_sq = dx * dx + dy * dy;
                if dist_sq <= r * r {
                    let dz = (r * r - dist_sq).sqrt();
                    Ok(vec![
                        Value::Number((gx + dx) as f64),
                        Value::Number((gy + dy) as f64),
                        Value::Number((gz - dz) as f64),
                    ])
                } else {
                    Ok(vec![
                        Value::Number(0.0),
                        Value::Number(0.0),
                        Value::Number(0.0),
                    ])
                }
            }
            // 0x800e6ab0: GlobePickRegionFromMap(lon_deg, lat_norm)
            ("GlobePickRegionFromMap", [Value::Number(lon), Value::Number(lat_norm)]) => {
                let lon = (*lon as f32).rem_euclid(360.0);
                let lat = *lat_norm as f32 * 90.0;
                // Authored continent centers in GlobeSelect (1..=5)
                let centers: [(i32, f32, f32); 5] = [
                    (1, 17.0, 2.0),
                    (2, -10.0, 96.0),
                    (3, 32.0, 232.0),
                    (4, -11.0, 279.7),
                    (5, -13.0, 48.5),
                ];
                let mut best_id = 0;
                let mut best_dist = 35.0f32 * 35.0f32;
                for (id, c_lat, c_lon) in centers {
                    let d_lat = lat - c_lat;
                    let mut d_lon = (lon - c_lon).rem_euclid(360.0);
                    if d_lon > 180.0 {
                        d_lon -= 360.0;
                    }
                    let d2 = d_lat * d_lat + d_lon * d_lon;
                    if d2 < best_dist {
                        best_dist = d2;
                        best_id = id;
                    }
                }
                Ok(vec![Value::Number(best_id as f64)])
            }
            _ => Err(format!("Unsupported frontend call/signature: {name}")),
        }
    }

    pub fn call_with_vm(
        &mut self,
        name: &str,
        args: &[Value],
        vm: &mut Bootstrap,
    ) -> Result<Vec<Value>> {
        match (name, args) {
            // 0x800e4e84: MatrixMultiply(a_mtx, b_mtx, out_mtx)
            ("MatrixMultiply", [Value::Table(a_id), Value::Table(b_id), Value::Table(out_id)]) => {
                let a = Self::read_matrix4(vm, *a_id)?;
                let b = Self::read_matrix4(vm, *b_id)?;
                let mut out = [[0.0f32; 4]; 4];
                for r in 0..4 {
                    for c in 0..4 {
                        out[r][c] = a[r][0] * b[0][c]
                            + a[r][1] * b[1][c]
                            + a[r][2] * b[2][c]
                            + a[r][3] * b[3][c];
                    }
                }
                Self::write_matrix4(vm, *out_id, &out)?;
                Ok(vec![])
            }
            // 0x800e544c: GlobeLatLonToOrientationMatrix(mtx, lat, lon)
            (
                "GlobeLatLonToOrientationMatrix",
                [Value::Table(mtx_id), Value::Number(lat), Value::Number(lon)],
            ) => {
                let mtx = Self::globe_lat_lon_to_orientation_matrix(*lat as f32, *lon as f32);
                Self::write_matrix4(vm, *mtx_id, &mtx)?;
                Ok(vec![])
            }
            // 0x800e6fa8: GlobeCalcLatLonFromLocationMtx(mtx)
            ("GlobeCalcLatLonFromLocationMtx", [Value::Table(mtx_id)]) => {
                let mtx = Self::read_matrix4(vm, *mtx_id)?;
                let (lat, lon) = Self::globe_calc_lat_lon_from_location_mtx(&mtx);
                Ok(vec![Value::Number(lat as f64), Value::Number(lon as f64)])
            }
            // 0x800e70d0: GlobeCalcLatLonFromOrientationMtx(mtx)
            ("GlobeCalcLatLonFromOrientationMtx", [Value::Table(mtx_id)]) => {
                let mtx = Self::read_matrix4(vm, *mtx_id)?;
                let (lat, lon) = Self::globe_calc_lat_lon_from_orientation_mtx(&mtx);
                Ok(vec![Value::Number(lat as f64), Value::Number(lon as f64)])
            }
            // 0x800e6ce8: GlobeCalcClickedPos(ix, iy, iz, orient_mtx, out_mtx)
            (
                "GlobeCalcClickedPos",
                [
                    Value::Number(ix),
                    Value::Number(iy),
                    Value::Number(iz),
                    Value::Table(orient_id),
                    Value::Table(out_id),
                ],
            ) => {
                let orient = Self::read_matrix4(vm, *orient_id)?;
                let (vx, vy, vz) = (*ix as f32, *iy as f32, *iz as f32);
                // Multiply by orient^T
                let tx = vx * orient[0][0] + vy * orient[1][0] + vz * orient[2][0];
                let ty = vx * orient[0][1] + vy * orient[1][1] + vz * orient[2][1];
                let tz = vx * orient[0][2] + vy * orient[1][2] + vz * orient[2][2];
                let out = Self::orthonormal_basis_from_forward([tx, ty, tz]);
                Self::write_matrix4(vm, *out_id, &out)?;
                let lat = out[2][1].clamp(-1.0, 1.0).asin() * RAD_TO_DEG;
                let lon = out[2][2].atan2(out[2][0]) * RAD_TO_DEG;
                Ok(vec![Value::Number(lat as f64), Value::Number(lon as f64)])
            }
            // 0x800e5ea8: GlobeSelectCalcTargetOrientation(orient_mtx, ix, iy, iz, target_mtx, pitch_deg)
            (
                "GlobeSelectCalcTargetOrientation",
                [
                    Value::Table(orient_id),
                    Value::Number(ix),
                    Value::Number(iy),
                    Value::Number(iz),
                    Value::Table(target_id),
                    Value::Number(pitch_deg),
                ],
            ) => {
                let orient = Self::read_matrix4(vm, *orient_id)?;
                let (vx, vy, vz) = (*ix as f32, *iy as f32, *iz as f32);
                let tx = vx * orient[0][0] + vy * orient[1][0] + vz * orient[2][0];
                let ty = vx * orient[0][1] + vy * orient[1][1] + vz * orient[2][1];
                let tz = vx * orient[0][2] + vy * orient[1][2] + vz * orient[2][2];
                let loc = Self::orthonormal_basis_from_forward([tx, ty, tz]);
                let (lat, lon) = Self::globe_calc_lat_lon_from_location_mtx(&loc);
                let target =
                    Self::globe_lat_lon_to_orientation_matrix(lat + *pitch_deg as f32, lon);
                Self::write_matrix4(vm, *target_id, &target)?;
                Ok(vec![])
            }
            // QuadraticBezier(x0, y0, x1, y1, steps, arc_offset, out_tbl)
            (
                "QuadraticBezier",
                [
                    Value::Number(x0),
                    Value::Number(y0),
                    Value::Number(x1),
                    Value::Number(y1),
                    Value::Number(steps),
                    Value::Number(offset),
                    Value::Table(out_id),
                ],
            ) => {
                let n = (*steps as isize).clamp(1, 128) as usize;
                let dx = *x1 - *x0;
                let dy = *y1 - *y0;
                let len = (dx * dx + dy * dy).sqrt().max(1.0);
                let nx = -dy / len;
                let ny = dx / len;
                let cx = (*x0 + *x1) * 0.5 + nx * *offset;
                let cy = (*y0 + *y1) * 0.5 + ny * *offset;
                vm.table_set(*out_id, Value::String("n".into()), Value::Number(n as f64))?;
                for i in 1..=n {
                    let t = if n > 1 {
                        (i - 1) as f64 / (n - 1) as f64
                    } else {
                        0.0
                    };
                    let u = 1.0 - t;
                    let bx = u * u * *x0 + 2.0 * u * t * cx + t * t * *x1;
                    let by = u * u * *y0 + 2.0 * u * t * cy + t * t * *y1;
                    let pt_id = vm.tables.len();
                    vm.tables.push(Default::default());
                    vm.table_set(pt_id, Value::Number(1.0), Value::Number(bx))?;
                    vm.table_set(pt_id, Value::Number(2.0), Value::Number(by))?;
                    vm.table_set(*out_id, Value::Number(i as f64), Value::Table(pt_id))?;
                }
                Ok(vec![])
            }
            // Mission stats breakdown tables for Victory / Defeat screens
            ("GetMissionSpeedData", [Value::Table(tbl_id)]) => {
                for (idx, val) in [(1, 180.0), (2, 600.0), (3, 165.0), (4, 165.0)] {
                    vm.table_set(*tbl_id, Value::Number(idx as f64), Value::Number(val))?;
                }
                Ok(vec![])
            }
            ("GetMissionPowerData" | "GetMissionTechniqueData", [Value::Table(tbl_id)]) => {
                for (idx, val) in [
                    (1, 10.0),
                    (2, 10.0),
                    (3, 4.0),
                    (4, 4.0),
                    (5, 2.0),
                    (6, 2.0),
                    (7, 0.0),
                    (8, 0.0),
                    (9, 1.0),
                    (10, 1.0),
                ] {
                    vm.table_set(*tbl_id, Value::Number(idx as f64), Value::Number(val))?;
                }
                Ok(vec![])
            }
            // Sun / lighting colour tables for Victory / Defeat globe backdrop
            (
                "GetSunDirection"
                | "GetSunAmbientColour"
                | "GetSunDirectionalColour"
                | "GetAntiSunDirectionalColour",
                [Value::Table(tbl_id)],
            ) => {
                let defaults = if name == "GetSunDirection" {
                    [90.0, 0.0, 0.0]
                } else {
                    [50.0, 50.0, 50.0]
                };
                for (i, v) in defaults.into_iter().enumerate() {
                    vm.table_set(*tbl_id, Value::Number((i + 1) as f64), Value::Number(v))?;
                }
                Ok(vec![])
            }
            (
                "SetSunDirection"
                | "SetSunAmbientColour"
                | "SetSunDirectionalColour"
                | "SetAntiSunDirectionalColour",
                [Value::Table(_)],
            ) => Ok(vec![]),
            ("RunLevel", [Value::Number(level)]) => {
                self.launched_level = Some(*level as i32);
                vm.globals
                    .insert("FrontendDone".into(), Value::Boolean(true));
                vm.globals.insert("PageDone".into(), Value::Boolean(true));
                Ok(vec![])
            }
            ("OpenPage" | "UpdatePage" | "RenderPage" | "ClosePage", [Value::Table(_)]) => {
                Ok(vec![])
            }
            _ => self.call(name, args),
        }
    }

    /// Native name/integer tables at 0x8048d9a8 and 0x8048d4a0.
    pub fn install(vm: &mut Bootstrap) -> Result<()> {
        if ["GAMETYPE", "GAMESTATUS", "VIDEO"]
            .iter()
            .any(|name| vm.globals.contains_key(*name))
        {
            return Err("Frontend constant table already exists".into());
        }
        for (table, members) in [
            (
                "GAMETYPE",
                &[
                    ("Frontend", 0),
                    ("Frontend2", 1),
                    ("SinglePlayer", 2),
                    ("Skirmish", 3),
                    ("Assault", 4),
                    ("Coop", 5),
                    ("MiniGame", 6),
                    ("Unknown", 7),
                ][..],
            ),
            (
                "GAMESTATUS",
                &[
                    ("NONE", 0),
                    ("FRONTEND", 1),
                    ("UNITS", 2),
                    ("CONCEPT", 4),
                    ("CREDITS", 8),
                    ("ENDCREDITS", 16),
                    ("TRAINING", 32),
                    ("PROLOGUE", 64),
                    ("PLAY", 256),
                    ("QUIT", 512),
                    ("LOSE", 1024),
                    ("WIN", 2048),
                    ("MPPLAY", 4096),
                    ("MPLOBBY", 8192),
                    ("MPLOSE", 16384),
                    ("MPWIN", 32768),
                    ("CLIENT", 268435456),
                    ("SERVER", 536870912),
                ][..],
            ),
            ("VIDEO", &[("OFF", 0), ("ONCE", 1), ("LOOPED", 2)][..]),
            ("Build", &[("Debug", 0), ("Release", 1), ("Final", 2)][..]),
            (
                "CARD",
                &[
                    ("NONE", 0),
                    ("Access", 1),
                    ("AllocFailed", 2),
                    ("Busy", 3),
                    ("Corrupt", 4),
                    ("ECCCritical", 5),
                    ("Exists", 6),
                    ("Invalid", 7),
                    ("MaxFD", 8),
                    ("NoBlocks", 9),
                    ("NoBlocksMP", 10),
                    ("NoInodes", 11),
                    ("NoExists", 12),
                    ("NotEmpty", 13),
                    ("OpenFD", 14),
                    ("Authentication", 15),
                    ("Unknown", 16),
                    ("FatalError", 17),
                    ("NotReady", 18),
                    ("InitFailed", 19),
                    ("NoMemory", 20),
                    ("WriteError", 21),
                    ("ReadError", 22),
                    ("CRCError", 23),
                    ("UnknownOp", 24),
                    ("UnknownOpState", 25),
                    ("UnknownError", 26),
                ][..],
            ),
            (
                "GRADE",
                &[("NONE", 1), ("C", 2), ("B", 3), ("A", 4), ("S", 5)][..],
            ),
            (
                "MATCHRESULT",
                &[("Started", 0), ("S", 1), ("C", 2), ("Lost", 3)][..],
            ),
            (
                "MissionData",
                &[
                    ("TroopDeaths", 2),
                    ("GroundDeaths", 4),
                    ("AirDeaths", 6),
                    ("WaterDeaths", 8),
                    ("CurrentTime", 3),
                ][..],
            ),
            (
                "NETWORK",
                &[
                    ("UNKNOWN", 0),
                    ("Initialising", 1),
                    ("GetServers", 2),
                    ("LoginFailed", 3),
                    ("InterfaceFailed", 4),
                    ("DHCPFailed", 5),
                    ("OK", 6),
                    ("Creating", 7),
                    ("CreateFailed", 8),
                    ("Joining", 9),
                    ("JoinFailed", 10),
                    ("LostConnection", 11),
                ][..],
            ),
            ("PLAYER_INDEX", &[("One", 1), ("Two", 2)][..]),
            ("CursorState", &[("Normal", 0), ("Selected", 1)][..]),
            (
                "constant",
                &[
                    ("ONCE", 0),
                    ("LOOPED", 1),
                    ("FILENAME", 0),
                    ("PLAYER_ONE", 0),
                    ("MISSIONOVER_LOSE", 0),
                    ("MISSIONOVER_PLAYER1_WINS", 1),
                ][..],
            ),
            ("eFieldToggle", &[("Off", 0), ("On", 1)][..]),
            ("eModeAction", &[("Follow", 0), ("Wait", 1)][..]),
            (
                "eNameCheckResults",
                &[("Allowed", 0), ("NotAllowed", 1)][..],
            ),
            (
                "eRematchStatus",
                &[("Pending", 0), ("Yes", 1), ("No", 2)][..],
            ),
            (
                "eWiiMatchingStatus",
                &[
                    ("Success", 0),
                    ("Busy", 1),
                    ("BusyThread", 2),
                    ("Connecting", 3),
                    ("ConnectionProblem", 4),
                    ("Failure", 5),
                ][..],
            ),
        ] {
            if vm.globals.contains_key(table) {
                return Err(format!("Frontend constant table already exists: {table}"));
            }
            let id = vm.tables.len();
            vm.tables.push(
                members
                    .iter()
                    .map(|(member, value)| (member.to_string(), Value::Number(*value as f64)))
                    .collect(),
            );
            vm.globals.insert(table.into(), Value::Table(id));
        }
        for name in [
            "GetFramesPerSecond",
            "GetNumOfGameType",
            "GetCursorPosition",
            "GetCursorRoll",
            "SetCursorState",
            "EnableControllerErrorMessages",
            "OpenSprite",
            "CloseSprite",
            "GetSprite",
            "MoveSprite",
            "DrawSprite",
            "BuildType",
            "sin",
            "cos",
            "tan",
            "mod",
            "sqrt",
            "abs",
            "ceil",
            "floor",
            "rint",
            "GetNumGlobalStrings",
            "GetString",
            "GetStringFromTable",
            "FormatString",
            "GetCharFromString",
            "GetLanguageStr",
            "GetRegionStr",
            "GetVersionStr",
            "SetFontKernFactor",
            "GetScreenRatio",
            "GetTime",
            "GetRandom",
            "PlayVideo",
            "IsVideoFinished",
            "GetTotalFrames",
            "GetFrameNum",
            "StartVO",
            "StopVO",
            "CheckVOFinished",
            "SetCurrentMissionAttempted",
            "GetCurrentMissionAttempted",
            "SetCurrentMissionCompleted",
            "GetCurrentMissionCompleted",
            "SetCurrentFMVOutSeen",
            "GetCurrentFMVOutSeen",
            "GetPersistentData",
            "GetLevelsDone",
            "SetLevelsDone",
            "GetLevelsAttemptedOrDone",
            "RegisterMissionAttempt",
            "SetSelectedSlot",
            "StartNewSlot",
            "GetProfileName",
            "SetProfileName",
            "ClearProfile",
            "SaveProfileToUndoBuffer",
            "RestoreProfileFromUndoBuffer",
            "GetProfileNameCheckResult",
            "GetSlotTime",
            "GetCardError",
            "ClearCardError",
            "CheckSaveFile",
            "CreateSaveFile",
            "WriteSaveFile",
            "DeleteSaveFile",
            "CheckNANDResources",
            "GetGameSaveValid",
            "SetGameSaveValid",
            "GetGeneralPurposeFlags",
            "SetGeneralPurposeFlags",
            "GetUnlockFlags",
            "SetUnlockFlags",
            "GetUnlockSeenFlags",
            "SetUnlockSeenFlags",
            "AddUnlockFlag",
            "RemoveUnlockFlag",
            "TestUnlockFlag",
            "TestAnyUnlockFlag",
            "GetCurrentLevel",
            "SetCurrentLevel",
            "GetLevelGameType",
            "GetMissionStatus",
            "SetMissionStatus",
            "TestMissionStatus",
            "GetMissionData",
            "GetStoredMissionData",
            "SetCurrentMissionStats",
            "LoadCurrentMissionStats",
            "SetMissionComplete",
            "GetMissionSpeedData",
            "GetMissionPowerData",
            "GetMissionTechniqueData",
            "GetObjectivesResult",
            "TriggerFrameSave",
            "BlockHomebutton",
            "UnblockHomebutton",
            "ResetToIPL",
            "StartMusic",
            "StopMusic",
            "CheckMusicFinished",
            "SetMusicVolume",
            "SetMusicVolumeS",
            "GetMusicVolume",
            "SetSFXVolume",
            "GetSFXVolume",
            "SetCOVoiceVolume",
            "GetCOVoiceVolume",
            "SetCOSubtitles",
            "GetCOSubtitles",
            "SetMovieSubtitles",
            "GetMovieSubtitles",
            "SetFieldToggle",
            "GetFieldToggle",
            "SetFollowMode",
            "GetFollowMode",
            "SetTutorialMode",
            "GetTutorialMode",
            "InitSettings",
            "ApplySettings",
            "EndSettings",
            "PlaySound",
            "PlayEffect",
            "StartWidget",
            "StopWidget",
            "GetWidgetValue",
            "SetWidgetValue",
            "SetListBoxData",
            "GetCurrentMPProfile",
            "SetCurrentMPProfile",
            "GetIsClientOrServer",
            "SetIsClientOrServer",
            "GetNetworkStatus",
            "GetNetworkErrorCode",
            "RequestNetworkOpen",
            "RequestNetworkClose",
            "RequestLoadMultiPlayer",
            "RequestCreateMultiPlayer",
            "RequestSaveMultiPlayer",
            "RequestDeleteMultiPlayer",
            "DontSaveMultiPlayer",
            "MultiPlayerNeedsSaving",
            "GetMPPageFromCode",
            "GetMPStatistics",
            "UpdateMPStatistics",
            "UpdateMPUnlockStatus",
            "GetOwnRank",
            "GetRankIncreased",
            "GetRankDecreased",
            "ResetRankChange",
            "GetRankFromDiff",
            "GetSkill",
            "GetSkillUncertainty",
            "GetFriendKey",
            "GetFriendName",
            "SetFriendName",
            "GetFriendRank",
            "SetFriendRank",
            "GetFriendStatusCode",
            "GetFriendStatusStringTag",
            "SetOwnStatusStringTag",
            "RemoveOwnStatusStringTag",
            "AddFriend",
            "DeleteFriend",
            "IsOkToChangeFriends",
            "StartMatchMaking",
            "StopMatchMaking",
            "CheckMatchMaking",
            "startSearching",
            "LeaveMPGame",
            "IsNetworkPlayer",
            "GetPlayerInformationReady",
            "GetPlayerInformation",
            "GetFriendlyArmy",
            "GetEnemyArmy",
            "GetFriendlyScoreTotals",
            "GetEnemyScoreTotals",
            "GetTmpStore",
            "SetTmpStore",
            "UpdateRematch",
            "IQuit",
            "OtherPlayerQuit",
            "WaitFor",
            "MatrixMultiply",
            "RotationToVector",
            "GlobeLatLonToOrientationMatrix",
            "GlobeCalcLatLonFromLocationMtx",
            "GlobeCalcLatLonFromOrientationMtx",
            "GlobeCalcClickedPos",
            "GlobeSelectCalcTargetOrientation",
            "CalcGlobeInterection",
            "GlobePickRegionFromMap",
            "QuadraticBezier",
            "GetSunDirection",
            "SetSunDirection",
            "GetSunAmbientColour",
            "SetSunAmbientColour",
            "GetSunDirectionalColour",
            "SetSunDirectionalColour",
            "GetAntiSunDirectionalColour",
            "SetAntiSunDirectionalColour",
            "RunLevel",
            "OpenPage",
            "UpdatePage",
            "RenderPage",
            "ClosePage",
        ] {
            vm.globals.insert(name.into(), Value::Native(name.into()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_rate_and_duplicates() {
        let c = level_catalog::Catalog::parse("<levels><level name=\"unused\" filename=\"@os:Data/CompoundFiles/A.xml\" gametype=\"frontend2\"/><level name=\"unused\" filename=\"@os:Data/CompoundFiles/B.xml\" gametype=\"frontend2\"/></levels>").unwrap();
        let mut env = Environment::new(&c, 59.94).unwrap();
        assert_eq!(
            env.call("GetFramesPerSecond", &[]).unwrap(),
            vec![Value::Number(59.0)]
        );
        assert_eq!(
            env.call("GetNumOfGameType", &[Value::Number(1.0)]).unwrap(),
            vec![Value::Number(2.0)]
        );
        assert!(env.call("GetNumOfGameType", &[Value::Number(0.5)]).is_err());
        assert!(env.call("GetFramesPerSecond", &[Value::Nil]).is_err());
        for invalid in [0.0, -1.0, f32::NAN, f32::INFINITY, 4294967296.0] {
            assert!(Environment::new(&c, invalid).is_err());
        }
    }

    #[test]
    fn globe_orientation_round_trip() {
        for (lat, lon) in [(17.0f32, 2.0f32), (-10.0, 96.0), (32.0, -128.0)] {
            let mtx = Environment::globe_lat_lon_to_orientation_matrix(lat, lon);
            let (out_lat, out_lon) = Environment::globe_calc_lat_lon_from_orientation_mtx(&mtx);
            assert!((out_lat + lat).abs() < 0.05, "lat={lat} out_lat={out_lat}");
            let expected_lon = lon - 90.0;
            let diff_lon = (out_lon - expected_lon + 180.0).rem_euclid(360.0) - 180.0;
            assert!(diff_lon.abs() < 0.05, "lon={lon} out_lon={out_lon}");
        }
    }
}
