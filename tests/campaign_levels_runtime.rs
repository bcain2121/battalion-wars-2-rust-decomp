#![cfg(feature = "viewer")]
use bw_rust::{
    ObjectGraph,
    first_mission::{Input, Session},
    parse_objects,
};
use std::path::Path;

fn load_graph_and_archive(root: &Path, level: &str) -> (ObjectGraph, Vec<u8>) {
    let data = root.join("local/extracted/files/Data/CompoundFiles");
    let preload = std::fs::read_to_string(data.join(format!("{level}_Level_preload.xml")))
        .or_else(|_| std::fs::read_to_string(data.join(format!("{level}_Level_Preload.xml"))))
        .unwrap();
    let level_xml = std::fs::read_to_string(data.join(format!("{level}_Level.xml"))).unwrap();
    let archive = std::fs::read(data.join(format!("{level}_Level.res"))).unwrap();
    let mut graph = ObjectGraph::default();
    graph.merge_dedup(parse_objects(&preload).unwrap()).unwrap();
    graph
        .merge_dedup(parse_objects(&level_xml).unwrap())
        .unwrap();
    (graph, archive)
}

/// Drive each selected level with the diagnostic [`Autopilot`] (ordinary input
/// only) and report objective completions, mission outcome and the first
/// unsupported boundary. `BW_SURVEY_LEVELS`, `BW_SURVEY_FRAMES` select scope.
#[test]
#[ignore = "diagnostic autopilot survey over private original assets"]
fn autopilot_survey_campaign_levels() {
    use bw_rust::autopilot::{Autopilot, Goal};
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let data = root.join("local/extracted/files/Data/CompoundFiles");
    if !data.join("SP_1.1_Level.xml").exists() {
        return;
    }
    let frames: usize = std::env::var("BW_SURVEY_FRAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(48_000);
    let only = std::env::var("BW_SURVEY_LEVELS").unwrap_or_else(|_| "SP_1.1".into());
    for level in only.split(',') {
        let (graph, archive) = load_graph_and_archive(root, level);
        let mut session = match Session::load_level(root, level, &graph, &archive) {
            Ok(s) => s,
            Err(err) => {
                println!("AUTOPILOT {level}: LOAD ERR -> {err}");
                continue;
            }
        };
        let mut pilot = Autopilot::new();
        let mut completed = std::collections::BTreeSet::new();
        let mut last_goal = Goal::Idle;
        let mut result = format!("RAN {frames} frames");
        for frame in 0..frames {
            let (input, goal) = pilot.step(&session);
            if goal != last_goal && !matches!(goal, Goal::Enemy(_)) {
                let p = session
                    .program
                    .host
                    .world
                    .player
                    .and_then(|p| session.program.host.positions.get(&p).copied());
                println!("AUTOPILOT {level}: frame {frame} goal {goal:?} pos {p:?}");
                last_goal = goal;
            }
            if std::env::var("BW_SURVEY_DEBUG_STUCK").is_ok() && frame % 60 == 0 {
                let w = &session.program.host.world;
                let pid = w.player;
                let p = pid.and_then(|p| session.program.host.positions.get(&p).copied());
                let hp = pid.and_then(|p| w.units.get(&p)).map(|u| u.health);
                let dv = pid.and_then(|p| w.direct_velocity.get(&p).copied());
                let cps: Vec<_> = w
                    .capture_point_state
                    .iter()
                    .map(|(cid, st)| (cid.0, st.owner.as_str(), st.height as i32))
                    .collect();
                println!(
                    "DBG {level} f={frame}: player={pid:?} hp={hp:?} goal={goal:?} pos={p:?} mov={:?} look={:?} fire={} dv={dv:?} cps={cps:?}",
                    input.movement, input.look, input.fire
                );
            }
            let recon_hp_before = session
                .program
                .host
                .world
                .units
                .get(&bw_rust::ObjectId(50022321))
                .map(|u| u.health);
            if let Err(err) = session.tick(1.0 / 60.0, input) {
                result = format!("TICK ERR frame {frame} -> {err}");
                break;
            }
            let recon_hp_after = session
                .program
                .host
                .world
                .units
                .get(&bw_rust::ObjectId(50022321))
                .map(|u| u.health);
            if std::env::var("BW_SURVEY_DEBUG_STUCK").is_ok() && recon_hp_before != recon_hp_after {
                let p = session
                    .program
                    .host
                    .positions
                    .get(&bw_rust::ObjectId(50022321))
                    .copied();
                let shots: Vec<_> = session
                    .combat
                    .shots
                    .iter()
                    .filter(|s| s.hit == Some(bw_rust::ObjectId(50022321)) && s.damage > 0.0)
                    .map(|s| (s.shooter.0, s.damage, s.from))
                    .collect();
                println!(
                    "RECON HP f={frame}: {recon_hp_before:?} -> {recon_hp_after:?} at {p:?} shots={shots:?}"
                );
            }
            for (id, (_, done)) in &session.program.host.world.objectives {
                if *done && completed.insert(*id) {
                    println!(
                        "AUTOPILOT {level}: frame {frame} objective {} complete",
                        id.0
                    );
                }
            }
            if let Some(outcome) = &session.program.host.world.mission_complete {
                result = format!("OUTCOME frame {frame} -> {outcome:?}");
                break;
            }
            if frame % 3600 == 3599 {
                let w = &session.program.host.world;
                let p = w
                    .player
                    .and_then(|p| session.program.host.positions.get(&p).copied());
                let hp = w.player.and_then(|p| w.units.get(&p)).map(|u| u.health);
                let spd = w
                    .player
                    .and_then(|p| w.units.get(&p))
                    .and_then(|u| u.forward_speed);
                println!(
                    "AUTOPILOT {level}: frame {frame} pos {p:?} hp {hp:?} spd {spd:?} frozen {} goal {goal:?} mov {:?} fire {} xfer {}",
                    w.frozen, input.movement, input.fire, input.transfer
                );
            }
        }
        println!(
            "AUTOPILOT {level}: {result} objectives_done={}",
            completed.len()
        );
        if std::env::var("BW_SURVEY_DUMP").is_ok() {
            let dump_complete = std::env::var("BW_SURVEY_DUMP_COMPLETE").unwrap_or_default();
            let dump_script = std::env::var("BW_SURVEY_DUMP_SCRIPT").unwrap_or_default();
            for (id, t) in &session.scheduler.threads {
                println!(
                    "THREAD {level}: {id} {} pc={} status={:?}",
                    t.name, t.pc, t.status
                );
                if !dump_script.is_empty() && t.name.contains(&dump_script) {
                    println!("  FULL {level} {}: {}", t.name, t.debug_context(5000));
                } else if !matches!(t.status, bw_rust::mission_vm::Status::Complete(_))
                    || (!dump_complete.is_empty() && t.name.contains(&dump_complete))
                {
                    println!("  CTX {level} {}: {}", t.name, t.debug_context(65));
                }
            }
            for m in &session.markers {
                if let Ok(Some(p)) = m.position(&session.program.host) {
                    let target = m.effective_target(&session.program.host);
                    let w = &session.program.host.world;
                    let t = w.entity(target).ok().map(|u| {
                        (
                            u.health,
                            u.max_health,
                            u.invulnerable,
                            u.army.clone(),
                            u.active,
                        )
                    });
                    println!(
                        "MARKER {level}: {} -> {} at {p:?} target={t:?} prop={} building={}",
                        m.id.0,
                        target.0,
                        w.props.contains_key(&target),
                        w.buildings.contains(&target)
                    );
                }
            }
            let w = &session.program.host.world;
            println!(
                "ROSTER {level}: {:?} player={:?}",
                w.roster
                    .iter()
                    .map(|id| (
                        id.0,
                        w.units.get(id).map(|u| u.health),
                        session.program.host.positions.get(id).copied(),
                        w.orders.get(id).map(|o| (o.target, o.radius)),
                        w.squad_follow.contains(id),
                    ))
                    .collect::<Vec<_>>(),
                w.player
            );
            if let Some(p) = w
                .player
                .and_then(|p| session.program.host.positions.get(&p).copied())
            {
                for (id, prop) in &w.props {
                    let Some(&pos) = session.program.host.positions.get(id) else {
                        continue;
                    };
                    let dx = pos[0] - p[0];
                    let dz = pos[2] - p[2];
                    let d = (dx * dx + dz * dz).sqrt();
                    if d <= 50.0 || prop.max_health >= 200.0 {
                        println!(
                            "NEAR PROP {level}: {} d={d:.1} pos={pos:?} hp={}/{} invuln={} army={} active={:?} bldg={}",
                            id.0,
                            prop.health,
                            prop.max_health,
                            prop.invulnerable,
                            prop.army,
                            prop.active,
                            w.buildings.contains(id)
                        );
                    }
                }
                for (id, u) in &w.units {
                    if u.active != Some(true) || u.health <= 0.0 {
                        continue;
                    }
                    let Some(&pos) = session.program.host.positions.get(id) else {
                        continue;
                    };
                    let dx = pos[0] - p[0];
                    let dz = pos[2] - p[2];
                    let d = (dx * dx + dz * dz).sqrt();
                    if d <= 2000.0 {
                        println!(
                            "NEAR UNIT {level}: {} d={d:.1} pos={pos:?} hp={}/{} invuln={} army={} water={} air={} order={:?} atk={:?}",
                            id.0,
                            u.health,
                            u.max_health,
                            u.invulnerable,
                            u.army,
                            w.water_units.contains(id),
                            w.air_shapes.contains_key(id) && !w.water_units.contains(id),
                            w.orders.get(id).map(|o| (o.target, o.radius)),
                            w.attack_orders.get(id).map(|t| t.0),
                        );
                    }
                }
            }
        }
    }
}

/// Survey every single-player level and report the first load or tick boundary
/// for each, without stopping at the first failing level. Diagnostic only: it
/// supplies a confirm press and otherwise idle input, so it measures startup
/// and early-script coverage, not mission completion.
#[test]
#[ignore = "diagnostic survey over private original assets"]
fn survey_all_campaign_levels() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let data = root.join("local/extracted/files/Data/CompoundFiles");
    if !data.join("SP_1.1_Level.xml").exists() {
        return;
    }
    let frames: usize = std::env::var("BW_SURVEY_FRAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3600);
    let only = std::env::var("BW_SURVEY_LEVELS").ok();
    let levels = [
        "SP_0.1", "SP_1.1", "SP_1.2", "SP_1.3", "SP_1.4", "SP_1.5", "SP_2.1", "SP_2.2", "SP_2.3",
        "SP_3.1", "SP_3.2", "SP_3.3", "SP_3.4", "SP_4.1", "SP_4.2", "SP_4.3", "SP_5.1", "SP_5.2",
        "SP_5.3", "SP_5.4",
    ];
    for level in levels {
        if only
            .as_deref()
            .is_some_and(|o| !o.split(',').any(|l| l == level))
        {
            continue;
        }
        let (graph, archive) = load_graph_and_archive(root, level);
        let mut session = match Session::load_level(root, level, &graph, &archive) {
            Ok(s) => s,
            Err(err) => {
                println!("SURVEY {level}: LOAD ERR -> {err}");
                continue;
            }
        };
        let mut result = format!("OK {frames} frames");
        for frame in 0..frames {
            let mut input = Input::default();
            if frame % 120 == 119 {
                input.confirm = true;
            }
            if let Err(err) = session.tick(1.0 / 60.0, input) {
                result = format!("TICK ERR frame {frame} -> {err}");
                break;
            }
        }
        println!(
            "SURVEY {level}: {result} (frozen={})",
            session.program.host.world.frozen
        );
    }
}

#[test]
fn probe_campaign_levels_loading() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    if !root
        .join("local/extracted/files/Data/CompoundFiles/SP_1.1_Level.xml")
        .exists()
    {
        return;
    }
    for level in ["SP_1.1", "SP_1.2", "SP_1.3", "SP_1.4", "SP_1.5"] {
        let (graph, archive) = load_graph_and_archive(root, level);
        match Session::load_level(root, level, &graph, &archive) {
            Ok(mut session) => {
                println!(
                    "{level}: LOADED OK! player={:?} units={} props={}",
                    session.program.host.world.player,
                    session.program.host.world.units.len(),
                    session.program.host.world.props.len()
                );
                let mut tick_err = None;
                for frame in 0..1800 {
                    let mut input = Input::default();
                    if frame == 120 {
                        input.confirm = true;
                    }
                    if let Err(err) = session.tick(1.0 / 60.0, input) {
                        tick_err = Some((frame, err));
                        break;
                    }
                }
                if let Some((frame, err)) = tick_err {
                    panic!("{level}: TICK ERR at frame {frame} -> {err}");
                } else {
                    println!(
                        "{level}: TICKED 1800 FRAMES OK! player={:?} frozen={}",
                        session.program.host.world.player, session.program.host.world.frozen
                    );
                }
            }
            Err(err) => {
                panic!("{level}: ERR -> {err}");
            }
        }
    }
}

