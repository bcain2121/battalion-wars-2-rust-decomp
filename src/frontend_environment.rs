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
}

impl Environment {
    /// The caller supplies the simulation's frontend rate, not a measured GPU FPS.
    pub fn new(catalog: &level_catalog::Catalog, frame_rate: f32) -> Result<Self> {
        if !frame_rate.is_finite() || !(1.0..4294967296.0).contains(&frame_rate) {
            return Err("Unsupported frontend frame rate".into());
        }
        let mut counts = BTreeMap::new();
        for entry in &catalog.entries {
            *counts.entry(entry.gametype.clone()).or_insert(0) += 1;
        }
        // Lazy singleton constructor 0x8024c870 writes 1 to the disabled byte.
        // This is a fresh frontend environment, not imported mid-session state.
        Ok(Self {
            counts,
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
        })
    }

    pub fn controller_errors_enabled(&self) -> bool {
        !self.controller_errors_disabled
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
                        self.video_total_frames = 600;
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
            ("SetCurrentFMVOutSeen", [val]) => {
                self.current_fmv_out_seen = !matches!(val, Value::Nil | Value::Boolean(false));
                Ok(vec![])
            }
            ("GetCurrentFMVOutSeen", []) => Ok(vec![Value::Boolean(self.current_fmv_out_seen)]),
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
                        Value::Boolean(true),
                        Value::Number((gx + dx) as f64),
                        Value::Number((gy + dy) as f64),
                        Value::Number((gz - dz) as f64),
                    ])
                } else {
                    Ok(vec![
                        Value::Boolean(false),
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
            "GetCursorRoll",
            "EnableControllerErrorMessages",
            "OpenSprite",
            "CloseSprite",
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
            "MatrixMultiply",
            "RotationToVector",
            "GlobeLatLonToOrientationMatrix",
            "GlobeCalcLatLonFromLocationMtx",
            "GlobeCalcLatLonFromOrientationMtx",
            "GlobeCalcClickedPos",
            "GlobeSelectCalcTargetOrientation",
            "CalcGlobeInterection",
            "GlobePickRegionFromMap",
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