#[test]
fn probe_multiplayer_levels_loading() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    if !root
        .join("local/extracted/files/Data/CompoundFiles/MP1_Level.xml")
        .exists()
    {
        return;
    }
    let mp_levels = [
        "MP1", "MP2", "MP3", "MP4", "MP5", "MP6", "MP7", "MP8", "MP9", "MP10", "MP11", "MP12",
        "MP13", "MP14", "MP15", "MP17",
    ];
    let mut failures = Vec::new();
    for level in mp_levels {
        let (graph, archive) = load_graph_and_archive(root, level);
        match Session::load_level(root, level, &graph, &archive) {
            Ok(mut session) => {
                assert!(
                    session.program.host.world.is_multiplayer,
                    "{level}: expected is_multiplayer == true"
                );
                assert!(
                    !session.program.host.world.roster.is_empty(),
                    "{level}: expected non-empty Player 1 roster"
                );
                let mut saw_ai_orders = false;
                for frame in 0..300 {
                    let mut input = Input::default();
                    if frame % 60 == 59 {
                        input.confirm = true;
                    }
                    if let Err(err) = session.tick(1.0 / 60.0, input) {
                        println!("{level}: TICK ERR at frame {frame} -> {err}");
                        failures.push(format!("{level} tick {frame}: {err}"));
                        break;
                    }
                    let w = &session.program.host.world;
                    if !w.attack_orders.is_empty()
                        || !w.entering_vehicle.is_empty()
                        || !w.orders.is_empty()
                    {
                        saw_ai_orders = true;
                    }
                }
                let w = &session.program.host.world;
                println!(
                    "{level}: OK! player={:?} roster={} units={} atk={} cap={} orders={}",
                    w.player,
                    w.roster.len(),
                    w.units.len(),
                    w.attack_orders.len(),
                    w.entering_vehicle.len(),
                    w.orders.len()
                );
                if w.player.is_none() {
                    failures.push(format!(
                        "{level}: expected Player 1 controlled unit after cutscene skip"
                    ));
                }
                if !saw_ai_orders {
                    failures.push(format!(
                        "{level}: Skirmish AI issued no orders in 300 frames"
                    ));
                }
                if level == "MP7" {
                    let w = &mut session.program.host.world;
                    let p1_army = w
                        .player
                        .and_then(|p| w.units.get(&p))
                        .map(|u| u.army.clone())
                        .unwrap_or_default();
                    let mut killed = 0;
                    for u in w.units.values_mut() {
                        if u.army != p1_army && !u.army.is_empty() && u.health > 0.0 {
                            u.health = 0.0;
                            killed += 1;
                            if killed >= 5 {
                                break;
                            }
                        }
                    }
                    w.timer_seconds = Some(0.0);
                    for frame in 300..360 {
                        let mut input = Input::default();
                        if frame % 15 == 0 {
                            input.confirm = true;
                        }
                        if let Err(err) = session.tick(1.0 / 60.0, input) {
                            failures.push(format!("MP7 win check tick {frame}: {err}"));
                            break;
                        }
                        if session.program.host.world.mission_complete.is_some() {
                            break;
                        }
                    }
                    if !matches!(
                        session.program.host.world.mission_complete,
                        Some(bw_rust::mission_world::MissionOutcome::Won { .. })
                    ) {
                        failures.push(format!(
                            "MP7: expected MissionOutcome::Won after timer expiry with score lead, got {:?}",
                            session.program.host.world.mission_complete
                        ));
                    }
                }
            }
            Err(err) => {
                println!("{level}: LOAD ERR -> {err}");
                failures.push(format!("{level} load: {err}"));
            }
        }
    }
    assert!(failures.is_empty(), "MP failures:\n{}", failures.join("\n"));
}
