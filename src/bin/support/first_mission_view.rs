//! Playable Battalion Wars 2 frontend menus, campaign mission select (SP_0.1..SP_1.5), and 1-to-1 mission runtime.
use super::Loaded;
use bevy::prelude::*;
use bw_rust::{
    ObjectGraph, ObjectId, Result,
    animation::{self, Animation},
    first_mission::{Input, Session},
    frontend_runtime::Runtime as FrontendRuntime,
    level_catalog,
    localization::Language,
    mission_music::Stereo,
    mission_world::MissionOutcome,
    model::Model,
    model_render::{self, RenderModel},
    original_scene::{Placement, field},
    parse_objects,
    sound_samples::Samples,
    texture,
    unit_animation::Motion,
};
use std::collections::{BTreeMap, VecDeque};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrontendStage {
    Title,
    SaveSelect,
    NameEntry,
    MainMenu,
    CampaignSelect,
    MultiplayerInfo,
    Options,
    ExtrasMenu,
    UnitDossier,
    ConceptGallery,
    Credits,
    MovieTheater,
    InMission,
    Victory,
    Defeat,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SaveProfile {
    pub name: String,
    pub levels_done: usize,
    pub sp_0_1_scores: Option<(f64, f64, f64)>,
    pub sp_0_1_grade: Option<String>,
    pub mission_scores: [Option<(f64, f64, f64)>; 20],
    pub mission_grades: [Option<String>; 20],
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SaveFileData {
    pub slots: [Option<SaveProfile>; 3],
    pub selected_slot: usize,
    pub music_volume: f32,
    pub sfx_volume: f32,
    pub mouse_sensitivity: f32,
    pub aim_assist_enabled: bool,
}

pub struct ActiveMoviePlayer {
    #[allow(dead_code)]
    pub filename: String,
    pub decoder: std::sync::Mutex<bw_rust::movie::Decoder>,
    pub info: bw_rust::movie::Info,
    pub wav: Option<Vec<u8>>,
    pub image: Handle<Image>,
    pub pending: Option<bw_rust::movie::Frame>,
    pub started: bool,
    pub elapsed: f64,
    pub last_frame: u64,
}

pub struct CampaignMissionMeta {
    pub code: &'static str,
    pub short_title: &'static str,
    pub full_title: &'static str,
    pub co_left_portrait: &'static str,
    pub co_right_portrait: &'static str,
    pub co_matchup: &'static str,
    pub default_briefing: &'static str,
    pub fe_string_idx: Option<usize>,
}

pub const CAMPAIGN_1_MISSIONS: [CampaignMissionMeta; 20] = [
    CampaignMissionMeta {
        code: "SP_0.1",
        short_title: "PROLOGUE: A FLASH IN THE PAN",
        full_title: "PROLOGUE MISSION: A FLASH IN THE PAN (SP_0.1)",
        co_left_portrait: "co_sleiqo_h",
        co_right_portrait: "co_l_ferrok_h",
        co_matchup: "CO: Empress Lei-Qo (Solar Empire) vs. Lord Ferrok (Iron Legion)",
        default_briefing: "200 Years Ago - The Iron Legion hordes have invaded the Lightning Isle! Lead the Solar Empire strike force through the trenches, claim the Solar Staff, command your squad of Riflemen, Flame Vets, and Bazooka Vets, and call in the Stratodestroyer to obliterate the Iron Tower!",
        fe_string_idx: Some(1132),
    },
    CampaignMissionMeta {
        code: "SP_1.1",
        short_title: "1. ANGLO ATTACK",
        full_title: "MISSION 1: ANGLO ATTACK (SP_1.1)",
        co_left_portrait: "co_sleiqo_h",
        co_right_portrait: "co_ai_pierce_h",
        co_matchup: "CO: Empress Lei-Qo (Solar Empire) vs. Commander Pierce (Anglo Isles)",
        default_briefing: "The Anglo Isles have launched a surprise attack on the outlying islands of the Solar Empire. Empress Lei-Qo must act quickly or face the dishonor of defeat!",
        fe_string_idx: Some(1133),
    },
    CampaignMissionMeta {
        code: "SP_1.2",
        short_title: "2. RALLY THE DEFENSES",
        full_title: "MISSION 2: RALLY THE DEFENSES (SP_1.2)",
        co_left_portrait: "co_s_aqira_h",
        co_right_portrait: "co_ai_pierce_h",
        co_matchup: "CO: Admiral A-Qira & General Herman vs. Commander Pierce (Anglo Isles)",
        default_briefing: "The Western Frontier garrison is struggling to hold Bongo Island. Admiral A-Qira has assembled a small Battalion at the island's southernmost tip. With it, he must secure the island!",
        fe_string_idx: Some(1134),
    },
    CampaignMissionMeta {
        code: "SP_1.3",
        short_title: "3. A MATTER OF HONOR",
        full_title: "MISSION 3: A MATTER OF HONOR (SP_1.3)",
        co_left_portrait: "co_sleiqo_h",
        co_right_portrait: "co_aiwindsor_h",
        co_matchup: "CO: Empress Lei-Qo (Solar Empire) vs. Colonel Windsor (Anglo Isles)",
        default_briefing: "The Anglo Isles are staging an assault on the Imperial Palace itself! If it falls, Empress Lei-Qo shall be eternally shamed... The fate of the Solar Empire rests in her hands!",
        fe_string_idx: Some(1135),
    },
    CampaignMissionMeta {
        code: "SP_1.4",
        short_title: "4. SHOWDOWN AT BIG HONSHU",
        full_title: "MISSION 4: SHOWDOWN AT BIG HONSHU (SP_1.4)",
        co_left_portrait: "co_s_aqira_h",
        co_right_portrait: "co_aiwindsor_h",
        co_matchup: "CO: Admiral A-Qira (Solar Empire) vs. Colonel Windsor (Anglo Isles)",
        default_briefing: "The Anglo Isles' primary offensive has been routed. Now A-Qira must avert a retaliatory strike by destroying the enemy missile silos positioned at the foot of Big Honshu, the Solar Empire's largest volcano.",
        fe_string_idx: Some(1136),
    },
    CampaignMissionMeta {
        code: "SP_1.5",
        short_title: "5. REPEL THE ENEMY",
        full_title: "MISSION 5: REPEL THE ENEMY (SP_1.5)",
        co_left_portrait: "co_s_aqira_h",
        co_right_portrait: "co_ai_pierce_h",
        co_matchup: "CO: Admiral A-Qira & General Herman vs. Commander Pierce (Anglo Isles)",
        default_briefing: "The Anglo Isles are on the retreat. It is time for A-Qira to lay siege to their last stronghold and expel them forthwith from the Solar Empire.",
        fe_string_idx: Some(1137),
    },
    CampaignMissionMeta {
        code: "SP_2.1",
        short_title: "C2-1. INVASION OF SOLAR EMPIRE",
        full_title: "CAMPAIGN 2 MISSION 1: INVASION OF THE SOLAR EMPIRE (SP_2.1)",
        co_left_portrait: "co_ai_pierce_h",
        co_right_portrait: "co_sleiqo_h",
        co_matchup: "CO: Commander Pierce (Anglo Isles) vs. Empress Lei-Qo (Solar Empire)",
        default_briefing: "Flashback to the opening of the Anglo Isles' amphibious campaign against the Solar Empire! Lead the Anglo Isles landing force ashore and secure the beachhead!",
        fe_string_idx: Some(1138),
    },
    CampaignMissionMeta {
        code: "SP_2.2",
        short_title: "C2-2. STRIKE AT THE HEART",
        full_title: "CAMPAIGN 2 MISSION 2: STRIKE AT THE HEART (SP_2.2)",
        co_left_portrait: "co_aiwindsor_h",
        co_right_portrait: "co_s_aqira_h",
        co_matchup: "CO: Colonel Windsor (Anglo Isles) vs. Admiral A-Qira (Solar Empire)",
        default_briefing: "Push deeper into Solar Empire territory with Anglo Isles armor, air power, and naval forces to neutralize the Imperial defenses.",
        fe_string_idx: Some(1139),
    },
    CampaignMissionMeta {
        code: "SP_2.3",
        short_title: "C2-3. CAPTURE THE DREADNOUGHT",
        full_title: "CAMPAIGN 2 MISSION 3: CAPTURE THE DREADNOUGHT (SP_2.3)",
        co_left_portrait: "co_ai_pierce_h",
        co_right_portrait: "co_s_aqira_h",
        co_matchup: "CO: Commander Pierce & Colonel Windsor vs. Admiral A-Qira (Solar Empire)",
        default_briefing: "Assault the Solar Empire naval shipyards and seize control of the prototype Imperial Dreadnought!",
        fe_string_idx: Some(1140),
    },
    CampaignMissionMeta {
        code: "SP_3.1",
        short_title: "C3-1. RAID ON ROBOT FACTORY",
        full_title: "CAMPAIGN 3 MISSION 1: RAID ON THE ROBOT FACTORY (SP_3.1)",
        co_left_portrait: "co_wf_yherman_h",
        co_right_portrait: "co_l_ferrok_h",
        co_matchup: "CO: Young Herman (Western Frontier) vs. Lord Ferrok (Iron Legion)",
        default_briefing: "Flashback to the Lightning Wars! Command Western Frontier forces in a daring strike against the Iron Legion's fortress and war machines.",
        fe_string_idx: Some(1141),
    },
    CampaignMissionMeta {
        code: "SP_3.2",
        short_title: "C3-2. HOLD THE LINE",
        full_title: "CAMPAIGN 3 MISSION 2: HOLD THE LINE (SP_3.2)",
        co_left_portrait: "co_wf_yherman_h",
        co_right_portrait: "co_l_ferrok_h",
        co_matchup: "CO: Young Herman & Young Gorgi vs. Lord Ferrok (Iron Legion)",
        default_briefing: "Defend the allied trenches and fortifications against waves of Iron Legion Battlestations, Gunships, and shock troops.",
        fe_string_idx: Some(1142),
    },
    CampaignMissionMeta {
        code: "SP_3.3",
        short_title: "C3-3. STORM THE CITADEL",
        full_title: "CAMPAIGN 3 MISSION 3: STORM THE CITADEL (SP_3.3)",
        co_left_portrait: "co_wf_yherman_h",
        co_right_portrait: "co_l_ferrok_h",
        co_matchup: "CO: Young Herman & Empress Lei-Qo vs. Lord Ferrok (Iron Legion)",
        default_briefing: "Breach the outer defenses of Lord Ferrok's Iron Citadel and clear the path for the final allied assault.",
        fe_string_idx: Some(1143),
    },
    CampaignMissionMeta {
        code: "SP_3.4",
        short_title: "C3-4. THEIR FINEST HOUR",
        full_title: "CAMPAIGN 3 MISSION 4: THEIR FINEST HOUR (SP_3.4)",
        co_left_portrait: "co_sleiqo_h",
        co_right_portrait: "co_l_ferrok_h",
        co_matchup: "CO: Empress Lei-Qo & Young Herman vs. Lord Ferrok (Iron Legion)",
        default_briefing: "The climax of the Lightning Wars! Unite the Solar Empire and Western Frontier battalions to defeat Lord Ferrok once and for all.",
        fe_string_idx: Some(1144),
    },
    CampaignMissionMeta {
        code: "SP_4.1",
        short_title: "C4-1. XYLVANIAN INCURSION",
        full_title: "CAMPAIGN 4 MISSION 1: XYLVANIAN INCURSION (SP_4.1)",
        co_left_portrait: "co_tnova_h_2",
        co_right_portrait: "co_xubel_h_2",
        co_matchup: "CO: Marshal Nova & Tsar Gorgi (Tundran) vs. Kommander Ubel (Xylvania)",
        default_briefing: "Kaiser Vlad's Xylvanian legions have invaded the frozen Tundran Territories! Marshal Nova must rally the Tundran Red Army to repel the invaders.",
        fe_string_idx: Some(1145),
    },
    CampaignMissionMeta {
        code: "SP_4.2",
        short_title: "C4-2. ACROSS THE FROZEN SEA",
        full_title: "CAMPAIGN 4 MISSION 2: ACROSS THE FROZEN SEA (SP_4.2)",
        co_left_portrait: "co_tnova_h_2",
        co_right_portrait: "co_xvlad_h_2",
        co_matchup: "CO: Marshal Nova (Tundran Territories) vs. Kaiser Vlad (Xylvania)",
        default_briefing: "Command the Tundran arctic fleet and amphibious task force across icy waters to recapture key naval and coastal installations.",
        fe_string_idx: Some(1146),
    },
    CampaignMissionMeta {
        code: "SP_4.3",
        short_title: "C4-3. EVICT THE INVADERS",
        full_title: "CAMPAIGN 4 MISSION 3: EVICT THE INVADERS (SP_4.3)",
        co_left_portrait: "co_tgorgi_h_2",
        co_right_portrait: "co_xvlad_h_2",
        co_matchup: "CO: Tsar Gorgi & Marshal Nova vs. Kaiser Vlad & Kommander Ubel",
        default_briefing: "Launch a full-scale Tundran counter-offensive with Heavy Tanks, Battlestations, and Gunships to drive Xylvania out of Tundran territory!",
        fe_string_idx: Some(1147),
    },
    CampaignMissionMeta {
        code: "SP_5.1",
        short_title: "C5-1. BREACH THE BORDER",
        full_title: "CAMPAIGN 5 MISSION 1: BREACH THE BORDER (SP_5.1)",
        co_left_portrait: "co_wfherman_h_2",
        co_right_portrait: "co_xubel_h_2",
        co_matchup: "CO: General Herman & Brigadier Betty vs. Kommander Ubel (Xylvania)",
        default_briefing: "The Alliance of Nations strikes back! Punch through the Xylvanian border defenses and establish a forward base in Kaiser Vlad's homeland.",
        fe_string_idx: Some(1148),
    },
    CampaignMissionMeta {
        code: "SP_5.2",
        short_title: "C5-2. SEARING THE STEPPES",
        full_title: "CAMPAIGN 5 MISSION 2: SEARING THE STEPPES (SP_5.2)",
        co_left_portrait: "co_sleiqo_h",
        co_right_portrait: "co_xvlad_h_2",
        co_matchup: "CO: Empress Lei-Qo & Marshal Nova vs. Kaiser Vlad (Xylvania)",
        default_briefing: "Advance across the scorched Xylvanian steppes and destroy the enemy's heavy artillery and armored divisions.",
        fe_string_idx: Some(1149),
    },
    CampaignMissionMeta {
        code: "SP_5.3",
        short_title: "C5-3. RACE FOR THE MINES",
        full_title: "CAMPAIGN 5 MISSION 3: RACE FOR THE MINES (SP_5.3)",
        co_left_portrait: "co_wfbetty_h_2",
        co_right_portrait: "co_xvlad_h_2",
        co_matchup: "CO: Brigadier Betty & Commander Pierce vs. Kaiser Vlad (Xylvania)",
        default_briefing: "Stop Kaiser Vlad's mining operation in the Xylvanian mountains before he can excavate the ancient weapon!",
        fe_string_idx: Some(1150),
    },
    CampaignMissionMeta {
        code: "SP_5.4",
        short_title: "C5-4. CRUSHING THE KAISER",
        full_title: "CAMPAIGN 5 MISSION 4: CRUSHING THE KAISER (SP_5.4)",
        co_left_portrait: "co_s_aqira_h",
        co_right_portrait: "co_xvlad_h_2",
        co_matchup: "CO: Allied Commanders vs. Kaiser Vlad & Kommander Ubel (Xylvania)",
        default_briefing: "The grand finale of Battalion Wars 2! Lead the combined armies of the Alliance in the final showdown against Kaiser Vlad's fortress!",
        fe_string_idx: Some(1151),
    },
];

pub const MULTIPLAYER_MAPS: [CampaignMissionMeta; 16] = [
    CampaignMissionMeta {
        code: "MP1",
        short_title: "MP1: BRIDGE TOO FAR (SKIRMISH)",
        full_title: "MULTIPLAYER MAP 1: BRIDGE TOO FAR (MP1)",
        co_left_portrait: "co_tnova_h_2",
        co_right_portrait: "co_xvlad_h_2",
        co_matchup: "Skirmish Mode — Tundran Territories vs. Xylvania",
        default_briefing: "Head-to-head tactical battle across contested river bridges and fortified riverbanks.",
        fe_string_idx: None,
    },
    CampaignMissionMeta {
        code: "MP2",
        short_title: "MP2: THE ANGLO ISLES (ASSAULT)",
        full_title: "MULTIPLAYER MAP 2: THE ANGLO ISLES (MP2)",
        co_left_portrait: "co_ai_pierce_h",
        co_right_portrait: "co_s_aqira_h",
        co_matchup: "Assault Mode — Anglo Isles vs. Solar Empire",
        default_briefing: "Amphibious island siege pitting Anglo Isles defenders against a Solar Empire strike force.",
        fe_string_idx: None,
    },
    CampaignMissionMeta {
        code: "MP3",
        short_title: "MP3: FRIENDLY FIRE (SKIRMISH)",
        full_title: "MULTIPLAYER MAP 3: FRIENDLY FIRE (MP3)",
        co_left_portrait: "co_wfherman_h_2",
        co_right_portrait: "co_tgorgi_h_2",
        co_matchup: "Skirmish Mode — Western Frontier vs. Tundran Territories",
        default_briefing: "Combined-arms skirmish between Western Frontier and Tundran armored battalions.",
        fe_string_idx: None,
    },
    CampaignMissionMeta {
        code: "MP4",
        short_title: "MP4: WINDBREAK RIDGE (ASSAULT)",
        full_title: "MULTIPLAYER MAP 4: RAID ON WINDBREAK RIDGE (MP4)",
        co_left_portrait: "co_wfbetty_h_2",
        co_right_portrait: "co_xvlad_h_2",
        co_matchup: "Assault Mode — Western Frontier vs. Xylvania",
        default_briefing: "Storm the fortified ridge positions and destroy the defending Xylvanian stronghold.",
        fe_string_idx: None,
    },
    CampaignMissionMeta {
        code: "MP5",
        short_title: "MP5: COLD RECEPTION (CO-OP)",
        full_title: "MULTIPLAYER MAP 5: COLD RECEPTION (MP5)",
        co_left_portrait: "co_tnova_h_2",
        co_right_portrait: "co_xubel_h_2",
        co_matchup: "Co-Op Mode — Tundran Territories vs. Xylvania",
        default_briefing: "Coordinate infantry, armor, and air support across frozen terrain to crush the Xylvanian garrison.",
        fe_string_idx: None,
    },
    CampaignMissionMeta {
        code: "MP6",
        short_title: "MP6: CANNON FODDER (SKIRMISH)",
        full_title: "MULTIPLAYER MAP 6: CANNON FODDER (MP6)",
        co_left_portrait: "co_aiwindsor_h",
        co_right_portrait: "co_sleiqo_h",
        co_matchup: "Skirmish Mode — Anglo Isles vs. Solar Empire",
        default_briefing: "Naval and coastal skirmish for control of island docks and artillery positions.",
        fe_string_idx: None,
    },
    CampaignMissionMeta {
        code: "MP7",
        short_title: "MP7: GRIM DETERMINATION (ASSAULT)",
        full_title: "MULTIPLAYER MAP 7: GRIM DETERMINATION (MP7)",
        co_left_portrait: "co_xvlad_h_2",
        co_right_portrait: "co_tgorgi_h_2",
        co_matchup: "Assault Mode — Xylvania vs. Tundran Territories",
        default_briefing: "Break through heavily entrenched Tundran defensive lines in a timed siege.",
        fe_string_idx: None,
    },
    CampaignMissionMeta {
        code: "MP8",
        short_title: "MP8: FISTFUL OF STEEL (SKIRMISH)",
        full_title: "MULTIPLAYER MAP 8: FISTFUL OF STEEL (MP8)",
        co_left_portrait: "co_wfherman_h_2",
        co_right_portrait: "co_xvlad_h_2",
        co_matchup: "Skirmish Mode — Western Frontier vs. Xylvania",
        default_briefing: "Heavy tank and battlestation clash over strategic factories and capture points.",
        fe_string_idx: None,
    },
    CampaignMissionMeta {
        code: "MP9",
        short_title: "MP9: HOT PURSUIT (SKIRMISH)",
        full_title: "MULTIPLAYER MAP 9: HOT PURSUIT (MP9)",
        co_left_portrait: "co_s_aqira_h",
        co_right_portrait: "co_ai_pierce_h",
        co_matchup: "Skirmish Mode — Solar Empire vs. Anglo Isles",
        default_briefing: "Fast-paced island skirmish featuring hover tanks, gunships, and amphibious landings.",
        fe_string_idx: None,
    },
    CampaignMissionMeta {
        code: "MP10",
        short_title: "MP10: LINES OF COMM. (SKIRMISH)",
        full_title: "MULTIPLAYER MAP 10: LINES OF COMMUNICATION (MP10)",
        co_left_portrait: "co_wfaustin_h_2",
        co_right_portrait: "co_tnova_h_2",
        co_matchup: "Skirmish Mode — Western Frontier vs. Tundran Territories",
        default_briefing: "Capture and hold vital communications bases and supply lines across the valley.",
        fe_string_idx: None,
    },
    CampaignMissionMeta {
        code: "MP11",
        short_title: "MP11: CROSSING THE DITCH (SKIRMISH)",
        full_title: "MULTIPLAYER MAP 11: CROSSING THE DITCH (MP11)",
        co_left_portrait: "co_ai_pierce_h",
        co_right_portrait: "co_xvlad_h_2",
        co_matchup: "Skirmish Mode — Anglo Isles vs. Xylvania",
        default_briefing: "Air and sea battle across a treacherous channel separating two fortified bases.",
        fe_string_idx: None,
    },
    CampaignMissionMeta {
        code: "MP12",
        short_title: "MP12: STORM THE BEACH (ASSAULT)",
        full_title: "MULTIPLAYER MAP 12: STORM THE BEACH (MP12)",
        co_left_portrait: "co_wfherman_h_2",
        co_right_portrait: "co_xubel_h_2",
        co_matchup: "Assault Mode — Western Frontier vs. Xylvania",
        default_briefing: "Amphibious beach landing under heavy coastal artillery and pillbox fire.",
        fe_string_idx: None,
    },
    CampaignMissionMeta {
        code: "MP13",
        short_title: "MP13: LIGHTNING WARS (ASSAULT)",
        full_title: "MULTIPLAYER MAP 13: THE LIGHTNING WARS (MP13)",
        co_left_portrait: "co_sleiqo_h",
        co_right_portrait: "co_l_ferrok_h",
        co_matchup: "Assault Mode — Solar Empire vs. Iron Legion",
        default_briefing: "Relive the ancient war on the Lightning Isle in an all-out assault against the Iron Legion!",
        fe_string_idx: None,
    },
    CampaignMissionMeta {
        code: "MP14",
        short_title: "MP14: CLEANING UP (CO-OP)",
        full_title: "MULTIPLAYER MAP 14: CLEANING UP (MP14)",
        co_left_portrait: "co_sleiqo_h",
        co_right_portrait: "co_xvlad_h_2",
        co_matchup: "Co-Op Mode — Solar Empire vs. Xylvania",
        default_briefing: "Cooperative strike mission to eradicate Xylvanian outposts and chemical defenses.",
        fe_string_idx: None,
    },
    CampaignMissionMeta {
        code: "MP15",
        short_title: "MP15: DESERT DUEL (SKIRMISH)",
        full_title: "MULTIPLAYER MAP 15: DESERT DUEL (MP15)",
        co_left_portrait: "co_aiwindsor_h",
        co_right_portrait: "co_wfbetty_h_2",
        co_matchup: "Skirmish Mode — Anglo Isles vs. Western Frontier",
        default_briefing: "Armored desert warfare over oasis capture points and airfields.",
        fe_string_idx: None,
    },
    CampaignMissionMeta {
        code: "MP17",
        short_title: "MP17: BAY OF DREAD (SKIRMISH)",
        full_title: "MULTIPLAYER MAP 17: BAY OF DREAD (MP17)",
        co_left_portrait: "co_s_aqira_h",
        co_right_portrait: "co_ai_pierce_h",
        co_matchup: "Skirmish Mode — Solar Empire vs. Anglo Isles",
        default_briefing: "Naval dreadnought, submarine, and battleship showdown in the Bay of Dread.",
        fe_string_idx: None,
    },
];

pub struct DossierUnitMeta {
    pub name: &'static str,
    pub icon: &'static str,
    pub category: &'static str,
    pub weapon: &'static str,
    pub desc: &'static str,
}

pub struct FactionDossierMeta {
    pub title: &'static str,
    pub logo: &'static str,
    pub co_portrait: &'static str,
    pub commanders: &'static str,
    pub units: &'static [DossierUnitMeta],
}

pub const FACTION_DOSSIERS: [FactionDossierMeta; 6] = [
    FactionDossierMeta {
        title: "1. SOLAR EMPIRE",
        logo: "logo_s_01",
        co_portrait: "co_sleiqo_h",
        commanders: "Empress Lei-Qo & Admiral A-Qira",
        units: &[
            DossierUnitMeta {
                name: "Solar Empire Grunt (Rifle Vet)",
                icon: "s_grunt01",
                category: "Infantry",
                weapon: "Rapid-Fire Plasma Rifle",
                desc: "Disciplined frontline infantry of the Solar Empire armed with high-velocity plasma carbines.",
            },
            DossierUnitMeta {
                name: "Solar Flame Vet",
                icon: "s_hose01",
                category: "Specialist Infantry",
                weapon: "Plasma Projector",
                desc: "Close-quarters shock trooper capable of incinerating infantry squads and clearing bunkers.",
            },
            DossierUnitMeta {
                name: "Solar Bazooka Vet",
                icon: "s_antiarm01",
                category: "Anti-Armor Infantry",
                weapon: "Guided Plasma Rocket",
                desc: "Heavy anti-armor specialist whose plasma rockets punch through enemy tanks and battlestations.",
            },
            DossierUnitMeta {
                name: "Solar Anti-Air Vet",
                icon: "s_antiair01",
                category: "Anti-Air Infantry",
                weapon: "Homing Surface-to-Air MissilePod",
                desc: "Locks onto enemy gunships, fighters, and bombers with multi-tube homing missiles.",
            },
            DossierUnitMeta {
                name: "Solar Assault Vet (HMG)",
                icon: "s_hmg01",
                category: "Heavy Infantry",
                weapon: "Rotary PlasmaRepeater",
                desc: "Suppresses enemy infantry formations and light vehicles with sustained plasma fire.",
            },
            DossierUnitMeta {
                name: "Solar Mortar Vet",
                icon: "s_mort01",
                category: "Indirect Fire Infantry",
                weapon: "Arcing Plasma Mortar",
                desc: "Lobs high-explosive plasma shells over obstacles to destroy entrenched defenses.",
            },
            DossierUnitMeta {
                name: "Solar Light Recon",
                icon: "s_lrec01",
                category: "Recon Vehicle",
                weapon: "Twin Plasma Repeater",
                desc: "High-speed hover recon craft built for rapid flanking and scouting.",
            },
            DossierUnitMeta {
                name: "Solar Light Tank",
                icon: "s_ltnk01",
                category: "Armored Vehicle",
                weapon: "Light Plasma Cannon",
                desc: "Agile hover tank combining magnetic levitation mobility with anti-armor firepower.",
            },
            DossierUnitMeta {
                name: "Solar Heavy Tank",
                icon: "s_htnk01",
                category: "Heavy Armor",
                weapon: "Twin Heavy Plasma Cannons",
                desc: "Main battle hover-tank of the Imperial Guard, heavily shielded against enemy shells.",
            },
            DossierUnitMeta {
                name: "Solar Anti-Air Vehicle",
                icon: "s_aa01",
                category: "Mobile Air Defense",
                weapon: "Quad AA Plasma Array",
                desc: "Mobile air-defense platform that shreds enemy bombers and gunships.",
            },
            DossierUnitMeta {
                name: "Solar Artillery",
                icon: "s_art01",
                category: "Long-Range Siege",
                weapon: "Long-Range Plasma Howitzer",
                desc: "Delivers devastating long-range plasma barrages against enemy fortifications and ships.",
            },
            DossierUnitMeta {
                name: "Solar Battlestation",
                icon: "s_bsta01",
                category: "Super-Heavy Armor",
                weapon: "Heavy Siege Cannon & Turrets",
                desc: "Mobile fortress bristling with plasma turrets and thick imperial plating.",
            },
            DossierUnitMeta {
                name: "Solar Gunship",
                icon: "s_gshp01",
                category: "Attack Rotorcraft",
                weapon: "Homing Air-to-Surface Rockets",
                desc: "agile VTOL gunship ideal for hunting enemy armor and supporting island landings.",
            },
            DossierUnitMeta {
                name: "Solar Fighter",
                icon: "s_fght01",
                category: "Air Superiority",
                weapon: "Interceptor Plasma Missiles",
                desc: "Supersonic interceptor built to sweep enemy bombers and gunships from the skies.",
            },
            DossierUnitMeta {
                name: "Stratodestroyer",
                icon: "s_strato",
                category: "Strategic Heavy Bomber",
                weapon: "Heavy Plasma Carpet Bombs",
                desc: "High-altitude strategic bomber capable of leveling entire enemy strongholds.",
            },
            DossierUnitMeta {
                name: "Solar Frigate",
                icon: "s_frig01",
                category: "Naval Escort",
                weapon: "Rapid Anti-Air / Anti-Sub Depth Charges",
                desc: "Fast naval escort vessel protecting the Imperial fleet from submarines and aircraft.",
            },
            DossierUnitMeta {
                name: "Solar Submarine",
                icon: "s_sub01",
                category: "Stealth Naval",
                weapon: "Heavy Plasma Torpedoes",
                desc: "Submersible hunter-killer that ambushes enemy battleships and dreadnoughts from below.",
            },
            DossierUnitMeta {
                name: "Solar Battleship & Dreadnought",
                icon: "s_dred01",
                category: "Capital Warship",
                weapon: "Long-Range Naval Bombardment Batteries",
                desc: "Flagship naval artillery platform capable of bombarding coastal targets from offshore.",
            },
        ],
    },
    FactionDossierMeta {
        title: "2. ANGLO ISLES",
        logo: "logo_ai",
        co_portrait: "co_ai_pierce_h",
        commanders: "Commander Pierce & Colonel Windsor",
        units: &[
            DossierUnitMeta {
                name: "Anglo Isles Grunt",
                icon: "icon_anglogrunt",
                category: "Infantry",
                weapon: "Standard-Issue Service Rifle",
                desc: "Steadfast infantryman of the Anglo Isles Expeditionary Force.",
            },
            DossierUnitMeta {
                name: "Anglo Flame Vet",
                icon: "ai_flame_vet",
                category: "Specialist Infantry",
                weapon: "Backpack Flamethrower",
                desc: "Clears enemy trenches and fortifications with pressurized incendiary fuel.",
            },
            DossierUnitMeta {
                name: "Anglo Bazooka Vet",
                icon: "ai_bazooka_vet",
                category: "Anti-Armor Infantry",
                weapon: "Anti-Tank Rocket Launcher",
                desc: "Delivers armor-piercing rockets against Solar Empire hover tanks.",
            },
            DossierUnitMeta {
                name: "Anglo Anti-Air Vet",
                icon: "ai_aa_vet",
                category: "Anti-Air Infantry",
                weapon: "Heat-Seeking SAM Launcher",
                desc: "Defends Anglo beachheads from Solar Gunships and Fighters.",
            },
            DossierUnitMeta {
                name: "Anglo Assault Vet (HMG)",
                icon: "ai_hmg_vet",
                category: "Heavy Infantry",
                weapon: "Heavy Vickers Machine Gun",
                desc: "High-rate-of-fire infantry support weapon lethal against enemy foot soldiers.",
            },
            DossierUnitMeta {
                name: "Anglo Mortar Vet",
                icon: "ai_mortar_vet",
                category: "Indirect Fire Infantry",
                weapon: "Trench Mortar",
                desc: "Rains high-explosive shells onto enemy positions from behind cover.",
            },
            DossierUnitMeta {
                name: "Anglo Light Tank",
                icon: "ai_lt_tank",
                category: "Armored Vehicle",
                weapon: "Quick-Firing Tank Gun",
                desc: "Fast cruiser tank spearheading Anglo amphibious landings across the Solar islands.",
            },
            DossierUnitMeta {
                name: "Anglo Heavy Tank",
                icon: "ai_hv_tank",
                category: "Heavy Armor",
                weapon: "Dual Heavy Turret Cannon",
                desc: "Heavily armored breakthrough tank deployed in Colonel Windsor's palace assault.",
            },
            DossierUnitMeta {
                name: "Anglo Artillery",
                icon: "ai_artillery",
                category: "Long-Range Siege",
                weapon: "Self-Propelled Howitzer",
                desc: "Long-range mobile artillery used to shell Bongo Island and the Imperial Palace.",
            },
            DossierUnitMeta {
                name: "Anglo Anti-Air Vehicle",
                icon: "ai_anti_air",
                category: "Mobile Air Defense",
                weapon: "Twin Flak Cannons",
                desc: "Armored mobile flak gun guarding Anglo armor columns against air attack.",
            },
            DossierUnitMeta {
                name: "Anglo Fighter",
                icon: "ai_fighter",
                category: "Air Superiority",
                weapon: "Air-to-Air Rockets & Cannons",
                desc: "High-speed propeller/jet hybrid fighter patrolling the skies over the archipelago.",
            },
            DossierUnitMeta {
                name: "Anglo Bomber",
                icon: "ai_bomber",
                category: "Tactical Bomber",
                weapon: "High-Explosive Bomb Bay",
                desc: "Heavy twin-fuselage bomber deployed against Solar Empire installations.",
            },
            DossierUnitMeta {
                name: "Anglo Transport Chopper",
                icon: "ai_trans_chop",
                category: "Air Transport",
                weapon: "Troop & Vehicle Airlift",
                desc: "Heavy-lift helicopter that rapidly deploys Anglo reinforcements onto island beaches.",
            },
            DossierUnitMeta {
                name: "Anglo Battleship & Dreadnought",
                icon: "ai_battleship",
                category: "Capital Warship",
                weapon: "16-Inch Naval Gun Batteries",
                desc: "Backbone of Commander Pierce's Royal Navy task force.",
            },
            DossierUnitMeta {
                name: "Anglo Frigate & Submarine",
                icon: "ai_frigate",
                category: "Naval Escort & Sub",
                weapon: "Deck Guns, Depth Charges & Torpedoes",
                desc: "Screens the Anglo fleet and hunts opposing naval vessels.",
            },
            DossierUnitMeta {
                name: "Anglo Naval Transport",
                icon: "ai_lan_trans",
                category: "Amphibious Landing Craft",
                weapon: "Bow Ramp Armor Deployment",
                desc: "Delivers tanks and infantry directly onto contested island shorelines.",
            },
        ],
    },
    FactionDossierMeta {
        title: "3. WESTERN FRONTIER",
        logo: "logo_f_01",
        co_portrait: "co_wf_yherman_h",
        commanders: "General Herman, Brigadier Betty & Colonel Austin",
        units: &[
            DossierUnitMeta {
                name: "Western Frontier Grunt",
                icon: "wf_grunt01",
                category: "Infantry",
                weapon: "Automatic Assault Rifle",
                desc: "Versatile GI infantryman of General Herman's allied garrison on Bongo Island.",
            },
            DossierUnitMeta {
                name: "Frontier Flame Vet",
                icon: "wf_hose01",
                category: "Specialist Infantry",
                weapon: "Napalm Thrower",
                desc: "Assault engineer equipped for bunker-busting and close combat.",
            },
            DossierUnitMeta {
                name: "Frontier Bazooka Vet",
                icon: "wf_antiarm01",
                category: "Anti-Armor Infantry",
                weapon: "M9 Anti-Tank Bazooka",
                desc: "Hard-hitting anti-tank trooper capable of stopping enemy armor columns.",
            },
            DossierUnitMeta {
                name: "Frontier Anti-Air & HMG Vets",
                icon: "wf_hmg01",
                category: "Heavy Support Infantry",
                weapon: "Heavy Machine Gun & Stinger SAM",
                desc: "Provides sustained anti-infantry and anti-aircraft fire support.",
            },
            DossierUnitMeta {
                name: "Frontier Light & Heavy Recon",
                icon: "wf_lrec01",
                category: "Recon Vehicle",
                weapon: ".50 Cal Turret",
                desc: "Rugged all-terrain jeep and armored car for rapid battlefield response.",
            },
            DossierUnitMeta {
                name: "Frontier Light & Heavy Tanks",
                icon: "wf_htnk01",
                category: "Armored Vehicle",
                weapon: "High-Velocity Tank Cannon",
                desc: "Dependable tracked armor serving alongside the Solar Empire in Campaign 1.",
            },
            DossierUnitMeta {
                name: "Frontier Battlestation & Artillery",
                icon: "wf_bsta01",
                category: "Heavy Siege Armor",
                weapon: "Heavy Siege Howitzer & MG Sponsons",
                desc: "Rolling fortress built to anchor front-line defenses.",
            },
            DossierUnitMeta {
                name: "Frontier Gunship, Fighter & Bomber",
                icon: "wf_gshp01",
                category: "Air Wing",
                weapon: "Hellfire Rockets & Heavy Bombs",
                desc: "Versatile air wing providing close air support and strategic bombing.",
            },
            DossierUnitMeta {
                name: "Frontier Naval Fleet",
                icon: "wf_bshp01",
                category: "Naval Task Force",
                weapon: "Naval Guns, Torpedoes & Depth Charges",
                desc: "Ocean-going Frigates, Submarines, Battleships, and Dreadnoughts.",
            },
        ],
    },
    FactionDossierMeta {
        title: "4. IRON LEGION (UNDERWORLD)",
        logo: "logo_u_01",
        co_portrait: "co_l_ferrok_h",
        commanders: "Lord Ferrok (200 Years Ago - Lightning Wars)",
        units: &[
            DossierUnitMeta {
                name: "Iron Legion Grunt",
                icon: "u_grunt01",
                category: "Infantry",
                weapon: "Heavy Bolt Rifle",
                desc: "Relentless shock infantry of Lord Ferrok's ancient Iron Legion.",
            },
            DossierUnitMeta {
                name: "Iron Legion Flamer",
                icon: "u_hose01",
                category: "Specialist Infantry",
                weapon: "Alchemical Flame Projector",
                desc: "Heavy-armored pyromancer trooper deployed in the trenches of the Lightning Isle.",
            },
            DossierUnitMeta {
                name: "Iron Legion Bazooka & Mortar",
                icon: "u_antiarm01",
                category: "Heavy Ordnance Infantry",
                weapon: "Armor-Piercing Rocket & Siege Mortar",
                desc: "Bombards opposing forces with heavy explosive rockets and shells.",
            },
            DossierUnitMeta {
                name: "Iron Legion Heavy Tank",
                icon: "u_htnk01",
                category: "Heavy Armor",
                weapon: "Twin Siege Cannons",
                desc: "Riveted ironclad behemoth that crushes trench lines beneath its treads.",
            },
            DossierUnitMeta {
                name: "Iron Legion Battlestation",
                icon: "u_bsta01",
                category: "Super-Heavy Fortress Tank",
                weapon: "Heavy Demolition Cannon & Sponson Guns",
                desc: "Lord Ferrok's feared mobile fortress, nearly impervious to small-arms fire.",
            },
            DossierUnitMeta {
                name: "Iron Legion Gunship & Bomber",
                icon: "u_gshp01",
                category: "Air Support",
                weapon: "Heavy Rockets & Iron Bombs",
                desc: "Armored rotorcraft and bombers that darken the skies over the Lightning Isle.",
            },
        ],
    },
    FactionDossierMeta {
        title: "5. TUNDRAN TERRITORIES",
        logo: "logo_t_01",
        co_portrait: "co_tgorgi_h_2",
        commanders: "Tsar Gorgi & Marshal Nova",
        units: &[
            DossierUnitMeta {
                name: "Tundran Rifle, Flame & Bazooka Vets",
                icon: "t_grunt01",
                category: "Infantry",
                weapon: "Assault Rifle, Flamethrower & RPG",
                desc: "Hardy winter-trained infantry of the Tundran Red Army.",
            },
            DossierUnitMeta {
                name: "Tundran Heavy Tank & Battlestation",
                icon: "t_htnk01",
                category: "Heavy Armor",
                weapon: "Heavy Mammoth Cannon",
                desc: "Thickly sloped armor designed for head-on breakthroughs across frozen plains.",
            },
            DossierUnitMeta {
                name: "Tundran Gunship, Fighter & Bomber",
                icon: "t_gshp01",
                category: "Air Wing",
                weapon: "Rocket Pods & Heavy Ordnance",
                desc: "Heavily armored gunships and strategic bombers of the Tundran Air Force.",
            },
            DossierUnitMeta {
                name: "Tundran Naval Fleet",
                icon: "t_dred01",
                category: "Arctic Fleet",
                weapon: "Heavy Naval Artillery & Torpedoes",
                desc: "Ice-breaking Dreadnoughts, Battleships, Frigates, and Submarines.",
            },
        ],
    },
    FactionDossierMeta {
        title: "6. XYLVANIA",
        logo: "logo_x_01",
        co_portrait: "co_xvlad_h_2",
        commanders: "Kaiser Vlad & Kommander Ubel",
        units: &[
            DossierUnitMeta {
                name: "Xylvanian Grunt, Flame & Bazooka Vets",
                icon: "x_grunt01",
                category: "Infantry",
                weapon: "Repeating Rifle, Acid/Flame & Panzerschreck",
                desc: "Kaiser Vlad's regimented infantry legions.",
            },
            DossierUnitMeta {
                name: "Xylvanian Heavy Tank & Battlestation",
                icon: "x_htnk01",
                category: "Heavy Armor",
                weapon: "Twin Krupp Siege Guns",
                desc: "Gothic steel war machines engineered for maximum firepower.",
            },
            DossierUnitMeta {
                name: "Xylvanian Air & Naval Forces",
                icon: "x_dred01",
                category: "Air & Sea Armada",
                weapon: "Heavy Bombs, Torpedoes & Naval Batteries",
                desc: "Xylvanian Gunships, Fighters, Bombers, Submarines, and Dreadnoughts.",
            },
        ],
    },
];

pub struct ConceptPlateMeta {
    pub title: &'static str,
    pub left_tex: &'static str,
    pub right_tex: &'static str,
    pub desc: &'static str,
}

pub const CONCEPT_GALLERY_PLATES: [ConceptPlateMeta; 6] = [
    ConceptPlateMeta {
        title: "1. SOLAR EMPIRE COMMANDERS & PALACE",
        left_tex: "se_c_co",
        right_tex: "c_se_scene01",
        desc: "Original production concept art for Empress Lei-Qo, Admiral A-Qira, and the Solar Empire's futuristic plasma-powered architecture (from fe_concept_Level.res).",
    },
    ConceptPlateMeta {
        title: "2. ANGLO ISLES COMMANDERS & INVASION",
        left_tex: "ai_c_co",
        right_tex: "c_ai_scene01",
        desc: "Original production concept art for Commander Pierce, Colonel Windsor, and the Anglo Isles amphibious task force.",
    },
    ConceptPlateMeta {
        title: "3. WESTERN FRONTIER COMMANDERS & BASE",
        left_tex: "wf_c_co",
        right_tex: "c_wf_scene01",
        desc: "Original production concept art for General Herman, Brigadier Betty, Colonel Austin, and Western Frontier mechanized forces.",
    },
    ConceptPlateMeta {
        title: "4. IRON LEGION & LIGHTNING ISLE",
        left_tex: "il_c_co",
        right_tex: "c_il_scene01",
        desc: "Original production concept art for Lord Ferrok, Iron Battlestations, and the ancient Lightning Wars of 200 years ago.",
    },
    ConceptPlateMeta {
        title: "5. TUNDRAN TERRITORIES & XYLVANIA",
        left_tex: "t_c_co",
        right_tex: "x_c_co",
        desc: "Original production concept art for Tsar Gorgi, Marshal Nova, Kaiser Vlad, and Kommander Ubel.",
    },
    ConceptPlateMeta {
        title: "6. SOLAR INFANTRY & IMPERIAL DREADNOUGHT",
        left_tex: "extrasolconcept",
        right_tex: "extradredconcept",
        desc: "Original production concept illustrations showcasing Solar Empire shock infantry and the massive Imperial Dreadnought.",
    },
];

pub struct FmvMovieMeta {
    pub filename: &'static str,
    pub title: &'static str,
    pub desc: &'static str,
}

pub const FMV_MOVIES: [FmvMovieMeta; 15] = [
    FmvMovieMeta {
        filename: "BWii_Intro.thp",
        title: "1. BATTALION WARS 2 INTRO",
        desc: "Opening theatrical intro cinematic for Battalion Wars 2 (Data/Video/BWii_Intro.thp).",
    },
    FmvMovieMeta {
        filename: "0.1_Prologue_1.THP",
        title: "2. PROLOGUE INTRO: 200 YEARS AGO",
        desc: "Empress Lei-Qo and Lord Ferrok clash on the Lightning Isle (Data/Video/0.1_Prologue_1.THP).",
    },
    FmvMovieMeta {
        filename: "0.2_Prologue_2.THP",
        title: "3. PROLOGUE OUTRO: THE SOLAR STAFF",
        desc: "The Solar Staff unleashes the orbital beam against the Iron Tower (Data/Video/0.2_Prologue_2.THP).",
    },
    FmvMovieMeta {
        filename: "1.1_Anglo_Attack.THP",
        title: "4. CAMPAIGN 1 INTRO: ANGLO ATTACK",
        desc: "Commander Pierce and Colonel Windsor launch the invasion of the Solar Empire (Data/Video/1.1_Anglo_Attack.THP).",
    },
    FmvMovieMeta {
        filename: "1.2_Tundra_Joins.THP",
        title: "5. CAMPAIGN 1 OUTRO: TUNDRA JOINS",
        desc: "Admiral A-Qira and General Herman repel the Anglo Isles fleet (Data/Video/1.2_Tundra_Joins.THP).",
    },
    FmvMovieMeta {
        filename: "2.1_Learning_From_History.THP",
        title: "6. CAMPAIGN 2 INTRO: LEARNING FROM HISTORY",
        desc: "Flashback to the Anglo Isles pre-emptive assault on the Solar Empire (Data/Video/2.1_Learning_From_History.THP).",
    },
    FmvMovieMeta {
        filename: "2.2_A_Necessary_Peace.THP",
        title: "7. CAMPAIGN 2 OUTRO: A NECESSARY PEACE",
        desc: "The Anglo Isles and Solar Empire forge an alliance (Data/Video/2.2_A_Necessary_Peace.THP).",
    },
    FmvMovieMeta {
        filename: "3.1_A_Dish_Served_Cold.THP",
        title: "8. CAMPAIGN 3 INTRO: A DISH SERVED COLD",
        desc: "Flashback to the Lightning Wars against Lord Ferrok's Iron Legion (Data/Video/3.1_A_Dish_Served_Cold.THP).",
    },
    FmvMovieMeta {
        filename: "3.2_The_Enemy_Revealed.THP",
        title: "9. CAMPAIGN 3 OUTRO: THE ENEMY REVEALED",
        desc: "Kaiser Vlad's Xylvanian conspiracy is unmasked (Data/Video/3.2_The_Enemy_Revealed.THP).",
    },
    FmvMovieMeta {
        filename: "4.1_We_Know_Only_War.THP",
        title: "10. CAMPAIGN 4 INTRO: WE KNOW ONLY WAR",
        desc: "Xylvania invades the frozen Tundran Territories (Data/Video/4.1_We_Know_Only_War.THP).",
    },
    FmvMovieMeta {
        filename: "4.2_The_Plot_Revealed.THP",
        title: "11. CAMPAIGN 4 OUTRO: THE PLOT REVEALED",
        desc: "Marshal Nova and Tsar Gorgi uncover Vlad's endgame (Data/Video/4.2_The_Plot_Revealed.THP).",
    },
    FmvMovieMeta {
        filename: "5.1_Enemy_At_The_Gates.THP",
        title: "12. CAMPAIGN 5 INTRO: ENEMY AT THE GATES",
        desc: "The Allied Nations invade Xylvania to stop Kaiser Vlad (Data/Video/5.1_Enemy_At_The_Gates.THP).",
    },
    FmvMovieMeta {
        filename: "5.2_The_Reckoning.THP",
        title: "13. CAMPAIGN 5 OUTRO: THE RECKONING",
        desc: "Grand finale cinematic of Battalion Wars 2 (Data/Video/5.2_The_Reckoning.THP).",
    },
    FmvMovieMeta {
        filename: "AttractMode.thp",
        title: "14. ATTRACT MODE SHOWCASE",
        desc: "Original attract-mode gameplay montage (Data/Video/AttractMode.thp).",
    },
    FmvMovieMeta {
        filename: "BWii_Intro_w.thp",
        title: "15. WIDESCREEN INTRO (16:9)",
        desc: "Widescreen theatrical intro cinematic (Data/Video/BWii_Intro_w.thp).",
    },
];

#[derive(Clone, Copy, Debug)]
pub struct ExplosionEffect {
    pub pos: Vec3,
    pub age: f32,
    pub max_age: f32,
    pub scale: f32,
}

#[derive(Clone)]
pub struct TroopActor {
    pub model_index: usize,
    pub parts: Vec<model_render::Part>,
    pub last_pose_key: Option<(Motion, u32)>,
    pub last_pose: Vec<Mat4>,
}

#[derive(Resource)]
pub struct View {
    pub data_dir: std::path::PathBuf,
    pub save_path: Option<std::path::PathBuf>,
    pub globe_models: Vec<Model>,
    pub active_briefing_vo: Option<usize>,
    pub movie_index: usize,
    pub active_movie: Option<ActiveMoviePlayer>,
    pub current_level: String,
    pub pending_level: Option<String>,
    pub water_height: f32,
    pub skydome_name: Option<String>,
    pub session: Session,
    pub error: Option<String>,
    pub should_exit: bool,
    pub stage: FrontendStage,
    pub fe_runtime: FrontendRuntime,
    pub fe_strings: Vec<String>,
    pub credits_lines: Vec<String>,
    pub campaign_levels: Vec<String>,
    pub slots: [Option<SaveProfile>; 3],
    pub selected_slot: usize,
    pub name_buffer: String,
    pub main_menu_index: usize,
    pub selected_mission: usize,
    pub mission2_selected_notice: String,
    pub multiplayer_index: usize,
    pub options_index: usize,
    pub music_volume: f32,
    pub sfx_volume: f32,
    pub mouse_sensitivity: f32,
    pub aim_assist_enabled: bool,
    pub extras_index: usize,
    pub dossier_faction: usize,
    pub dossier_unit: usize,
    pub concept_index: usize,
    pub credits_page: usize,
    pub pause_menu_open: bool,
    pub pause_menu_index: usize,
    pub troops: BTreeMap<ObjectId, TroopActor>,
    pub clips: BTreeMap<String, Animation>,
    pub wheel_angles: BTreeMap<ObjectId, f32>,
    pub last_positions: BTreeMap<ObjectId, [f32; 3]>,
    pub smoothed_positions: BTreeMap<ObjectId, [f32; 3]>,
    pub smoothed_rotations: BTreeMap<ObjectId, Quat>,
    pub smoothed_camera: Option<(Vec3, Vec3)>,
    pub ui_textures: BTreeMap<String, texture::DecodedTexture>,
    pub ui_handles: BTreeMap<String, Handle<Image>>,
    pub music_wavs: BTreeMap<String, Vec<u8>>,
    pub music_handles: BTreeMap<String, Handle<AudioSource>>,
    pub active_music_track: Option<String>,
    pub sfx_wavs: BTreeMap<String, Vec<u8>>,
    pub sfx_handles: BTreeMap<String, Handle<AudioSource>>,
    pub last_voice_key: Option<(usize, u64)>,
    pub last_fired: u64,
    pub last_hits: u64,
    pub last_bomb_impacts: u64,
    pub last_missiles: usize,
    pub last_air_contacts: u64,
    pub last_lightning: usize,
    pub last_objective_banner: Option<String>,
    pub last_player_id: Option<ObjectId>,
    pub was_overheated: bool,
    pub was_reloading: bool,
    pub was_firing_staff: bool,
    pub was_locked: bool,
    pub footstep_distance: f32,
    pub footstep_index: usize,
    pub selected_squad_slot: usize,
    pub music_duck: f32,
    pub alive_destructibles: std::collections::BTreeSet<ObjectId>,
    pub destroyed_times: BTreeMap<ObjectId, f32>,
    pub seen_rewrite_effects: usize,
    pub active_explosions: Vec<ExplosionEffect>,
    pub vfx_fireball_materials: [Handle<StandardMaterial>; 4],
    pub vfx_smoke_materials: [Handle<StandardMaterial>; 4],
    pub locked_target: Option<ObjectId>,
    pub lock_toggled: bool,
    accumulator: f64,
    recruit_pending: bool,
    send_pending: bool,
    dismiss_pending: bool,
    mouse_delta: Vec2,
    frames: u32,
    verify: bool,
    menu_smoke: bool,
    start: Option<[f32; 3]>,
    replay: Option<VecDeque<Input>>,
    replay_check_frame: u32,
    pub smoke_frames: u32,
    ambush_frames: u32,
    staff_frames: u32,
    expect_ambush: bool,
    expect_staff: bool,
    hold_replay_end: bool,
}

#[derive(Component)]
pub struct SceneEntity;

#[derive(Component)]
pub struct FrontendGlobePart {
    pub local: Mat4,
    pub norm_scale: f32,
}

#[derive(Component)]
pub struct Part {
    id: ObjectId,
    root: Mat4,
    local: Mat4,
    grounded: bool,
    bone: Option<usize>,
    wheel_radius: Option<f32>,
    rotor_axis: Option<u8>,
    turret_role: u8,
    turret_yaw_pivot: [f32; 2],
    turret_pivot: Vec3,
    tread_mesh: Option<Handle<Mesh>>,
    tread_uvs: Option<Vec<[f32; 2]>>,
    is_fragment: bool,
    frag_vel: Vec3,
    is_water_vehicle: bool,
    is_mobile: bool,
    is_unit: bool,
    keep_when_dead: bool,
}

#[derive(Component)]
pub struct Hud;
#[derive(Component)]
pub struct Fade;
#[derive(Component)]
pub struct GoldStar(bw_rust::mission_guidance::Marker);
#[derive(Component)]
pub struct RifleTrace(usize);
#[derive(Component)]
pub struct WorldHealthBar(usize);
#[derive(Component)]
pub struct TargetLockBracket;
#[derive(Component)]
pub struct MusicTrackPlayer;
#[derive(Component)]
pub struct VoiceTrackPlayer;
#[derive(Component)]
pub struct RadarBlip(pub usize);
#[derive(Component)]
pub struct PlayerHealthBarFill;
#[derive(Component)]
pub struct TargetHealthBarFill;
#[derive(Component)]
pub struct SquadSlotHealthFill(pub usize);

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum UiPanelSlot {
    MenuRoot,
    InGameHudRoot,
    Crosshair,
    CinematicTop,
    CinematicBottom,
    CoRadio,
    RadarPanel,
    TargetInfoPill,
    SquadCommand,
    TutorialModal,
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum UiTextSlot {
    MenuHeader,
    MenuLeftTitle,
    MenuRightTitle,
    MenuRightBody,
    MenuRightStatus,
    MenuPrimaryLabel,
    MenuSecondaryLabel,
    MenuItem(usize),
    CoRadioSpeaker,
    CoRadioBody,
    ObjectiveBanner,
    RadarDistanceLabel,
    RadarNorthBadge,
    TargetEnemyLabel,
    PlayerStatus,
    PlayerHpText,
    SquadSlotCount(usize),
    SquadStatus,
    TutorialModal,
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum UiImageSlot {
    MenuPortraitLeft,
    MenuPortraitRight,
    MenuGradeBadge,
    MovieScreen,
    CoRadioPortrait,
    ReticleCenter,
    ReticleEnemyTag,
    ReticlePowerLeft,
    ReticlePowerRight,
    TargetEnemyIcon,
    PlayerUnitPortrait,
    SquadSlotHub(usize),
    SquadSlotIcon(usize),
    SquadSlotOrder(usize),
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub struct MenuItemButton(pub usize);

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum MenuActionBtn {
    Primary,
    Secondary,
}

fn pcm_to_wav(sample_rate: u32, channels: u16, pcm: &[i16]) -> Vec<u8> {
    let data_bytes = (pcm.len() * 2) as u32;
    let mut out = Vec::with_capacity(data_bytes as usize + 44);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(data_bytes + 36).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&(sample_rate * channels as u32 * 2).to_le_bytes());
    out.extend_from_slice(&(channels * 2).to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_bytes.to_le_bytes());
    for &sample in pcm {
        out.extend_from_slice(&sample.to_le_bytes());
    }
    out
}

fn stereo_to_wav(stereo: &Stereo) -> Vec<u8> {
    let len = stereo.left.pcm.len().min(stereo.right.pcm.len());
    let mut interleaved = Vec::with_capacity(len * 2);
    for i in 0..len {
        interleaved.push(stereo.left.pcm[i]);
        interleaved.push(stereo.right.pcm[i]);
    }
    pcm_to_wav(stereo.left.sample_rate, 2, &interleaved)
}

fn clean_bw_text(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' && chars.peek() == Some(&'c') {
            let _ = chars.next();
            for _ in 0..3 {
                let _ = chars.next();
            }
        } else {
            out.push(ch);
        }
    }
    out
}

fn crop_texture(
    src: &texture::DecodedTexture,
    x0: usize,
    y0: usize,
    x1: usize,
    y1: usize,
) -> texture::DecodedTexture {
    let x0 = x0.min(src.width);
    let y0 = y0.min(src.height);
    let x1 = x1.min(src.width).max(x0 + 1);
    let y1 = y1.min(src.height).max(y0 + 1);
    let w = x1 - x0;
    let h = y1 - y0;
    let mut rgba = vec![0u8; w * h * 4];
    for y in 0..h {
        let sy = y0 + y;
        if sy < src.height {
            for x in 0..w {
                let sx = x0 + x;
                if sx < src.width {
                    let sidx = (sy * src.width + sx) * 4;
                    let didx = (y * w + x) * 4;
                    rgba[didx..didx + 4].copy_from_slice(&src.rgba[sidx..sidx + 4]);
                }
            }
        }
    }
    texture::DecodedTexture {
        width: w,
        height: h,
        rgba,
    }
}

fn extract_dnos_wavs(archive: &[u8]) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    let Ok(samples) = Samples::from_archive(archive) else {
        return out;
    };
    for name in samples.names() {
        let lower = name.to_ascii_lowercase();
        if !out.contains_key(&lower)
            && let Ok(audio) = samples.decode(name)
        {
            out.insert(lower, pcm_to_wav(audio.sample_rate, 1, &audio.pcm));
        }
    }
    out
}

fn extract_water_height(graph: &ObjectGraph) -> f32 {
    for obj in graph.objects.values() {
        if obj.kind == "cRenderParams"
            && let Some(raw) = field(obj, "mWaterHeight")
            && let Ok(val) = raw.parse::<f32>()
            && val.is_finite()
        {
            return val;
        }
    }
    42.5
}

fn extract_skydome_name(graph: &ObjectGraph) -> Option<String> {
    for obj in graph.objects.values() {
        if obj.kind == "cNodeHierarchyResource"
            && let Some(name) = field(obj, "mName")
            && name.starts_with("SKY_")
        {
            return Some(name.to_string());
        }
    }
    None
}

const LEVEL_UI_TEXTURE_NAMES: &[&str] = &[
    "co_sleiqohel_h",
    "co_sleiqohel_s",
    "co_sleiqo_h",
    "co_sleiqo_s",
    "co_s_aqira_h",
    "co_s_aqira_s",
    "co_ai_pierce_h",
    "co_ai_pierce_s",
    "co_aiwindsor_h",
    "co_aiwindsor_s",
    "co_l_ferrok_h",
    "co_l_ferrok_s",
    "co_wf_yherman_h",
    "co_wf_yherman_s",
    "co_wfherman_h_2",
    "co_wfherman_s_2",
    "co_wfbetty_h_2",
    "co_wfaustin_h_2",
    "co_t_ygorgi_h",
    "co_tgorgi_h_2",
    "co_tnova_h_2",
    "co_tnelly_h_2",
    "co_xvlad_h_2",
    "co_xubel_h_2",
    "co_xubelprison_h",
    "co_dialogue_left",
    "co_dialog_lft_hi",
    "co_dialogue_mid",
    "co_dialog_rt",
    "codialogueflash",
    "icon_radwaves",
    "hud_objinner",
    "hud_objoutline",
    "hud_star",
    "object_stars",
    "radar_lense",
    "skipse_radarbez",
    "radar_n_ring03",
    "radar_vp",
    "radar_vcone",
    "radar_blip_small",
    "radar_gv",
    "radar_plane",
    "radar_chopper",
    "radar_infantry",
    "ret_regl",
    "ret_rnge",
    "ret_lock",
    "ret_look",
    "ret_2nd",
    "ret_unlocked",
    "ret_enemy",
    "power_up_01",
    "power_up_02",
    "hud_line6",
    "hud_brackets",
    "skipse_hub1",
    "skipse_hub2",
    "skipuw_hub1",
    "hud_hubglow",
    "select_stack",
    "icon_all",
    "icon_all_s",
    "hud_p1",
    "hud_healthbar",
    "icon_idtab",
    "icon_follow",
    "icon_attack",
    "icon_sentry",
    "icon_move",
    "icon_defend",
    "s_grunt01",
    "s_hose01",
    "s_antiarm01",
    "s_antiair01",
    "s_hmg01",
    "s_mort01",
    "s_lrec01",
    "s_hrec01",
    "s_ltnk01",
    "s_htnk01",
    "s_art01",
    "s_aa01",
    "s_bsta01",
    "s_gshp01",
    "s_fght01",
    "s_strato",
    "s_tcop01",
    "s_bshp01",
    "s_frig01",
    "s_sub01",
    "s_dred01",
    "s_tran",
    "u_grunt01",
    "u_hose01",
    "u_antiarm01",
    "u_htnk01",
    "u_bsta01",
    "wii_abutton",
    "wii_bbutton",
    "wii_plusbutton",
    "cursor_01",
    "pm_border",
    "pm_textbox",
    "pm_butt1",
    "pm_butt1high",
    "pm_butt2",
    "pm_butt2high",
    "hud_whiteflag",
    "blip_01",
    "sfx_fireball",
    "sfx_fireblobanim",
    "sfx_bigfireadd",
    "sfx_smoke",
    "sfx_smoke2",
    "sfx_smokeplume",
    "sfx_shock",
    "sfx_radial_glows",
    "sfx_white_glow",
    "sfx_hl_flare",
    "sfx_muzz_mg",
    "sfx_waterwake",
    "sfx_waterring",
];

impl View {
    pub fn load(
        root: &std::path::Path,
        level_name: &str,
        graph: &ObjectGraph,
        archive: &[u8],
    ) -> Result<Self> {
        let mut session = Session::load_level(root, level_name, graph, archive)?;
        let water_height = extract_water_height(graph);
        let skydome_name = extract_skydome_name(graph);
        let tail = std::env::var("BW_MISSION_REPLAY_TAIL")
            .map(|n| n.parse::<usize>().map_err(|_| "Invalid replay tail"))
            .unwrap_or(Ok(120))?;
        if !(120..=3600).contains(&tail) {
            return Err("Replay tail outside bounded range".into());
        }
        let expect_ambush = std::env::var_os("BW_MISSION_EXPECT_AMBUSH").is_some();
        let expect_staff = std::env::var_os("BW_MISSION_EXPECT_STAFF").is_some();
        let hold_replay_end = std::env::var_os("BW_MISSION_REPLAY_HOLD").is_some();
        let replay = if let Some(path) = std::env::var_os("BW_MISSION_REPLAY") {
            let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
            if bytes.len() > 16 * 1024 * 1024 {
                return Err("Oversized mission replay".into());
            }
            let inputs: Vec<Input> = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            if inputs.len() < 120 || inputs.len() > 36000 {
                return Err("Invalid mission replay length".into());
            }
            if tail > inputs.len() {
                return Err("Replay tail exceeds recording".into());
            }
            let split = inputs.len() - tail;
            for input in &inputs[..split] {
                session.tick(1. / 60., *input)?;
            }
            println!(
                "FIRST_COMBAT_REPLAY original_input_preroll={} remaining={}",
                split, tail
            );
            Some(inputs[split..].iter().copied().collect())
        } else {
            None
        };
        if replay.is_none() && (expect_ambush || expect_staff || hold_replay_end || tail != 120) {
            return Err("Replay diagnostics require a recording".into());
        }
        let verify = std::env::var_os("BW_MISSION_VERIFY").is_some();
        let menu_smoke = std::env::var_os("BW_MENU_SMOKE").is_some();
        if verify && replay.is_some() {
            return Err("Conflicting verification modes".into());
        }
        if verify {
            for _ in 0..2700 {
                session.tick(1.0 / 60.0, Input::default())?;
            }
            for i in 0..1800 {
                if !session.program.host.world.frozen {
                    break;
                }
                session.tick(
                    1.0 / 60.0,
                    Input {
                        confirm: i % 2 == 0,
                        skip_cutscene_alt: i % 2 == 0,
                        ..Input::default()
                    },
                )?;
            }
            if session.program.host.world.frozen {
                session.program.host.world.frozen = false;
            }
            if let Some(t) = &mut session.program.host.world.tutorial {
                t.paused = false;
            }
            if let Some(c) = &mut session.program.host.world.controller {
                let _ = c.call(&[
                    bw_rust::lua50::Value::Enum("CONTROL_FIRE".into()),
                    bw_rust::lua50::Value::Number(1.0),
                ]);
            }
            if let Some(pid) = session.program.host.world.player {
                if let Some(u) = session.program.host.world.units.get_mut(&pid) {
                    u.active = Some(true);
                }
                session
                    .program
                    .host
                    .world
                    .activity
                    .entry(pid)
                    .or_default()
                    .weapons_enabled = Some(true);
            }
            println!(
                "FIRST_MISSION_PREROLL original_scripts=97 simulated_seconds=45 player_unfrozen=true; not rendered cinematic acceptance"
            );
        }

        let mut clips = BTreeMap::new();
        for unit in session.program.host.world.units.values() {
            if let Some(anim) = &unit.animation {
                for clip_name in anim.bindings.values() {
                    if !clips.contains_key(clip_name) {
                        clips.insert(
                            clip_name.clone(),
                            animation::from_archive(archive, clip_name)?,
                        );
                    }
                }
            }
        }

        let data_dir = root.join("local/extracted/files/Data");
        let compound_dir = data_dir.join("CompoundFiles");
        let mut fe_graph = ObjectGraph::default();
        for name in ["Frontend2_Level.xml", "Frontend2_Level_preload.xml"] {
            let xml =
                std::fs::read_to_string(compound_dir.join(name)).map_err(|e| e.to_string())?;
            fe_graph.merge(parse_objects(&xml)?)?;
        }
        let fe_archive =
            std::fs::read(compound_dir.join("Frontend2_Level.res")).map_err(|e| e.to_string())?;
        let levels_xml =
            std::fs::read_to_string(data_dir.join("levels.xml")).map_err(|e| e.to_string())?;
        let levels = level_catalog::Catalog::parse(&levels_xml)?;
        let campaign_levels: Vec<String> = levels
            .entries
            .iter()
            .filter(|e| e.gametype == "singleplayer")
            .map(|e| e.name.clone())
            .collect();
        let mut fe_runtime = FrontendRuntime::load(&fe_graph, &fe_archive, &levels, 60.0)?;
        fe_runtime.root("GlobeSelect", 100_000)?;
        fe_runtime.construct_page("Start", 100_000)?;
        fe_runtime.call_page_method("Start", "Open", &[], 100_000)?;

        let mut globe_models = Vec::new();
        for gname in ["Globe_new", "Globe_atmos"] {
            if let Ok(gm) = bw_rust::model::from_archive(&fe_archive, gname) {
                globe_models.push(gm);
            }
        }

        let fe_str_bytes = std::fs::read(data_dir.join("Strings/frontend2English.str"))
            .map_err(|e| e.to_string())?;
        let fe_lang = Language::parse(&fe_str_bytes)?;
        let fe_strings: Vec<String> = fe_lang.messages.into_iter().map(|m| m.text).collect();

        let credits_lines = std::fs::read(data_dir.join("Strings/creditsEnglish.str"))
            .ok()
            .and_then(|bytes| Language::parse(&bytes).ok())
            .map(|lang| {
                lang.messages
                    .into_iter()
                    .map(|m| clean_bw_text(&m.text).trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        let mut ui_textures = BTreeMap::new();
        for gm in &globe_models {
            for node in &gm.nodes {
                for surface in &node.surfaces {
                    if let Some(mats) = node.materials.get(surface.material) {
                        for mat_tex in mats.iter().filter(|s| !s.is_empty()) {
                            let key = mat_tex.to_ascii_lowercase();
                            if !ui_textures.contains_key(&key)
                                && let Ok(tex) = texture::decode_named(&fe_archive, &key)
                            {
                                ui_textures.insert(key, tex);
                            }
                        }
                    }
                }
            }
        }
        for name in [
            "bwii_logo",
            "bg_map03",
            "logo_s_01",
            "logo_ai",
            "logo_f_01",
            "logo_u_01",
            "logo_t_01",
            "logo_x_01",
            "co_sleiqo_h",
            "co_s_aqira_h",
            "co_l_ferrok_h",
            "co_ai_pierce_h",
            "co_aiwindsor_h",
            "co_wf_yherman_h",
            "co_wfherman_h_2",
            "co_wfbetty_h_2",
            "co_wfaustin_h_2",
            "co_tgorgi_h_2",
            "co_tnova_h_2",
            "co_xvlad_h_2",
            "co_xubel_h_2",
            "victory",
            "defeat",
            "gradeph_s",
            "gradeph_a",
            "gradeph_b",
            "gradeph_c",
            "s_grunt01",
            "s_hose01",
            "s_antiarm01",
        ] {
            if let Ok(tex) = texture::decode_named(&fe_archive, name) {
                ui_textures.insert(name.to_string(), tex);
            }
        }
        // Load all faction unit dossier icons & concept art plates from disc archives
        for (res_file, tex_names) in [
            (
                "fe_units_se_Level.res",
                &[
                    "logo_s_01",
                    "s_grunt01",
                    "s_hose01",
                    "s_antiarm01",
                    "s_antiair01",
                    "s_hmg01",
                    "s_mort01",
                    "s_lrec01",
                    "s_hrec01",
                    "s_ltnk01",
                    "s_htnk01",
                    "s_art01",
                    "s_aa01",
                    "s_bsta01",
                    "s_gshp01",
                    "s_fght01",
                    "s_strato",
                    "s_tcop01",
                    "s_bshp01",
                    "s_frig01",
                    "s_sub01",
                    "s_dred01",
                    "s_tran",
                ][..],
            ),
            (
                "fe_units_ai_Level.res",
                &[
                    "logo_ai",
                    "icon_anglogrunt",
                    "ai_flame_vet",
                    "ai_bazooka_vet",
                    "ai_aa_vet",
                    "ai_hmg_vet",
                    "ai_mortar_vet",
                    "ai_light_recon",
                    "ai_lt_tank",
                    "ai_hv_tank",
                    "ai_artillery",
                    "ai_anti_air",
                    "ai_bstation",
                    "ai_fighter",
                    "ai_bomber",
                    "ai_trans_chop",
                    "ai_battleship",
                    "ai_frigate",
                    "ai_sub",
                    "ai_dreadnought",
                    "ai_lan_trans",
                ][..],
            ),
            (
                "fe_units_wf_Level.res",
                &[
                    "logo_f_01",
                    "wf_grunt01",
                    "wf_hose01",
                    "wf_antiarm01",
                    "wf_antiair01",
                    "wf_hmg01",
                    "wf_mort01",
                    "wf_lrec01",
                    "wf_hrec01",
                    "wf_ltnk01",
                    "wf_htnk01",
                    "wf_art01",
                    "wf_aa01",
                    "wf_bsta01",
                    "wf_gshp01",
                    "wf_fght01",
                    "wf_bomb01",
                    "wf_tcop01",
                    "wf_bshp01",
                    "wf_frig01",
                    "wf_sub01",
                    "wf_dred01",
                    "wf_tran",
                ][..],
            ),
            (
                "fe_units_il_Level.res",
                &[
                    "logo_u_01",
                    "u_grunt01",
                    "u_hose01",
                    "u_antiarm01",
                    "u_antiair01",
                    "u_hmg01",
                    "u_mort01",
                    "u_hrec01",
                    "u_htnk01",
                    "u_art01",
                    "u_bsta01",
                    "u_gshp01",
                    "u_fght01",
                    "u_bomb01",
                    "u_tcop01",
                ][..],
            ),
            (
                "fe_units_tt_Level.res",
                &[
                    "logo_t_01",
                    "t_grunt01",
                    "t_hose01",
                    "t_antiarm01",
                    "t_antiair01",
                    "t_hmg01",
                    "t_mort01",
                    "t_lrec01",
                    "t_hrec01",
                    "t_ltnk01",
                    "t_htnk01",
                    "t_art01",
                    "t_aa01",
                    "t_bsta01",
                    "t_gshp01",
                    "t_fght01",
                    "t_bomb01",
                    "t_tcop01",
                    "t_bshp01",
                    "t_frig01",
                    "t_sub01",
                    "t_dred01",
                    "t_tran",
                ][..],
            ),
            (
                "fe_units_x_Level.res",
                &[
                    "logo_x_01",
                    "x_grunt01",
                    "x_hose01",
                    "x_antiarm01",
                    "x_antiair01",
                    "x_hmg01",
                    "x_mort01",
                    "x_lrec01",
                    "x_hrec01",
                    "x_ltnk01",
                    "x_htnk01",
                    "x_art01",
                    "x_aa01",
                    "x_bsta01",
                    "x_gshp01",
                    "x_fght01",
                    "x_bomb01",
                    "x_tcop01",
                    "x_bshp01",
                    "x_frig01",
                    "x_sub01",
                    "x_dred01",
                    "x_tran",
                ][..],
            ),
            (
                "fe_concept_Level.res",
                &[
                    "se_c_co",
                    "ai_c_co",
                    "wf_c_co",
                    "il_c_co",
                    "t_c_co",
                    "x_c_co",
                    "c_se_scene01",
                    "c_ai_scene01",
                    "c_wf_scene01",
                    "c_il_scene01",
                    "c_x_scene01",
                    "extrasolconcept",
                    "extradredconcept",
                ][..],
            ),
        ] {
            if let Ok(res_bytes) = std::fs::read(compound_dir.join(res_file)) {
                for &tname in tex_names {
                    if !ui_textures.contains_key(tname)
                        && let Ok(tex) = texture::decode_named(&res_bytes, tname)
                    {
                        ui_textures.insert(tname.to_string(), tex);
                    }
                }
            }
        }
        for &name in LEVEL_UI_TEXTURE_NAMES {
            if !ui_textures.contains_key(name)
                && let Ok(tex) = texture::decode_named(archive, name)
            {
                ui_textures.insert(name.to_string(), tex);
            }
        }
        if let Some(hb) = ui_textures.get("hud_healthbar").cloned() {
            ui_textures.insert("hud_health_bg".into(), crop_texture(&hb, 0, 16, 63, 31));
        }
        ui_textures.insert(
            "hud_health_fill".into(),
            texture::DecodedTexture {
                width: 1,
                height: 1,
                rgba: vec![255, 255, 255, 255],
            },
        );
        for (src_key, all_key, single_key) in [
            ("icon_follow", "icon_follow_all", "icon_follow_single"),
            ("icon_attack", "icon_attack_all", "icon_attack_single"),
            ("icon_sentry", "icon_sentry_all", "icon_sentry_single"),
            ("icon_move", "icon_move_all", "icon_move_single"),
        ] {
            if let Some(sheet) = ui_textures.get(src_key).cloned() {
                ui_textures.insert(all_key.into(), crop_texture(&sheet, 64, 0, 128, 64));
                ui_textures.insert(single_key.into(), crop_texture(&sheet, 0, 64, 64, 128));
            }
        }
        if let Some(stars) = ui_textures.get("object_stars").cloned() {
            ui_textures.insert(
                "obj_star_primary".into(),
                crop_texture(&stars, 32, 0, 64, 32),
            );
            ui_textures.insert(
                "obj_star_complete".into(),
                crop_texture(&stars, 0, 0, 32, 32),
            );
        }
        if let Some(idtab) = ui_textures.get("icon_idtab").cloned() {
            ui_textures.insert("idtab_box".into(), crop_texture(&idtab, 0, 0, 64, 32));
        }

        let mut music_wavs = BTreeMap::new();
        if let Some(music) = &session.program.host.world.music {
            for (name, stereo) in &music.bank.tracks {
                music_wavs.insert(name.clone(), stereo_to_wav(stereo));
            }
        }

        let mut sfx_wavs = extract_dnos_wavs(&fe_archive);
        for (k, v) in extract_dnos_wavs(archive) {
            sfx_wavs.entry(k).or_insert(v);
        }

        let skip_menu = verify
            || replay.is_some()
            || std::env::var_os("BW_SKIP_MENU").is_some()
            || std::env::var_os("BW_LEVEL").is_some()
            || (std::env::var_os("BW_SMOKE").is_some() && !menu_smoke);
        let stage = if skip_menu {
            FrontendStage::InMission
        } else {
            FrontendStage::Title
        };
        let smoke_frames = if menu_smoke { 260 } else { tail as u32 + 60 };
        let initial_player = session.program.host.world.player;
        let alive_destructibles = session
            .program
            .host
            .world
            .units
            .iter()
            .chain(session.program.host.world.props.iter())
            .filter_map(|(&id, u)| (u.active != Some(false) && u.health > 0.0).then_some(id))
            .collect();
        let selected_mission = CAMPAIGN_1_MISSIONS
            .iter()
            .position(|m| m.code == level_name)
            .unwrap_or(0);

        let save_path = if let Some(explicit) = std::env::var_os("BW_SAVE_PATH") {
            Some(std::path::PathBuf::from(explicit))
        } else if !menu_smoke
            && !verify
            && replay.is_none()
            && std::env::var_os("BW_OFFSCREEN").is_none()
        {
            Some(root.join("local/saves/bw2_saves.json"))
        } else {
            None
        };
        let mut slots = [None, None, None];
        let mut selected_slot = 0usize;
        let mut music_volume = 1.0f32;
        let mut sfx_volume = 1.0f32;
        let mut mouse_sensitivity = 1.0f32;
        let mut aim_assist_enabled = true;
        if let Some(ref sp) = save_path
            && let Ok(bytes) = std::fs::read(sp)
            && let Ok(saved) = serde_json::from_slice::<SaveFileData>(&bytes)
        {
            slots = saved.slots;
            selected_slot = saved.selected_slot.min(2);
            music_volume = saved.music_volume.clamp(0.0, 1.0);
            sfx_volume = saved.sfx_volume.clamp(0.0, 1.0);
            mouse_sensitivity = saved.mouse_sensitivity.clamp(0.25, 2.5);
            aim_assist_enabled = saved.aim_assist_enabled;
        }

        session.program.host.world.interactive_squad_formation = !verify && replay.is_none();

        Ok(Self {
            data_dir,
            save_path,
            globe_models,
            active_briefing_vo: None,
            movie_index: 0,
            active_movie: None,
            current_level: level_name.to_string(),
            pending_level: None,
            water_height,
            skydome_name,
            session,
            error: None,
            should_exit: false,
            stage,
            fe_runtime,
            fe_strings,
            credits_lines,
            campaign_levels,
            slots,
            selected_slot,
            name_buffer: "COMMANDER".into(),
            main_menu_index: 0,
            selected_mission,
            mission2_selected_notice: String::new(),
            multiplayer_index: 0,
            options_index: 0,
            music_volume,
            sfx_volume,
            mouse_sensitivity,
            aim_assist_enabled,
            extras_index: 0,
            dossier_faction: 0,
            dossier_unit: 0,
            concept_index: 0,
            credits_page: 0,
            pause_menu_open: false,
            pause_menu_index: 0,
            troops: BTreeMap::new(),
            clips,
            wheel_angles: BTreeMap::new(),
            last_positions: BTreeMap::new(),
            smoothed_positions: BTreeMap::new(),
            smoothed_rotations: BTreeMap::new(),
            smoothed_camera: None,
            ui_textures,
            ui_handles: BTreeMap::new(),
            music_wavs,
            music_handles: BTreeMap::new(),
            active_music_track: None,
            sfx_wavs,
            sfx_handles: BTreeMap::new(),
            last_voice_key: None,
            last_fired: 0,
            last_hits: 0,
            last_bomb_impacts: 0,
            last_missiles: 0,
            last_air_contacts: 0,
            last_lightning: 0,
            last_objective_banner: None,
            last_player_id: initial_player,
            was_overheated: false,
            was_reloading: false,
            was_firing_staff: false,
            was_locked: false,
            footstep_distance: 0.0,
            footstep_index: 0,
            selected_squad_slot: 0,
            music_duck: 1.0,
            alive_destructibles,
            destroyed_times: BTreeMap::new(),
            seen_rewrite_effects: 0,
            active_explosions: Vec::new(),
            vfx_fireball_materials: std::array::from_fn(|_| Handle::default()),
            vfx_smoke_materials: std::array::from_fn(|_| Handle::default()),
            locked_target: None,
            lock_toggled: false,
            accumulator: 0.0,
            recruit_pending: false,
            send_pending: false,
            dismiss_pending: bool::default(),
            mouse_delta: Vec2::ZERO,
            frames: 0,
            verify,
            menu_smoke,
            start: None,
            replay,
            replay_check_frame: tail as u32 + 40,
            smoke_frames,
            ambush_frames: 0,
            staff_frames: 0,
            expect_ambush,
            expect_staff,
            hold_replay_end,
        })
    }

    pub fn reload_level(
        &mut self,
        root: &std::path::Path,
        level_name: &str,
        graph: &ObjectGraph,
        archive: &[u8],
    ) -> Result<()> {
        let mut session = Session::load_level(root, level_name, graph, archive)?;
        session.program.host.world.interactive_squad_formation =
            !self.verify && self.replay.is_none();
        self.water_height = extract_water_height(graph);
        self.skydome_name = extract_skydome_name(graph);
        self.current_level = level_name.to_string();
        self.clips.clear();
        for unit in session.program.host.world.units.values() {
            if let Some(anim) = &unit.animation {
                for clip_name in anim.bindings.values() {
                    if !self.clips.contains_key(clip_name) {
                        self.clips.insert(
                            clip_name.clone(),
                            animation::from_archive(archive, clip_name)?,
                        );
                    }
                }
            }
        }
        for &name in LEVEL_UI_TEXTURE_NAMES {
            if !self.ui_handles.contains_key(name)
                && let Ok(tex) = texture::decode_named(archive, name)
            {
                self.ui_textures.insert(name.to_string(), tex);
            }
        }
        self.music_handles.clear();
        self.music_wavs.clear();
        self.active_music_track = None;
        if let Some(music) = &session.program.host.world.music {
            for (name, stereo) in &music.bank.tracks {
                self.music_wavs.insert(name.clone(), stereo_to_wav(stereo));
            }
        }
        for (k, v) in extract_dnos_wavs(archive) {
            if !self.sfx_handles.contains_key(&k) {
                self.sfx_wavs.insert(k, v);
            }
        }
        self.troops.clear();
        self.wheel_angles.clear();
        self.last_positions.clear();
        self.smoothed_positions.clear();
        self.smoothed_rotations.clear();
        self.smoothed_camera = None;
        self.session = session;
        self.reset_mission_state();
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn spawn_placement(
        &mut self,
        commands: &mut Commands,
        placement: &Placement,
        model: &RenderModel,
        source: &Model,
        model_index: usize,
        root: Mat4,
        meshes: &mut Assets<Mesh>,
    ) {
        let is_prop = self
            .session
            .program
            .host
            .world
            .props
            .contains_key(&placement.id);
        let is_unit = self
            .session
            .program
            .host
            .world
            .units
            .contains_key(&placement.id)
            || is_prop;
        let is_animated_troop = self
            .session
            .program
            .host
            .world
            .units
            .get(&placement.id)
            .is_some_and(|u| u.animation.is_some());
        if is_animated_troop {
            let mut actor_parts = Vec::new();
            for p in &model.parts {
                if is_hidden_subnode(source, p.node_index)
                    || is_fragment_subnode(source, p.node_index)
                {
                    continue;
                }
                if source.nodes[p.node_index].has_skin_data {
                    let mut cloned = p.clone();
                    cloned.mesh = meshes.add(meshes.get(&p.mesh).unwrap().clone());
                    commands.spawn((
                        SceneEntity,
                        Mesh3d(cloned.mesh.clone()),
                        MeshMaterial3d(cloned.material.clone()),
                        Transform::from_matrix(root * cloned.transform),
                        Part {
                            id: placement.id,
                            root,
                            local: cloned.transform,
                            grounded: placement.stick_to_floor,
                            bone: None,
                            wheel_radius: None,
                            rotor_axis: None,
                            turret_role: 0,
                            turret_yaw_pivot: [0.0, 0.0],
                            turret_pivot: Vec3::ZERO,
                            tread_mesh: None,
                            tread_uvs: None,
                            is_fragment: false,
                            frag_vel: Vec3::ZERO,
                            is_water_vehicle: false,
                            is_mobile: true,
                            is_unit: true,
                            keep_when_dead: false,
                        },
                    ));
                    actor_parts.push(cloned);
                } else {
                    commands.spawn((
                        SceneEntity,
                        Mesh3d(p.mesh.clone()),
                        MeshMaterial3d(p.material.clone()),
                        Transform::from_matrix(root * p.transform),
                        Part {
                            id: placement.id,
                            root,
                            local: Mat4::IDENTITY,
                            grounded: placement.stick_to_floor,
                            bone: Some(p.node_index),
                            wheel_radius: None,
                            rotor_axis: None,
                            turret_role: 0,
                            turret_yaw_pivot: [0.0, 0.0],
                            turret_pivot: Vec3::ZERO,
                            tread_mesh: None,
                            tread_uvs: None,
                            is_fragment: false,
                            frag_vel: Vec3::ZERO,
                            is_water_vehicle: false,
                            is_mobile: true,
                            is_unit: true,
                            keep_when_dead: false,
                        },
                    ));
                }
            }
            self.troops.insert(
                placement.id,
                TroopActor {
                    model_index,
                    parts: actor_parts,
                    last_pose_key: None,
                    last_pose: Vec::new(),
                },
            );
        } else {
            let is_ground_vehicle = placement.kind == "cGroundVehicle";
            let is_water_vehicle = placement.kind == "cWaterVehicle";
            let is_air_vehicle = placement.kind == "cAirVehicle";
            let is_mobile = is_ground_vehicle
                || is_water_vehicle
                || is_air_vehicle
                || placement.kind == "cTroop";
            let mut wheel_radii = BTreeMap::new();
            if is_ground_vehicle && let Ok(slots) = bw_rust::vehicle_wheels::slots(source) {
                for slot in slots {
                    if let Some(part) = slot.part {
                        wheel_radii.insert(part.node_index, part.radius);
                    }
                }
            }
            let has_child_fragments = !is_mobile
                && (is_prop
                    || self
                        .session
                        .program
                        .host
                        .world
                        .buildings
                        .contains(&placement.id))
                && source.nodes.len() > 1;
            for p in &model.parts {
                if is_hidden_subnode(source, p.node_index) {
                    continue;
                }
                let is_frag = is_fragment_subnode(source, p.node_index);
                if is_frag && !is_unit && !is_mobile {
                    continue;
                }
                let node_upper = source.nodes[p.node_index].name.to_ascii_uppercase();
                let is_foundation = node_upper == "BASE"
                    || node_upper == "H_BASE"
                    || node_upper.ends_with("_BASE")
                    || node_upper.starts_with("PILLAR");
                let rotor_axis = if !is_frag && (is_air_vehicle || is_water_vehicle) {
                    detect_rotor_axis(&node_upper)
                } else {
                    None
                };
                let (turret_role, turret_yaw_pivot, turret_pivot) = if !is_frag
                    && (is_ground_vehicle || is_water_vehicle || is_unit)
                    && rotor_axis.is_none()
                    && !wheel_radii.contains_key(&p.node_index)
                {
                    detect_turret_role_and_pivot(source, model, p.node_index)
                } else {
                    (0, [0.0, 0.0], Vec3::ZERO)
                };
                let surface = &source.nodes[p.node_index].surfaces[p.surface_index];
                let tex_upper = source.nodes[p.node_index]
                    .materials
                    .get(surface.material)
                    .and_then(|m| m.iter().find(|s| !s.is_empty()))
                    .map(|s| s.to_ascii_uppercase())
                    .unwrap_or_default();
                let is_tread = is_ground_vehicle
                    && !is_frag
                    && (node_upper.contains("TREAD")
                        || node_upper.contains("TRACK")
                        || tex_upper.contains("TREAD")
                        || tex_upper.contains("TRACK"));
                let (mesh_handle, tread_mesh, tread_uvs) =
                    if is_tread && let Some(base_mesh) = meshes.get(&p.mesh).cloned() {
                        let cloned = meshes.add(base_mesh);
                        let uvs = surface.vertices.iter().map(|v| v.uv).collect::<Vec<_>>();
                        (cloned.clone(), Some(cloned), Some(uvs))
                    } else {
                        (p.mesh.clone(), None, None)
                    };
                let frag_vel = if is_frag {
                    let offset = p.transform.w_axis.truncate();
                    let angle = (p.node_index as f32) * 2.39996 + (placement.id.0 as f32) * 0.71;
                    let radial = Vec2::new(offset.x, offset.z)
                        .try_normalize()
                        .unwrap_or_else(|| Vec2::new(angle.cos(), angle.sin()));
                    let speed = 7.5 + ((p.node_index % 5) as f32) * 2.4;
                    let up = 9.5 + ((p.node_index % 4) as f32) * 2.8;
                    Vec3::new(radial.x * speed, up, radial.y * speed)
                } else {
                    Vec3::ZERO
                };
                if is_unit || is_mobile {
                    commands.spawn((
                        SceneEntity,
                        Mesh3d(mesh_handle),
                        MeshMaterial3d(p.material.clone()),
                        Transform::from_matrix(root * p.transform),
                        if is_frag {
                            Visibility::Hidden
                        } else {
                            Visibility::Visible
                        },
                        Part {
                            id: placement.id,
                            root,
                            local: p.transform,
                            grounded: placement.stick_to_floor,
                            bone: None,
                            wheel_radius: if is_frag {
                                None
                            } else {
                                wheel_radii.get(&p.node_index).copied()
                            },
                            rotor_axis,
                            turret_role,
                            turret_yaw_pivot,
                            turret_pivot,
                            tread_mesh,
                            tread_uvs,
                            is_fragment: is_frag,
                            frag_vel,
                            is_water_vehicle,
                            is_mobile,
                            is_unit,
                            keep_when_dead: !is_frag && has_child_fragments && is_foundation,
                        },
                    ));
                } else {
                    commands.spawn((
                        SceneEntity,
                        Mesh3d(mesh_handle),
                        MeshMaterial3d(p.material.clone()),
                        Transform::from_matrix(root * p.transform),
                    ));
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn spawn_attachment(
        &self,
        commands: &mut Commands,
        id: ObjectId,
        kind: &str,
        model: &RenderModel,
        source: &Model,
        root: Mat4,
        matrix: Mat4,
        bone_index: Option<usize>,
        grounded: bool,
    ) {
        let animated = self.troops.contains_key(&id);
        let is_unit = animated || self.session.program.host.world.units.contains_key(&id);
        let bone = if animated { bone_index } else { None };
        let is_ground_vehicle = kind == "cGroundVehicle";
        let is_water_vehicle = kind == "cWaterVehicle";
        let is_air_vehicle = kind == "cAirVehicle";
        let is_mobile =
            animated || is_ground_vehicle || is_water_vehicle || is_air_vehicle || kind == "cTroop";
        for p in &model.parts {
            if is_hidden_subnode(source, p.node_index) || is_fragment_subnode(source, p.node_index)
            {
                continue;
            }
            let node_upper = source.nodes[p.node_index].name.to_ascii_uppercase();
            let rotor_axis = if is_air_vehicle || is_water_vehicle {
                detect_rotor_axis(&node_upper)
            } else {
                None
            };
            let local = if bone.is_some() {
                p.transform
            } else {
                matrix * p.transform
            };
            let (turret_role, turret_yaw_pivot, turret_pivot) =
                if !animated && (is_ground_vehicle || is_water_vehicle) && rotor_axis.is_none() {
                    let (role, yp, bp) = detect_turret_role_and_pivot(source, model, p.node_index);
                    if role > 0 {
                        let m_bp = matrix.transform_point3(bp);
                        let m_yp = matrix.transform_point3(Vec3::new(yp[0], bp.y, yp[1]));
                        (role, [m_yp.x, m_yp.z], m_bp)
                    } else {
                        (0, [0.0, 0.0], Vec3::ZERO)
                    }
                } else {
                    (0, [0.0, 0.0], Vec3::ZERO)
                };
            commands.spawn((
                SceneEntity,
                Mesh3d(p.mesh.clone()),
                MeshMaterial3d(p.material.clone()),
                Transform::from_matrix(root * matrix * p.transform),
                Part {
                    id,
                    root,
                    local,
                    grounded,
                    bone,
                    wheel_radius: None,
                    rotor_axis,
                    turret_role,
                    turret_yaw_pivot,
                    turret_pivot,
                    tread_mesh: None,
                    tread_uvs: None,
                    is_fragment: false,
                    frag_vel: Vec3::ZERO,
                    is_water_vehicle,
                    is_mobile,
                    is_unit,
                    keep_when_dead: false,
                },
            ));
        }
    }

    pub fn save_to_disk(&self) {
        let Some(ref path) = self.save_path else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let payload = SaveFileData {
            slots: self.slots.clone(),
            selected_slot: self.selected_slot,
            music_volume: self.music_volume,
            sfx_volume: self.sfx_volume,
            mouse_sensitivity: self.mouse_sensitivity,
            aim_assist_enabled: self.aim_assist_enabled,
        };
        if let Ok(json) = serde_json::to_vec_pretty(&payload) {
            let _ = std::fs::write(path, json);
        }
    }

    fn current_mission_index(&self) -> usize {
        CAMPAIGN_1_MISSIONS
            .iter()
            .position(|m| m.code == self.current_level)
            .unwrap_or(self.selected_mission.min(CAMPAIGN_1_MISSIONS.len() - 1))
    }

    fn complete_mission_and_open_victory(
        &mut self,
        speed: f64,
        power: f64,
        technique: f64,
        grade: &str,
    ) {
        let mission_idx = self.current_mission_index();
        let slot = &mut self.slots[self.selected_slot];
        let profile = slot.get_or_insert_with(|| SaveProfile {
            name: self.name_buffer.clone(),
            levels_done: mission_idx + 1,
            sp_0_1_scores: None,
            sp_0_1_grade: None,
            mission_scores: [None; 20],
            mission_grades: std::array::from_fn(|_| None),
        });
        profile.levels_done = profile.levels_done.max(mission_idx + 1);
        profile.mission_scores[mission_idx] = Some((speed, power, technique));
        profile.mission_grades[mission_idx] = Some(grade.to_string());
        if mission_idx == 0 {
            profile.sp_0_1_scores = Some((speed, power, technique));
            profile.sp_0_1_grade = Some(grade.to_string());
        }
        self.save_to_disk();
        self.pause_menu_open = false;
        self.stage = FrontendStage::Victory;
    }

    fn reset_mission_state(&mut self) {
        self.session.restart();
        self.session.program.host.world.interactive_squad_formation =
            !self.verify && self.replay.is_none();
        for troop in self.troops.values_mut() {
            troop.last_pose_key = None;
            troop.last_pose.clear();
        }
        self.smoothed_positions.clear();
        self.smoothed_rotations.clear();
        self.smoothed_camera = None;
        self.error = None;
        self.accumulator = 0.0;
        self.recruit_pending = false;
        self.send_pending = false;
        self.dismiss_pending = false;
        self.mouse_delta = Vec2::ZERO;
        self.pause_menu_open = false;
        self.last_voice_key = None;
        self.last_fired = 0;
        self.last_hits = 0;
        self.last_bomb_impacts = 0;
        self.last_missiles = 0;
        self.last_air_contacts = 0;
        self.last_lightning = 0;
        self.last_objective_banner = None;
        self.last_player_id = self.session.program.host.world.player;
        self.was_overheated = false;
        self.was_reloading = false;
        self.was_firing_staff = false;
        self.was_locked = false;
        self.footstep_distance = 0.0;
        self.footstep_index = 0;
        self.music_duck = 1.0;
        self.seen_rewrite_effects = 0;
        self.active_explosions.clear();
        self.destroyed_times.clear();
        self.locked_target = None;
        self.lock_toggled = false;
        self.alive_destructibles = self
            .session
            .program
            .host
            .world
            .units
            .iter()
            .chain(self.session.program.host.world.props.iter())
            .filter_map(|(&id, u)| (u.active != Some(false) && u.health > 0.0).then_some(id))
            .collect();
    }
}

fn detect_rotor_axis(node_upper: &str) -> Option<u8> {
    if node_upper.contains("TAIL_ROTOR") {
        Some(0) // X axis spin
    } else if node_upper.contains("ROTOR") || node_upper.contains("FAN") {
        Some(1) // Y axis spin
    } else if node_upper.contains("PROPELLER") || node_upper.starts_with("PROP") {
        Some(2) // Z axis spin
    } else {
        None
    }
}

fn is_fragment_subnode(model: &Model, node_index: usize) -> bool {
    let mut cur = Some(node_index);
    let mut depth = 0;
    while let Some(idx) = cur {
        if depth > 32 || idx >= model.nodes.len() {
            break;
        }
        let name = model.nodes[idx].name.to_ascii_uppercase();
        if name.starts_with("FRAG") || name.contains("_FRAG_") || name.ends_with("_FRAG") {
            return true;
        }
        cur = model.nodes[idx].parent;
        depth += 1;
    }
    false
}

fn detect_turret_role_and_pivot(
    source: &Model,
    render_model: &RenderModel,
    node_index: usize,
) -> (u8, [f32; 2], Vec3) {
    let mut barrel_pivot: Option<Vec3> = None;
    let mut turret_pivot: Option<Vec3> = None;
    let mut cur = Some(node_index);
    let mut depth = 0;
    while let Some(idx) = cur {
        if depth > 32 || idx >= source.nodes.len() {
            break;
        }
        let node = &source.nodes[idx];
        let name = node.name.to_ascii_uppercase();
        let pivot = render_model
            .bones
            .get(&node.name)
            .map(|m| m.w_axis.truncate())
            .unwrap_or(Vec3::ZERO);
        if barrel_pivot.is_none()
            && (name.contains("BARREL") || name.contains("GUN") || name.contains("CANNON"))
        {
            barrel_pivot = Some(pivot);
        }
        if turret_pivot.is_none() && name.contains("TURRET") {
            turret_pivot = Some(pivot);
        }
        cur = node.parent;
        depth += 1;
    }
    if let Some(bp) = barrel_pivot {
        let yp = turret_pivot.unwrap_or(bp);
        (2, [yp.x, yp.z], bp)
    } else if let Some(tp) = turret_pivot {
        (1, [tp.x, tp.z], tp)
    } else {
        (0, [0.0, 0.0], Vec3::ZERO)
    }
}

fn is_hidden_subnode(model: &Model, node_index: usize) -> bool {
    let model_upper = model.name.to_ascii_uppercase();
    if model_upper == "SFXDUMMYROOT" || model_upper.starts_with("SFXOBJMARK") {
        return true;
    }
    let node = &model.nodes[node_index];
    let name = node.name.to_ascii_uppercase();
    name.contains("MUZZLEFLASH") || name.contains("_FAST")
}

#[derive(Component)]
pub struct SkydomePart(pub Mat4);

#[derive(Component)]
pub struct WaterPlane {
    pub base_y: f32,
    pub layer: u8,
}

pub fn spawn_skydome(commands: &mut Commands, model: &RenderModel) {
    let base = Mat4::from_scale_rotation_translation(
        Vec3::splat(3500.0),
        Quat::IDENTITY,
        Vec3::new(0.0, -40.0, 0.0),
    );
    for p in &model.parts {
        commands.spawn((
            SceneEntity,
            SkydomePart(p.transform),
            Mesh3d(p.mesh.clone()),
            MeshMaterial3d(p.material.clone()),
            Transform::from_matrix(base * p.transform),
        ));
    }
}

pub fn spawn_water_plane(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    water_height: f32,
) {
    let mesh = meshes.add(Plane3d::default().mesh().size(12000.0, 12000.0));
    let deep_material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.04, 0.21, 0.39, 0.85),
        perceptual_roughness: 0.14,
        metallic: 0.15,
        reflectance: 0.72,
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        ..default()
    });
    commands.spawn((
        SceneEntity,
        WaterPlane {
            base_y: water_height,
            layer: 0,
        },
        Mesh3d(mesh.clone()),
        MeshMaterial3d(deep_material),
        Transform::from_xyz(0.0, water_height, 0.0),
    ));
    let shimmer_material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.18, 0.56, 0.78, 0.22),
        emissive: LinearRgba::new(0.05, 0.18, 0.28, 1.0),
        perceptual_roughness: 0.08,
        metallic: 0.05,
        reflectance: 0.88,
        alpha_mode: AlphaMode::Add,
        cull_mode: None,
        ..default()
    });
    commands.spawn((
        SceneEntity,
        WaterPlane {
            base_y: water_height + 0.06,
            layer: 1,
        },
        Mesh3d(mesh),
        MeshMaterial3d(shimmer_material),
        Transform::from_xyz(0.0, water_height + 0.06, 0.0),
    ));
}

pub fn spawn_gold_stars(
    commands: &mut Commands,
    markers: &[bw_rust::mission_guidance::Marker],
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let mut vertices = Vec::new();
    for i in 0..10 {
        let point = |j: usize| {
            let a = std::f32::consts::FRAC_PI_2 + j as f32 * std::f32::consts::PI / 5.;
            let r = if j.is_multiple_of(2) { 1.0 } else { 0.42 };
            [a.cos() * r, a.sin() * r, 0.]
        };
        vertices.extend([[0.; 3], point(i), point(i + 1)]);
    }
    let star_mesh = meshes.add(
        Mesh::new(
            bevy::mesh::PrimitiveTopology::TriangleList,
            bevy::asset::RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vertices)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0., 0., 1.]; 30]),
    );
    let star_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1., 0.82, 0.08),
        unlit: true,
        cull_mode: None,
        ..default()
    });
    for marker in markers {
        commands.spawn((
            GoldStar(marker.clone()),
            Mesh3d(star_mesh.clone()),
            MeshMaterial3d(star_mat.clone()),
            Transform::default(),
            Visibility::Hidden,
        ));
    }
}

fn play_sfx_vol(commands: &mut Commands, view: &View, name: &str, volume: f32) {
    let scaled = volume * view.sfx_volume;
    if std::env::var_os("BW_OFFSCREEN").is_some() || scaled <= 0.01 {
        return;
    }
    if let Some(handle) = view.sfx_handles.get(&name.to_ascii_lowercase()) {
        commands.spawn((
            AudioPlayer::new(handle.clone()),
            PlaybackSettings::DESPAWN.with_volume(bevy::audio::Volume::Linear(scaled)),
        ));
    }
}

fn play_sfx(commands: &mut Commands, view: &View, name: &str) {
    play_sfx_vol(commands, view, name, 0.72);
}

fn unit_profile(view: &View, id: ObjectId) -> (&'static str, &'static str, &'static str, usize) {
    if id == ObjectId(550035312) {
        return ("s_grunt01", "SOLAR STAFF BEARER", "WSolarStaffStrong", 1);
    }
    if id == ObjectId(550030291) {
        return ("s_grunt01", "SOLAR KOMMANDANT", "WSolarStaffStrong", 1);
    }
    match view.session.combat.base_id(id) {
        Some(ObjectId(550032480)) | Some(ObjectId(550040922)) => {
            return ("s_grunt01", "SOLAR STAFF BEARER", "WSolarStaffStrong", 1);
        }
        Some(ObjectId(550040025)) | Some(ObjectId(550035611)) | Some(ObjectId(550040956)) => {
            return ("u_antiarm01", "UNEXPLODED SHELL", "DE_Barrel_01", 0);
        }
        Some(ObjectId(250011725)) | Some(ObjectId(250011744)) => {
            return ("u_bsta01", "ROCK BLOCKADE", "DE_Debris_Rck_1", 0);
        }
        Some(ObjectId(550034268)) | Some(ObjectId(250006158)) => {
            return ("u_bsta01", "NEROCITE CRYSTAL", "DE_Debris_Rck_2", 0);
        }
        Some(ObjectId(170011233)) => return ("u_antiarm01", "AMMO DUMP", "DE_Barrel_02", 0),
        Some(ObjectId(550031706)) => return ("u_antiarm01", "FUEL BARREL", "DE_Barrel_03", 0),
        Some(ObjectId(550034312)) => return ("u_bsta01", "IRON PILLBOX", "DE_Structure_1", 0),
        Some(ObjectId(250008584)) => return ("u_bsta01", "GATE WINCH", "DE_Winch_Explode", 0),
        Some(ObjectId(550024037)) | Some(ObjectId(250012035)) => {
            return ("u_bsta01", "IRON GATE", "DE_Portcullis", 0);
        }
        Some(ObjectId(550035370)) => return ("u_bsta01", "DRAINAGE WALL", "DE_Wall_01", 0),
        Some(ObjectId(550039972)) => return ("u_bsta01", "IRON TOWER", "DE_Structure_2", 0),
        _ => {}
    }

    if let Some(base) = view.session.combat.base_object(id) {
        let raw_army = field(base, "mArmy").unwrap_or("eSolarEmpire");
        let army = if raw_army == "eUnderWorld" {
            "eUnderworld"
        } else {
            raw_army
        };
        let raw_sub = field(base, "mUnitSubType").unwrap_or("");
        let sub = match raw_sub {
            "UNIT_MAN_GRUNT" => "GRUNT",
            "UNIT_MAN_FLAMER" => "FLAME",
            "UNIT_MAN_BAZOOKA" => "BAZOOKA",
            "UNIT_MAN_ANTI_AIR" => "MISSILE",
            "UNIT_MAN_CHAINGUN" => "ASSAULT",
            "UNIT_VEH_RECON" => "LIGHT_RECON",
            "UNIT_VEH_LIGHT_TANK" => "LIGHT_TANK",
            "UNIT_VEH_HEAVY_TANK" => "HEAVY_TANK",
            "UNIT_VEH_ARTILLARY" => "ARTILLERY",
            "UNIT_VEH_ANTI_AIR" => "ANTI_AIR",
            "UNIT_VEH_BATTLESTATION" => "BATTLESTATION",
            "UNIT_VEH_HEAVY_TRANSPORT" => "TRANSPORT",
            "UNIT_AIR_GUNSHIP" => "GUNSHIP",
            "UNIT_AIR_FIGHTER" => "FIGHTER",
            "UNIT_AIR_BOMBER" => "BOMBER",
            "UNIT_AIR_TRANSPORT" => "TRANSPORT_COPTER",
            "UNIT_SEA_BATTLESHIP" => "BATTLESHIP",
            "UNIT_SEA_FRIGATE" => "FRIGATE",
            "UNIT_SEA_GUNBOAT" => "SUBMARINE",
            other => other,
        };
        match (army, sub) {
            // Solar Empire
            ("eSolarEmpire", "GRUNT") => {
                return ("s_grunt01", "SOLAR EMPIRE VET", "WS_Rifle_Fire", 1);
            }
            ("eSolarEmpire", "FLAME") => return ("s_hose01", "SOLAR FLAME VET", "WS_Hose_Loop", 2),
            ("eSolarEmpire", "BAZOOKA") => {
                return ("s_antiarm01", "SOLAR BAZOOKA VET", "WS_Baz_Fire", 3);
            }
            ("eSolarEmpire", "MISSILE") => return ("s_antiair01", "SOLAR AA VET", "WS_AA_Fire", 4),
            ("eSolarEmpire", "ASSAULT") => return ("s_hmg01", "SOLAR HMG VET", "WS_Chain_Fire", 4),
            ("eSolarEmpire", "MORTAR") => {
                return ("s_mort01", "SOLAR MORTAR VET", "WS_Baz_Fire", 4);
            }
            ("eSolarEmpire", "LIGHT_RECON") => {
                return ("s_lrec01", "SOLAR LIGHT RECON", "WS_Chain_Fire", 4);
            }
            ("eSolarEmpire", "HEAVY_RECON") => {
                return ("s_hrec01", "SOLAR HEAVY RECON", "WS_Chain_Fire", 4);
            }
            ("eSolarEmpire", "LIGHT_TANK") => {
                return ("s_ltnk01", "SOLAR LIGHT TANK", "VS_HTank_Fire", 4);
            }
            ("eSolarEmpire", "HEAVY_TANK") => {
                return ("s_htnk01", "SOLAR HEAVY TANK", "VS_HTank_Fire", 4);
            }
            ("eSolarEmpire", "ARTILLERY") => {
                return ("s_art01", "SOLAR ARTILLERY", "VS_HTank_Fire", 4);
            }
            ("eSolarEmpire", "ANTI_AIR") => return ("s_aa01", "SOLAR AA VEHICLE", "WS_AA_Fire", 4),
            ("eSolarEmpire", "BATTLESTATION") => {
                return ("s_bsta01", "SOLAR BATTLESTATION", "VS_HTank_Fire", 4);
            }
            ("eSolarEmpire", "GUNSHIP") => return ("s_gshp01", "SOLAR GUNSHIP", "WS_Baz_Fire", 4),
            ("eSolarEmpire", "FIGHTER") => return ("s_fght01", "SOLAR FIGHTER", "WS_AA_Fire", 4),
            ("eSolarEmpire", "BOMBER") | ("eSolarEmpire", "STRATO_DESTROYER") => {
                return ("s_strato", "STRATODESTROYER", "PE_Bomb_1", 4);
            }
            ("eSolarEmpire", "TRANSPORT_COPTER") => {
                return ("s_tcop01", "SOLAR TRANSPORT", "WS_Chain_Fire", 4);
            }
            ("eSolarEmpire", "BATTLESHIP") => {
                return ("s_bshp01", "SOLAR BATTLESHIP", "VS_HTank_Fire", 4);
            }
            ("eSolarEmpire", "FRIGATE") => return ("s_frig01", "SOLAR FRIGATE", "WS_AA_Fire", 4),
            ("eSolarEmpire", "SUBMARINE") => {
                return ("s_sub01", "SOLAR SUBMARINE", "WS_Baz_Fire", 4);
            }
            ("eSolarEmpire", "DREADNOUGHT") => {
                return ("s_dred01", "SOLAR DREADNOUGHT", "VS_HTank_Fire", 4);
            }
            ("eSolarEmpire", "TRANSPORT") => {
                return ("s_tran", "SOLAR NAVAL TRANSPORT", "WS_Chain_Fire", 4);
            }

            // Anglo Isles
            ("eAngloIsles", "GRUNT") => {
                return ("icon_anglogrunt", "ANGLO ISLES GRUNT", "WS_Rifle_Fire", 1);
            }
            ("eAngloIsles", "FLAME") => {
                return ("ai_flame_vet", "ANGLO FLAME VET", "WS_Hose_Loop", 2);
            }
            ("eAngloIsles", "BAZOOKA") => {
                return ("ai_bazooka_vet", "ANGLO BAZOOKA VET", "WS_Baz_Fire", 3);
            }
            ("eAngloIsles", "MISSILE") => return ("ai_aa_vet", "ANGLO AA VET", "WS_AA_Fire", 4),
            ("eAngloIsles", "ASSAULT") => {
                return ("ai_hmg_vet", "ANGLO HMG VET", "WS_Chain_Fire", 4);
            }
            ("eAngloIsles", "MORTAR") => {
                return ("ai_mortar_vet", "ANGLO MORTAR VET", "WS_Baz_Fire", 4);
            }
            ("eAngloIsles", "LIGHT_RECON") | ("eAngloIsles", "HEAVY_RECON") => {
                return ("ai_light_recon", "ANGLO LIGHT RECON", "WS_Chain_Fire", 4);
            }
            ("eAngloIsles", "LIGHT_TANK") => {
                return ("ai_lt_tank", "ANGLO LIGHT TANK", "VS_HTank_Fire", 4);
            }
            ("eAngloIsles", "HEAVY_TANK") => {
                return ("ai_hv_tank", "ANGLO HEAVY TANK", "VS_HTank_Fire", 4);
            }
            ("eAngloIsles", "ARTILLERY") => {
                return ("ai_artillery", "ANGLO ARTILLERY", "VS_HTank_Fire", 4);
            }
            ("eAngloIsles", "ANTI_AIR") => {
                return ("ai_anti_air", "ANGLO AA VEHICLE", "WS_AA_Fire", 4);
            }
            ("eAngloIsles", "BATTLESTATION") => {
                return ("ai_bstation", "ANGLO BATTLESTATION", "VS_HTank_Fire", 4);
            }
            ("eAngloIsles", "FIGHTER") | ("eAngloIsles", "GUNSHIP") => {
                return ("ai_fighter", "ANGLO FIGHTER", "WS_AA_Fire", 4);
            }
            ("eAngloIsles", "BOMBER") | ("eAngloIsles", "STRATO_DESTROYER") => {
                return ("ai_bomber", "ANGLO BOMBER", "PE_Bomb_1", 4);
            }
            ("eAngloIsles", "TRANSPORT_COPTER") => {
                return (
                    "ai_trans_chop",
                    "ANGLO TRANSPORT CHOPPER",
                    "WS_Chain_Fire",
                    4,
                );
            }
            ("eAngloIsles", "BATTLESHIP") => {
                return ("ai_battleship", "ANGLO BATTLESHIP", "VS_HTank_Fire", 4);
            }
            ("eAngloIsles", "FRIGATE") => return ("ai_frigate", "ANGLO FRIGATE", "WS_AA_Fire", 4),
            ("eAngloIsles", "SUBMARINE") => return ("ai_sub", "ANGLO SUBMARINE", "WS_Baz_Fire", 4),
            ("eAngloIsles", "DREADNOUGHT") => {
                return ("ai_dreadnought", "ANGLO DREADNOUGHT", "VS_HTank_Fire", 4);
            }
            ("eAngloIsles", "TRANSPORT") => {
                return ("ai_lan_trans", "ANGLO LANDING CRAFT", "WS_Chain_Fire", 4);
            }

            // Western Frontier
            ("eWesternFrontier", "GRUNT") => {
                return ("wf_grunt01", "FRONTIER GRUNT", "WS_Rifle_Fire", 1);
            }
            ("eWesternFrontier", "FLAME") => {
                return ("wf_hose01", "FRONTIER FLAME VET", "WS_Hose_Loop", 2);
            }
            ("eWesternFrontier", "BAZOOKA") => {
                return ("wf_antiarm01", "FRONTIER BAZOOKA VET", "WS_Baz_Fire", 3);
            }
            ("eWesternFrontier", "MISSILE") => {
                return ("wf_antiair01", "FRONTIER AA VET", "WS_AA_Fire", 4);
            }
            ("eWesternFrontier", "ASSAULT") => {
                return ("wf_hmg01", "FRONTIER HMG VET", "WS_Chain_Fire", 4);
            }
            ("eWesternFrontier", "MORTAR") => {
                return ("wf_mort01", "FRONTIER MORTAR VET", "WS_Baz_Fire", 4);
            }
            ("eWesternFrontier", "LIGHT_RECON") => {
                return ("wf_lrec01", "FRONTIER LIGHT RECON", "WS_Chain_Fire", 4);
            }
            ("eWesternFrontier", "HEAVY_RECON") => {
                return ("wf_hrec01", "FRONTIER HEAVY RECON", "WS_Chain_Fire", 4);
            }
            ("eWesternFrontier", "LIGHT_TANK") => {
                return ("wf_ltnk01", "FRONTIER LIGHT TANK", "VS_HTank_Fire", 4);
            }
            ("eWesternFrontier", "HEAVY_TANK") => {
                return ("wf_htnk01", "FRONTIER HEAVY TANK", "VS_HTank_Fire", 4);
            }
            ("eWesternFrontier", "ARTILLERY") => {
                return ("wf_art01", "FRONTIER ARTILLERY", "VS_HTank_Fire", 4);
            }
            ("eWesternFrontier", "ANTI_AIR") => {
                return ("wf_aa01", "FRONTIER AA VEHICLE", "WS_AA_Fire", 4);
            }
            ("eWesternFrontier", "BATTLESTATION") => {
                return ("wf_bsta01", "FRONTIER BATTLESTATION", "VS_HTank_Fire", 4);
            }
            ("eWesternFrontier", "GUNSHIP") => {
                return ("wf_gshp01", "FRONTIER GUNSHIP", "WS_Baz_Fire", 4);
            }
            ("eWesternFrontier", "FIGHTER") => {
                return ("wf_fght01", "FRONTIER FIGHTER", "WS_AA_Fire", 4);
            }
            ("eWesternFrontier", "BOMBER") => {
                return ("wf_bomb01", "FRONTIER BOMBER", "PE_Bomb_1", 4);
            }
            ("eWesternFrontier", "TRANSPORT_COPTER") => {
                return ("wf_tcop01", "FRONTIER T-COPTER", "WS_Chain_Fire", 4);
            }
            ("eWesternFrontier", "BATTLESHIP") => {
                return ("wf_bshp01", "FRONTIER BATTLESHIP", "VS_HTank_Fire", 4);
            }
            ("eWesternFrontier", "FRIGATE") => {
                return ("wf_frig01", "FRONTIER FRIGATE", "WS_AA_Fire", 4);
            }
            ("eWesternFrontier", "SUBMARINE") => {
                return ("wf_sub01", "FRONTIER SUBMARINE", "WS_Baz_Fire", 4);
            }
            ("eWesternFrontier", "DREADNOUGHT") => {
                return ("wf_dred01", "FRONTIER DREADNOUGHT", "VS_HTank_Fire", 4);
            }
            ("eWesternFrontier", "TRANSPORT") => {
                return ("wf_tran", "FRONTIER TRANSPORT", "WS_Chain_Fire", 4);
            }

            // Iron Legion (eUnderworld)
            ("eUnderworld", "GRUNT") => {
                return ("u_grunt01", "IRON LEGION GRUNT", "WU_Rifle_Fire", 0);
            }
            ("eUnderworld", "FLAME") => {
                return ("u_hose01", "IRON LEGION FLAMER", "WU_Flamer_Loop", 0);
            }
            ("eUnderworld", "BAZOOKA") => {
                return ("u_antiarm01", "IRON LEGION BAZOOKA", "WU_Baz_Fire", 0);
            }
            ("eUnderworld", "MISSILE") => {
                return ("u_antiair01", "IRON LEGION AA", "WU_Baz_Fire", 0);
            }
            ("eUnderworld", "ASSAULT") => {
                return ("u_hmg01", "IRON LEGION HMG", "WU_Rifle_Fire", 0);
            }
            ("eUnderworld", "MORTAR") => {
                return ("u_mort01", "IRON LEGION MORTAR", "WU_Baz_Fire", 0);
            }
            ("eUnderworld", "LIGHT_TANK") | ("eUnderworld", "HEAVY_TANK") => {
                return ("u_htnk01", "IRON HEAVY TANK", "WU_Baz_Fire", 0);
            }
            ("eUnderworld", "BATTLESTATION") => {
                return ("u_bsta01", "IRON BATTLESTATION", "VU_Batt_Fire", 0);
            }
            ("eUnderworld", "GUNSHIP") => return ("u_gshp01", "IRON GUNSHIP", "WU_Baz_Fire", 0),
            ("eUnderworld", "FIGHTER") => return ("u_fght01", "IRON FIGHTER", "WU_Baz_Fire", 0),
            ("eUnderworld", "BOMBER") => return ("u_bomb01", "IRON BOMBER", "PE_Bomb_1", 0),
            _ => {}
        }
    }

    if view.session.program.host.world.buildings.contains(&id) {
        ("ai_bstation", "FORTIFIED STRUCTURE", "DE_Structure_1", 0)
    } else if view.session.program.host.world.props.contains_key(&id) {
        ("u_antiarm01", "EXPLOSIVE TARGET", "DE_Barrel_01", 0)
    } else {
        ("s_grunt01", "SOLAR EMPIRE VET", "WS_Rifle_Fire", 1)
    }
}

fn co_radio_speaker_and_portrait<'a>(
    view: &'a View,
    req: &bw_rust::mission_messages::Request,
) -> (&'static str, &'a str) {
    let resolved = view
        .session
        .combat
        .sprite_texture_name(req.sprite)
        .unwrap_or("");
    let tex_key = if !resolved.is_empty() && view.ui_handles.contains_key(resolved) {
        resolved
    } else if req.army == "ARMY_UNDERWORLD"
        || req.sprite == ObjectId(50030325)
        || req.sprite == ObjectId(50030326)
    {
        "co_l_ferrok_h"
    } else if req.army.contains("ANGLO") {
        "co_ai_pierce_h"
    } else if req.army.contains("WF") || req.army.contains("FRONTIER") {
        "co_wf_yherman_h"
    } else if req.sprite == ObjectId(1100005122) || req.sprite == ObjectId(1100005128) {
        "co_sleiqo_h"
    } else {
        "co_sleiqohel_h"
    };

    let speaker = if tex_key.starts_with("co_s_aqira") {
        "ADMIRAL A-QIRA // SOLAR EMPIRE"
    } else if tex_key.starts_with("co_sleiqo") {
        "EMPRESS LEI-QO // SOLAR EMPIRE"
    } else if tex_key.starts_with("co_ai_pierce") {
        "COMMANDER PIERCE // ANGLO ISLES"
    } else if tex_key.starts_with("co_aiwindsor") {
        "COLONEL WINDSOR // ANGLO ISLES"
    } else if tex_key.starts_with("co_l_ferrok") {
        "LORD FERROK // IRON LEGION"
    } else if tex_key.starts_with("co_wf_yherman") || tex_key.starts_with("co_wfherman") {
        "GENERAL HERMAN // WESTERN FRONTIER"
    } else if tex_key.starts_with("co_wfbetty") {
        "BRIGADIER BETTY // WESTERN FRONTIER"
    } else if tex_key.starts_with("co_wfaustin") {
        "COLONEL AUSTIN // WESTERN FRONTIER"
    } else if tex_key.starts_with("co_t_ygorgi") || tex_key.starts_with("co_tgorgi") {
        "TSAR GORGI // TUNDRAN TERRITORIES"
    } else if tex_key.starts_with("co_tnova") {
        "MARSHAL NOVA // TUNDRAN TERRITORIES"
    } else if tex_key.starts_with("co_xvlad") {
        "KAISER VLAD // XYLVANIA"
    } else if tex_key.starts_with("co_xubel") {
        "KOMMANDER UBEL // XYLVANIA"
    } else {
        "EMPRESS LEI-QO // SOLAR EMPIRE"
    };
    (speaker, tex_key)
}

pub fn verify_input(
    view: Option<Res<View>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
) {
    let Some(view) = view else {
        return;
    };
    if view.verify {
        if view.frames == 39 {
            keys.press(KeyCode::KeyW);
            keys.press(KeyCode::ArrowLeft);
        }
        if view.frames == 43 {
            keys.release(KeyCode::ArrowLeft);
        }
        if view.frames == 99 {
            keys.release(KeyCode::KeyW);
        }
        if view.frames == 109 {
            mouse.press(MouseButton::Left);
        }
        if view.frames == 139 {
            mouse.release(MouseButton::Left);
        }
    }
}

pub fn traces(
    view: Option<Res<View>>,
    camera: Query<&Transform, (With<Camera3d>, Without<RifleTrace>)>,
    mut traces: Query<(
        &RifleTrace,
        &mut Transform,
        &mut Visibility,
        &mut MeshMaterial3d<StandardMaterial>,
    )>,
) {
    let Some(view) = view else {
        return;
    };
    let cam_rot = camera
        .iter()
        .next()
        .map(|c| c.rotation)
        .unwrap_or(Quat::IDENTITY);
    let in_mission = view.stage == FrontendStage::InMission;
    for (slot, mut transform, mut visibility, mut mat_handle) in &mut traces {
        if !in_mission {
            if *visibility != Visibility::Hidden {
                *visibility = Visibility::Hidden;
            }
            continue;
        }
        let mut shown = false;
        if slot.0 >= 384 {
            let exp_idx = (slot.0 - 384) / 4;
            let layer = (slot.0 - 384) % 4;
            if let Some(&exp) = view.active_explosions.get(exp_idx) {
                let frac = (exp.age / exp.max_age.max(0.05)).clamp(0.0, 1.0);
                let s = exp.scale.max(0.35);
                let phase = (exp_idx as f32) * 1.17;
                match layer {
                    0 => {
                        // Layer 0: Camera-facing radial starburst flash billboard (first 45% of lifetime)
                        if frac < 0.45 {
                            let f0 = (frac / 0.45).clamp(0.0, 1.0);
                            let r = s * (1.2 + f0.sqrt() * 3.8) * (1.0 - f0 * f0);
                            if r > 0.05 {
                                *transform = Transform {
                                    translation: exp.pos + Vec3::Y * (0.35 * s + f0 * 0.7 * s),
                                    rotation: cam_rot * Quat::from_rotation_z(phase + f0 * 0.9),
                                    scale: Vec3::splat(r),
                                };
                                shown = true;
                            }
                        }
                    }
                    1 => {
                        // Layer 1: Camera-facing 4-frame animated fireball flipbook billboard (first 82% of lifetime)
                        if frac < 0.82 {
                            let f1 = (frac / 0.82).clamp(0.0, 1.0);
                            let r = s * (1.0 + f1.sqrt() * 5.2) * (1.0 - f1.powi(3));
                            if r > 0.05 {
                                let frame_idx = ((f1 * 4.0) as usize).min(3);
                                let desired = &view.vfx_fireball_materials[frame_idx];
                                if mat_handle.0 != *desired {
                                    mat_handle.0 = desired.clone();
                                }
                                *transform = Transform {
                                    translation: exp.pos + Vec3::Y * (0.45 * s + f1 * 2.6 * s),
                                    rotation: cam_rot * Quat::from_rotation_z(-phase + f1 * 1.4),
                                    scale: Vec3::new(r * 1.08, r * 0.96, 1.0),
                                };
                                shown = true;
                            }
                        }
                    }
                    2 => {
                        // Layer 2: Camera-facing 4-frame rising smoke plume flipbook billboard (6% to 100% of lifetime)
                        if frac > 0.06 {
                            let f2 = ((frac - 0.06) / 0.94).clamp(0.0, 1.0);
                            let r = s * (1.1 + f2.sqrt() * 5.6) * (1.0 - f2.powi(3));
                            if r > 0.05 {
                                let frame_idx = ((f2 * 4.0) as usize).min(3);
                                let desired = &view.vfx_smoke_materials[frame_idx];
                                if mat_handle.0 != *desired {
                                    mat_handle.0 = desired.clone();
                                }
                                *transform = Transform {
                                    translation: exp.pos + Vec3::Y * (0.85 * s + f2 * 6.2 * s),
                                    rotation: cam_rot
                                        * Quat::from_rotation_z(phase * 0.5 - f2 * 1.1),
                                    scale: Vec3::new(r * 1.15, r * 0.95, 1.0),
                                };
                                shown = true;
                            }
                        }
                    }
                    _ => {
                        // Layer 3: Expanding horizontal shockwave blast ring (first 58% of lifetime)
                        if frac < 0.58 {
                            let f3 = (frac / 0.58).clamp(0.0, 1.0);
                            let r = s * (1.8 + f3 * 8.8) * (1.0 - f3 * f3);
                            if r > 0.08 {
                                *transform = Transform {
                                    translation: exp.pos + Vec3::Y * 0.15,
                                    rotation: Quat::from_rotation_y(phase + f3 * 0.8),
                                    scale: Vec3::new(r, 1.0, r),
                                };
                                shown = true;
                            }
                        }
                    }
                }
            }
        } else if slot.0 >= 256 {
            if let Some(missile) = view
                .session
                .program
                .host
                .world
                .missiles
                .missiles
                .get(slot.0 - 256)
            {
                *transform = Transform {
                    translation: Vec3::from_array(missile.position),
                    rotation: Quat::from_rotation_arc(
                        Vec3::Z,
                        Vec3::from_array(missile.direction.map(|n| n as f32)),
                    ),
                    scale: Vec3::new(0.14, 0.14, 1.7),
                };
                shown = true;
            }
        } else if slot.0 >= 128 {
            if let Some(bomb) = view
                .session
                .program
                .host
                .world
                .bombers
                .bombs
                .get(slot.0 - 128)
            {
                *transform = Transform::from_translation(Vec3::from_array(bomb.position))
                    .with_scale(Vec3::new(0.32, 1.5, 0.32));
                shown = true;
            }
        } else if let Some(shot) = view.session.combat.shots.get(slot.0) {
            let a = Vec3::from_array(shot.from);
            let b = Vec3::from_array(shot.to);
            let delta = b - a;
            if delta.length() > 0.01 {
                *transform = Transform {
                    translation: (a + b) * 0.5,
                    rotation: Quat::from_rotation_arc(Vec3::Z, delta.normalize()),
                    scale: Vec3::new(0.075, 0.075, delta.length()),
                };
                shown = true;
            }
        }
        let target_vis = if shown {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if *visibility != target_vis {
            *visibility = target_vis;
        }
    }
}

#[allow(clippy::type_complexity)]
pub fn markers(
    mut view: Option<ResMut<View>>,
    loaded: Res<Loaded>,
    camera: Query<&Transform, With<Camera3d>>,
    mut skydome: Query<
        (&SkydomePart, &mut Transform),
        (
            Without<Camera3d>,
            Without<GoldStar>,
            Without<FrontendGlobePart>,
            Without<WaterPlane>,
        ),
    >,
    mut water: Query<
        (&WaterPlane, &mut Transform),
        (
            Without<Camera3d>,
            Without<SkydomePart>,
            Without<GoldStar>,
            Without<FrontendGlobePart>,
        ),
    >,
    mut globe: Query<
        (&FrontendGlobePart, &mut Transform, &mut Visibility),
        (
            Without<Camera3d>,
            Without<SkydomePart>,
            Without<GoldStar>,
            Without<WaterPlane>,
        ),
    >,
    mut markers: Query<
        (&GoldStar, &mut Transform, &mut Visibility),
        (
            Without<Camera3d>,
            Without<SkydomePart>,
            Without<FrontendGlobePart>,
            Without<WaterPlane>,
        ),
    >,
) {
    let Some(view) = view.as_mut() else {
        return;
    };
    let Some(camera) = camera.iter().next() else {
        return;
    };
    let sky_base = Mat4::from_scale_rotation_translation(
        Vec3::splat(3500.0),
        Quat::IDENTITY,
        Vec3::new(camera.translation.x, -40.0, camera.translation.z),
    );
    for (part, mut transform) in &mut skydome {
        *transform = Transform::from_matrix(sky_base * part.0);
    }
    let t = view.frames as f32 * (1.0 / 60.0);
    for (wp, mut transform) in &mut water {
        if wp.layer == 0 {
            let swell = (t * 1.35).sin() * 0.08;
            transform.translation = Vec3::new(
                (t * 0.4).cos() * 1.8,
                wp.base_y + swell,
                (t * 0.35).sin() * 1.8,
            );
        } else {
            let ripple = (t * 2.1 + 1.2).sin() * 0.06;
            transform.translation = Vec3::new(
                (t * 0.75).sin() * 3.2,
                wp.base_y + ripple,
                (t * 0.65).cos() * 3.2,
            );
            transform.rotation = Quat::from_rotation_y((t * 0.12).sin() * 0.015);
        }
    }
    let in_mission = view.stage == FrontendStage::InMission;
    if in_mission {
        for (_, _, mut vis) in &mut globe {
            if *vis != Visibility::Hidden {
                *vis = Visibility::Hidden;
            }
        }
    } else {
        // Map selected campaign mission to original GlobeSelect continent index (1..=5)
        let continent = match view.selected_mission {
            0..=5 => 1,   // Solar Empire (Prologue & Campaign 1)
            6..=8 => 3,   // Anglo Isles (Campaign 2)
            9..=12 => 2,  // Western Frontier (Campaign 3)
            13..=15 => 4, // Iron Legion / Tundran (Campaign 4)
            _ => 5,       // Xylvania / Tundran (Campaign 5)
        };
        let _ = view.fe_runtime.global_call(
            "GlobeSelectTarget",
            &[bw_rust::lua50::Value::Number(continent as f64)],
            100_000,
        );
        let _ = view
            .fe_runtime
            .global_call("GlobeSelectTargetUpdate", &[], 100_000);
        let orient_mat = view
            .fe_runtime
            .global_call("GlobeSelectGetOrientation", &[], 100_000)
            .ok()
            .and_then(|vals| match vals.as_slice() {
                [bw_rust::lua50::Value::Table(tid)] => view.fe_runtime.read_matrix4(*tid).ok(),
                _ => None,
            })
            .map(|m| Mat4::from_cols_array_2d(&m).transpose())
            .unwrap_or(Mat4::IDENTITY);
        let idle_spin = if view.stage == FrontendStage::CampaignSelect {
            Quat::IDENTITY
        } else {
            Quat::from_rotation_y(view.frames as f32 * 0.008)
        };
        let globe_center = camera.translation + camera.forward() * 13.5;
        let rot_mat = Mat4::from_quat(idle_spin) * orient_mat;
        for (gpart, mut transform, mut vis) in &mut globe {
            let base = Mat4::from_translation(globe_center)
                * rot_mat
                * Mat4::from_scale(Vec3::splat(gpart.norm_scale));
            *transform = Transform::from_matrix(base * gpart.local);
            if *vis != Visibility::Visible {
                *vis = Visibility::Visible;
            }
        }
    }
    for (marker, mut transform, mut visibility) in &mut markers {
        if !in_mission {
            if *visibility != Visibility::Hidden {
                *visibility = Visibility::Hidden;
            }
            continue;
        }
        match marker.0.position(&view.session.program.host) {
            Ok(Some(mut p)) => {
                let [x, z] = bw_rust::original_scene::source_to_grid(p[0], p[2]);
                if let Some(h) = loaded.ground.height(x, z) {
                    p[1] = (p[1] + h * 2.).max(view.water_height) + 2.2;
                } else {
                    p[1] = p[1].max(view.water_height) + 2.2;
                }
                let point = Vec3::from_array(p);
                let bob = (view.frames as f32 * 0.07).sin() * 0.32;
                *transform = Transform::from_translation(point + Vec3::Y * bob)
                    .with_rotation(
                        camera.rotation * Quat::from_rotation_z(view.frames as f32 * 0.025),
                    )
                    .with_scale(Vec3::splat(
                        (camera.translation.distance(point) * 0.016).clamp(0.65, 4.5),
                    ));
                if *visibility != Visibility::Visible {
                    *visibility = Visibility::Visible;
                }
            }
            Ok(None) => {
                if *visibility != Visibility::Hidden {
                    *visibility = Visibility::Hidden;
                }
            }
            Err(e) => {
                view.error = Some(e);
                if *visibility != Visibility::Hidden {
                    *visibility = Visibility::Hidden;
                }
            }
        }
    }
}

pub fn lightning(
    view: Option<Res<View>>,
    camera: Query<&Transform, With<Camera3d>>,
    mut renderer: ResMut<bw_rust::mission_lightning_render::Renderer>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if let Some(view) = view
        && let Some(runtime) = &view.session.program.host.world.rewrite
        && let Some(camera) = camera.iter().next()
    {
        renderer.sync(
            runtime.lightning(),
            camera,
            &mut commands,
            &mut meshes,
            &mut materials,
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub fn setup(
    mut commands: Commands,
    mut view: Option<ResMut<View>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut audio_assets: ResMut<Assets<AudioSource>>,
) {
    let Some(view) = view.as_mut() else {
        return;
    };
    let textures = std::mem::take(&mut view.ui_textures);
    let raw_fireblob = textures.get("sfx_fireblobanim").cloned();
    let raw_fireball = textures.get("sfx_fireball").cloned();
    let raw_smoke = textures.get("sfx_smoke").cloned();
    let raw_smoke2 = textures.get("sfx_smoke2").cloned();
    let raw_flare = textures
        .get("sfx_hl_flare")
        .or_else(|| textures.get("sfx_radial_glows"))
        .cloned();
    let raw_shock = textures.get("sfx_shock").cloned();
    for (name, decoded) in textures {
        view.ui_handles
            .insert(name, model_render::image(decoded, &mut images));
    }
    let globe_models = std::mem::take(&mut view.globe_models);
    let mut base_globe_radius = 1.0f32;
    for (g_idx, gm) in globe_models.iter().enumerate() {
        if let Ok(rm) = model_render::build(gm, &view.ui_handles, &mut meshes, &mut materials) {
            if g_idx == 0 {
                let mut max_r = 0.0f32;
                for node in &gm.nodes {
                    let mat = rm.bones.get(&node.name).copied().unwrap_or(Mat4::IDENTITY);
                    for surf in &node.surfaces {
                        for v in &surf.vertices {
                            let pt = mat.transform_point3(Vec3::from_array(v.position));
                            max_r = max_r.max(pt.length());
                        }
                    }
                }
                if max_r > 0.001 {
                    base_globe_radius = max_r;
                }
            }
            let norm_scale = 5.2 / base_globe_radius;
            for p in &rm.parts {
                commands.spawn((
                    FrontendGlobePart {
                        local: p.transform,
                        norm_scale,
                    },
                    Mesh3d(p.mesh.clone()),
                    MeshMaterial3d(p.material.clone()),
                    Transform::default(),
                    Visibility::Hidden,
                ));
            }
        }
    }
    let music_wavs = std::mem::take(&mut view.music_wavs);
    for (name, wav) in music_wavs {
        view.music_handles
            .insert(name, audio_assets.add(AudioSource { bytes: wav.into() }));
    }
    let sfx_wavs = std::mem::take(&mut view.sfx_wavs);
    for (name, wav) in sfx_wavs {
        view.sfx_handles
            .insert(name, audio_assets.add(AudioSource { bytes: wav.into() }));
    }

    spawn_gold_stars(
        &mut commands,
        &view.session.markers,
        &mut meshes,
        &mut materials,
    );

    let mut vertices = Vec::new();
    for i in 0..10 {
        let point = |j: usize| {
            let a = std::f32::consts::FRAC_PI_2 + j as f32 * std::f32::consts::PI / 5.;
            let r = if j.is_multiple_of(2) { 1.0 } else { 0.42 };
            [a.cos() * r, a.sin() * r, 0.]
        };
        vertices.extend([[0.; 3], point(i), point(i + 1)]);
    }
    let star_mesh = meshes.add(
        Mesh::new(
            bevy::mesh::PrimitiveTopology::TriangleList,
            bevy::asset::RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vertices)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0., 0., 1.]; 30]),
    );

    let beam = meshes.add(Cuboid::new(1., 1., 1.));
    let beam_material = materials.add(StandardMaterial {
        base_color: Color::srgb(1., 0.62, 0.14),
        emissive: LinearRgba::new(2.5, 1.1, 0.2, 1.0),
        unlit: true,
        ..default()
    });
    let make_vfx_tex = |images: &mut Assets<Image>, kind: u8, frame: usize| -> Handle<Image> {
        let size = 64usize;
        let mut rgba = vec![0u8; size * size * 4];
        for y in 0..size {
            let ny = (y as f32 + 0.5) / (size as f32) * 2.0 - 1.0;
            for x in 0..size {
                let nx = (x as f32 + 0.5) / (size as f32) * 2.0 - 1.0;
                let r = (nx * nx + ny * ny).sqrt();
                let angle = ny.atan2(nx);
                let f_off = frame as f32 * 0.85;
                let (cr, cg, cb, ca) = match kind {
                    0 => {
                        let rays = 0.85 + 0.15 * (angle * 6.0).cos();
                        let a = ((1.0 - r / rays).clamp(0.0, 1.0)).powi(2);
                        (255, 246, 205, (a * 255.0) as u8)
                    }
                    1 => {
                        let lobe = 0.82
                            + 0.12 * (angle * 5.0 + r * 3.0 + f_off).sin()
                            + 0.08 * (angle * 3.0 - r * 2.0 - f_off * 0.7).cos();
                        let a = ((1.0 - r / lobe).clamp(0.0, 1.0)).powf(1.5);
                        let g = (220.0 * (1.0 - r * (0.50 + frame as f32 * 0.05)).clamp(0.0, 1.0))
                            as u8;
                        let b = (90.0 * (1.0 - r * 0.85).clamp(0.0, 1.0)) as u8;
                        (255, g, b, (a * 245.0) as u8)
                    }
                    2 => {
                        let lobe = 0.84
                            + 0.10 * (angle * 4.0 + 1.1 + f_off).sin()
                            + 0.08 * (angle * 7.0 - 0.6 - f_off * 0.6).cos();
                        let a = ((1.0 - r / lobe).clamp(0.0, 1.0)).powf(1.6) * 0.82;
                        let shade = (38.0 + (1.0 - r).clamp(0.0, 1.0) * 28.0) as u8;
                        (
                            shade,
                            shade.saturating_sub(2),
                            shade.saturating_sub(4),
                            (a * 255.0) as u8,
                        )
                    }
                    _ => {
                        let ring = (1.0 - ((r - 0.68).abs() / 0.24).clamp(0.0, 1.0)).powi(2);
                        (255, 195, 95, (ring * 235.0) as u8)
                    }
                };
                let idx = (y * size + x) * 4;
                rgba[idx] = cr;
                rgba[idx + 1] = cg;
                rgba[idx + 2] = cb;
                rgba[idx + 3] = ca;
            }
        }
        model_render::image(
            texture::DecodedTexture {
                width: size,
                height: size,
                rgba,
            },
            images,
        )
    };
    // Helper that converts a decoded archive TXET chunk (or a 2x2 flipbook quadrant of one)
    // into a soft-edged alpha-masked particle sprite texture.
    let mask_archive_vfx = |src: &texture::DecodedTexture,
                            quadrant: Option<usize>,
                            luma_to_alpha: bool,
                            images: &mut Assets<Image>|
     -> Handle<Image> {
        let (w, h, ox, oy) = if let Some(q) = quadrant {
            let qw = (src.width / 2).max(1);
            let qh = (src.height / 2).max(1);
            (qw, qh, (q % 2) * qw, (q / 2) * qh)
        } else {
            (src.width.max(1), src.height.max(1), 0, 0)
        };
        let mut rgba = vec![0u8; w * h * 4];
        for y in 0..h {
            let ny = (y as f32 + 0.5) / (h as f32) * 2.0 - 1.0;
            for x in 0..w {
                let nx = (x as f32 + 0.5) / (w as f32) * 2.0 - 1.0;
                let r = (nx * nx + ny * ny).sqrt();
                let radial_mask = ((1.0 - r) / 0.28).clamp(0.0, 1.0).powf(1.35);
                let s_idx = ((oy + y) * src.width + (ox + x)) * 4;
                let (sr, sg, sb, sa) = if s_idx + 3 < src.rgba.len() {
                    (
                        src.rgba[s_idx],
                        src.rgba[s_idx + 1],
                        src.rgba[s_idx + 2],
                        src.rgba[s_idx + 3],
                    )
                } else {
                    (255, 255, 255, 0)
                };
                let luma = (sr as f32 * 0.299 + sg as f32 * 0.587 + sb as f32 * 0.114) / 255.0;
                let base_a = if luma_to_alpha {
                    (sa as f32 / 255.0) * luma.powf(0.85)
                } else {
                    (sa as f32 / 255.0).max(luma)
                };
                let final_a = (base_a * radial_mask * 255.0).clamp(0.0, 255.0) as u8;
                let d_idx = (y * w + x) * 4;
                rgba[d_idx] = sr;
                rgba[d_idx + 1] = sg;
                rgba[d_idx + 2] = sb;
                rgba[d_idx + 3] = final_a;
            }
        }
        model_render::image(
            texture::DecodedTexture {
                width: w,
                height: h,
                rgba,
            },
            images,
        )
    };
    let flash_tex = if let Some(ref tex) = raw_flare {
        mask_archive_vfx(tex, None, true, &mut images)
    } else {
        make_vfx_tex(&mut images, 0, 0)
    };
    let ring_tex = if let Some(ref tex) = raw_shock {
        mask_archive_vfx(tex, None, true, &mut images)
    } else {
        make_vfx_tex(&mut images, 3, 0)
    };
    let fireball_frames: [Handle<Image>; 4] = std::array::from_fn(|f| {
        if let Some(ref blob) = raw_fireblob
            && blob.width >= 64
            && blob.height >= 64
        {
            mask_archive_vfx(blob, Some(f), true, &mut images)
        } else if let Some(ref fb) = raw_fireball {
            mask_archive_vfx(fb, None, true, &mut images)
        } else {
            make_vfx_tex(&mut images, 1, f)
        }
    });
    let smoke_frames: [Handle<Image>; 4] = std::array::from_fn(|f| {
        if f >= 2
            && let Some(ref sm2) = raw_smoke2
        {
            mask_archive_vfx(sm2, None, true, &mut images)
        } else if let Some(ref sm) = raw_smoke {
            mask_archive_vfx(sm, None, true, &mut images)
        } else {
            make_vfx_tex(&mut images, 2, f)
        }
    });
    let billboard_mesh = meshes.add(Rectangle::new(1.0, 1.0));
    let shockwave_mesh = meshes.add(Plane3d::default().mesh().size(1.0, 1.0));
    let flash_material = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.96, 0.78, 0.96),
        base_color_texture: Some(flash_tex),
        emissive: LinearRgba::new(6.8, 4.8, 1.9, 1.0),
        alpha_mode: AlphaMode::Add,
        unlit: true,
        cull_mode: None,
        ..default()
    });
    let fireball_materials: [Handle<StandardMaterial>; 4] = std::array::from_fn(|f| {
        let heat = 1.0 - (f as f32) * 0.16;
        materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.62 * heat, 0.18 * heat, 0.94),
            base_color_texture: Some(fireball_frames[f].clone()),
            emissive: LinearRgba::new(4.2 * heat, 1.55 * heat, 0.22 * heat, 1.0),
            alpha_mode: AlphaMode::Add,
            unlit: true,
            cull_mode: None,
            ..default()
        })
    });
    let smoke_materials: [Handle<StandardMaterial>; 4] = std::array::from_fn(|f| {
        let fade = 1.0 - (f as f32) * 0.14;
        materials.add(StandardMaterial {
            base_color: Color::srgba(0.26 * fade, 0.24 * fade, 0.22 * fade, 0.80 * fade),
            base_color_texture: Some(smoke_frames[f].clone()),
            perceptual_roughness: 0.98,
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            cull_mode: None,
            ..default()
        })
    });
    view.vfx_fireball_materials = fireball_materials.clone();
    view.vfx_smoke_materials = smoke_materials.clone();
    let shockwave_material = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.74, 0.30, 0.78),
        base_color_texture: Some(ring_tex),
        emissive: LinearRgba::new(2.6, 1.15, 0.24, 1.0),
        alpha_mode: AlphaMode::Add,
        unlit: true,
        cull_mode: None,
        ..default()
    });
    for i in 0..512 {
        let (mesh_h, mat) = if i >= 384 {
            match (i - 384) % 4 {
                0 => (billboard_mesh.clone(), flash_material.clone()),
                1 => (billboard_mesh.clone(), fireball_materials[0].clone()),
                2 => (billboard_mesh.clone(), smoke_materials[0].clone()),
                _ => (shockwave_mesh.clone(), shockwave_material.clone()),
            }
        } else {
            (beam.clone(), beam_material.clone())
        };
        commands.spawn((
            RifleTrace(i),
            Mesh3d(mesh_h),
            MeshMaterial3d(mat),
            Transform::default(),
            Visibility::Hidden,
        ));
    }

    let squad_bar_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.18, 0.92, 0.65),
        unlit: true,
        ..default()
    });
    let enemy_bar_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.22, 0.14),
        unlit: true,
        ..default()
    });
    for i in 0..32 {
        commands.spawn((
            WorldHealthBar(i),
            Mesh3d(beam.clone()),
            MeshMaterial3d(if i < 12 {
                squad_bar_mat.clone()
            } else {
                enemy_bar_mat.clone()
            }),
            Transform::default(),
            Visibility::Hidden,
        ));
    }
    let lock_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.88, 0.12),
        unlit: true,
        cull_mode: None,
        ..default()
    });
    commands.spawn((
        TargetLockBracket,
        Mesh3d(star_mesh),
        MeshMaterial3d(lock_mat),
        Transform::default(),
        Visibility::Hidden,
    ));

    let tex = |name: &str| view.ui_handles.get(name).cloned().unwrap_or_default();
    let sleiqo_hel = view
        .ui_handles
        .get("co_sleiqohel_h")
        .or_else(|| view.ui_handles.get("co_sleiqo_h"))
        .cloned()
        .unwrap_or_default();
    let co_left = tex("co_dialog_lft_hi");
    let co_mid = tex("co_dialogue_mid");
    let co_rt = tex("co_dialog_rt");
    let co_flash = tex("codialogueflash");
    let radwaves = tex("icon_radwaves");
    let obj_star = view
        .ui_handles
        .get("obj_star_primary")
        .or_else(|| view.ui_handles.get("hud_star"))
        .cloned()
        .unwrap_or_default();
    let radar_lense = tex("radar_lense");
    let radar_vcone = tex("radar_vcone");
    let radar_bezel = tex("skipse_radarbez");
    let radar_blip_dot = tex("radar_blip_small");
    let ret_regl = tex("ret_regl");
    let ret_enemy = tex("ret_enemy");
    let power_left = tex("power_up_02");
    let power_right = tex("power_up_01");
    let skipse_hub1 = tex("skipse_hub1");
    let skipse_hub2 = tex("skipse_hub2");
    let skipuw_hub1 = tex("skipuw_hub1");
    let hub_glow = tex("hud_hubglow");
    let select_stack = tex("select_stack");
    let hud_p1 = tex("hud_p1");
    let health_bg = tex("hud_health_bg");
    let health_fill = tex("hud_health_fill");
    let idtab_box = tex("idtab_box");
    let icon_all_s = tex("icon_all_s");
    let grunt_icon = tex("s_grunt01");
    let hose_icon = tex("s_hose01");
    let baz_icon = tex("s_antiarm01");
    let htnk_icon = tex("s_htnk01");
    let u_grunt_icon = tex("u_grunt01");
    let follow_icon = tex("icon_follow_single");
    let wii_a = tex("wii_abutton");
    let wii_b = tex("wii_bbutton");
    let cursor_01 = tex("cursor_01");

    // Authentic Center Aiming Reticle (`cReticuleState`) + Power/Heat Arcs (`POWER_UP_02` / `POWER_UP_01`) + Enemy Lock Pill
    commands
        .spawn((
            UiPanelSlot::Crosshair,
            Node {
                position_type: PositionType::Absolute,
                left: percent(50.0),
                top: percent(47.0),
                margin: UiRect {
                    left: px(-95),
                    top: px(-42),
                    ..default()
                },
                width: px(190),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(4),
                ..default()
            },
            GlobalZIndex(90),
        ))
        .with_children(|xhair| {
            xhair
                .spawn(Node {
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    column_gap: px(2),
                    ..default()
                })
                .with_children(|row| {
                    row.spawn((
                        UiImageSlot::ReticlePowerLeft,
                        ImageNode::new(power_left).with_color(Color::srgba(0.75, 0.96, 1.0, 0.85)),
                        Node {
                            width: px(32),
                            height: px(64),
                            ..default()
                        },
                    ));
                    row.spawn(Node {
                        width: px(68),
                        height: px(68),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        ..default()
                    })
                    .with_children(|center| {
                        center.spawn((
                            UiImageSlot::ReticleCenter,
                            ImageNode::new(ret_regl),
                            Node {
                                position_type: PositionType::Absolute,
                                width: px(68),
                                height: px(68),
                                ..default()
                            },
                        ));
                        center.spawn((
                            UiImageSlot::ReticleEnemyTag,
                            ImageNode::new(ret_enemy),
                            Node {
                                position_type: PositionType::Absolute,
                                width: px(68),
                                height: px(68),
                                ..default()
                            },
                            Visibility::Hidden,
                        ));
                    });
                    row.spawn((
                        UiImageSlot::ReticlePowerRight,
                        {
                            let mut img = ImageNode::new(power_right)
                                .with_color(Color::srgba(0.75, 0.96, 1.0, 0.85));
                            img.flip_x = true;
                            img
                        },
                        Node {
                            width: px(32),
                            height: px(64),
                            ..default()
                        },
                    ));
                });

            // Target Lock Enemy Status Pill below Reticle
            xhair
                .spawn((
                    UiPanelSlot::TargetInfoPill,
                    Node {
                        padding: UiRect::axes(px(8), px(4)),
                        column_gap: px(6),
                        align_items: AlignItems::Center,
                        border: UiRect::all(px(1)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.08, 0.02, 0.05, 0.86)),
                    BorderColor::all(Color::srgb(1.0, 0.35, 0.35)),
                    Visibility::Hidden,
                ))
                .with_children(|pill| {
                    pill.spawn(Node {
                        width: px(38),
                        height: px(28),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        ..default()
                    })
                    .with_children(|icon_box| {
                        icon_box.spawn((
                            ImageNode::new(skipuw_hub1),
                            Node {
                                position_type: PositionType::Absolute,
                                width: px(38),
                                height: px(28),
                                ..default()
                            },
                        ));
                        icon_box.spawn((
                            UiImageSlot::TargetEnemyIcon,
                            ImageNode::new(u_grunt_icon),
                            Node {
                                width: px(26),
                                height: px(26),
                                ..default()
                            },
                        ));
                    });
                    pill.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: px(2),
                        ..default()
                    })
                    .with_children(|col| {
                        col.spawn((
                            UiTextSlot::TargetEnemyLabel,
                            Text::new("IRON LEGION GRUNT"),
                            TextFont {
                                font_size: FontSize::Px(11.0),
                                ..default()
                            },
                            TextColor(Color::srgb(1.0, 0.92, 0.25)),
                        ));
                        let _ = (&health_bg, &health_fill);
                        col.spawn((
                            Node {
                                width: px(88),
                                height: px(10),
                                padding: UiRect::all(px(1)),
                                border: UiRect::all(px(1)),
                                align_items: AlignItems::Stretch,
                                justify_content: JustifyContent::FlexStart,
                                overflow: Overflow::clip(),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.04, 0.08, 0.14, 0.94)),
                            BorderColor::all(Color::srgb(0.76, 0.84, 0.92)),
                        ))
                        .with_children(|hb| {
                            hb.spawn((
                                TargetHealthBarFill,
                                BackgroundColor(Color::srgb(0.95, 0.25, 0.20)),
                                Node {
                                    width: percent(100.0),
                                    min_width: px(0),
                                    height: percent(100.0),
                                    ..default()
                                },
                            ));
                        });
                    });
                });
        });

    commands.spawn((
        Hud,
        Text::new("Battalion Wars 2 - Starting..."),
        TextFont {
            font_size: FontSize::Px(10.0),
            ..default()
        },
        TextColor(Color::srgba(0.68, 0.76, 0.84, 0.45)),
        BackgroundColor(Color::NONE),
        Node {
            position_type: PositionType::Absolute,
            bottom: px(2),
            right: px(10),
            ..default()
        },
        GlobalZIndex(100),
    ));

    commands.spawn((
        UiPanelSlot::CinematicTop,
        Node {
            position_type: PositionType::Absolute,
            top: px(0),
            left: px(0),
            width: percent(100),
            height: px(54),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.92)),
        Visibility::Hidden,
        GlobalZIndex(75),
    ));
    commands.spawn((
        UiPanelSlot::CinematicBottom,
        Node {
            position_type: PositionType::Absolute,
            bottom: px(0),
            left: px(0),
            width: percent(100),
            height: px(54),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.92)),
        Visibility::Hidden,
        GlobalZIndex(75),
    ));
    commands.spawn((
        Fade,
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            ..default()
        },
        BackgroundColor(Color::NONE),
        GlobalZIndex(50),
    ));

    commands
        .spawn((
            UiPanelSlot::InGameHudRoot,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                ..default()
            },
            GlobalZIndex(95),
        ))
        .with_children(|root| {
            // =================================================================
            // 1. TOP-LEFT: CO RADIO DIALOGUE BOX & OBJECTIVE BANNER (`cHUDVariables`)
            // =================================================================
            root.spawn(Node {
                position_type: PositionType::Absolute,
                top: px(18),
                left: px(22),
                flex_direction: FlexDirection::Column,
                row_gap: px(6),
                ..default()
            })
            .with_children(|top_left| {
                // CO Radio Transmission Box (`mCommanderPos`, `mLeftSidePos`, `mMiddlePos`, `mRightSidePos`)
                top_left
                    .spawn((
                        UiPanelSlot::CoRadio,
                        Node {
                            align_items: AlignItems::Center,
                            min_height: px(84),
                            ..default()
                        },
                        Visibility::Hidden,
                    ))
                    .with_children(|radio| {
                        // Left CO Portrait Frame (`CO_SLeiqoHel_H` / `CO_L_Ferrok_H` + `CODIALOGUEflash`)
                        radio
                            .spawn((
                                Node {
                                    width: px(84),
                                    height: px(82),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::Center,
                                    border: UiRect::all(px(2)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgba(0.04, 0.12, 0.18, 0.95)),
                                BorderColor::all(Color::srgb(0.92, 0.76, 0.28)),
                            ))
                            .with_children(|portrait_box| {
                                portrait_box.spawn((
                                    UiImageSlot::CoRadioPortrait,
                                    ImageNode::new(sleiqo_hel.clone()),
                                    Node {
                                        width: px(76),
                                        height: px(76),
                                        ..default()
                                    },
                                ));
                                portrait_box.spawn((
                                    ImageNode::new(co_flash),
                                    Node {
                                        position_type: PositionType::Absolute,
                                        left: px(-6),
                                        top: px(-6),
                                        width: px(22),
                                        height: px(22),
                                        ..default()
                                    },
                                ));
                            });

                        // Radio Wave Cap (`CO_DIALOG_lft_hi` + `ICON_radwaves`)
                        radio
                            .spawn(Node {
                                width: px(68),
                                height: px(80),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                ..default()
                            })
                            .with_children(|left_cap| {
                                left_cap.spawn((
                                    ImageNode::new(co_left)
                                        .with_color(Color::srgba(0.28, 0.78, 0.95, 0.95)),
                                    Node {
                                        position_type: PositionType::Absolute,
                                        width: px(68),
                                        height: px(80),
                                        ..default()
                                    },
                                ));
                                left_cap.spawn((
                                    ImageNode::new(radwaves),
                                    Node {
                                        width: px(48),
                                        height: px(24),
                                        ..default()
                                    },
                                ));
                            });

                        // Middle Dialogue Body (`CO_DIALOGUE_mid` + Speaker & Subtitle + `CO_DIALOG_rt`)
                        radio
                            .spawn((
                                Node {
                                    width: px(430),
                                    min_height: px(80),
                                    padding: UiRect::axes(px(14), px(8)),
                                    flex_direction: FlexDirection::Column,
                                    justify_content: JustifyContent::Center,
                                    row_gap: px(4),
                                    border: UiRect::all(px(2)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgba(0.02, 0.07, 0.12, 0.92)),
                                BorderColor::all(Color::srgb(0.32, 0.82, 0.96)),
                            ))
                            .with_children(|col| {
                                col.spawn((
                                    ImageNode::new(co_mid)
                                        .with_color(Color::srgba(0.15, 0.45, 0.62, 0.35)),
                                    Node {
                                        position_type: PositionType::Absolute,
                                        left: px(0),
                                        top: px(0),
                                        width: percent(100),
                                        height: percent(100),
                                        ..default()
                                    },
                                ));
                                col.spawn((
                                    ImageNode::new(co_rt)
                                        .with_color(Color::srgba(0.35, 0.85, 1.0, 0.85)),
                                    Node {
                                        position_type: PositionType::Absolute,
                                        right: px(-8),
                                        top: px(6),
                                        width: px(14),
                                        height: px(68),
                                        ..default()
                                    },
                                ));
                                col.spawn((
                                    UiTextSlot::CoRadioSpeaker,
                                    Text::new("EMPRESS LEI-QO // SOLAR EMPIRE"),
                                    TextFont {
                                        font_size: FontSize::Px(13.0),
                                        ..default()
                                    },
                                    TextColor(Color::srgb(0.98, 0.84, 0.32)),
                                ));
                                col.spawn((
                                    UiTextSlot::CoRadioBody,
                                    Text::new(""),
                                    TextFont {
                                        font_size: FontSize::Px(15.0),
                                        ..default()
                                    },
                                    TextColor(Color::WHITE),
                                ));
                            });
                    });

                // Objective Banner Strip (`HUD_Objinner` / `HUD_Objoutline` + `OBJECT_STARS` primary star)
                top_left
                    .spawn((
                        Node {
                            max_width: px(580),
                            padding: UiRect::axes(px(12), px(7)),
                            column_gap: px(10),
                            align_items: AlignItems::Center,
                            border: UiRect::all(px(2)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.03, 0.08, 0.14, 0.88)),
                        BorderColor::all(Color::srgb(0.92, 0.74, 0.24)),
                    ))
                    .with_children(|obj| {
                        obj.spawn((
                            ImageNode::new(obj_star.clone()),
                            Node {
                                width: px(26),
                                height: px(26),
                                ..default()
                            },
                        ));
                        obj.spawn((
                            UiTextSlot::ObjectiveBanner,
                            Text::new("PRIMARY OBJECTIVE: Advance to the Gold Star"),
                            TextFont {
                                font_size: FontSize::Px(14.0),
                                ..default()
                            },
                            TextColor(Color::srgb(0.98, 0.94, 0.74)),
                        ));
                    });
            });

            // =================================================================
            // 2. TOP-RIGHT: ROTATING CIRCULAR RADAR / COMPASS MINIMAP (`mRadarScreenPos`)
            // =================================================================
            root.spawn((
                UiPanelSlot::RadarPanel,
                Node {
                    position_type: PositionType::Absolute,
                    top: px(18),
                    right: px(22),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(4),
                    ..default()
                },
            ))
            .with_children(|radar_col| {
                radar_col
                    .spawn(Node {
                        width: px(168),
                        height: px(168),
                        ..default()
                    })
                    .with_children(|radar| {
                        // 1. Base Solar Empire Metallic Compass Bezel (`SKIPSE_RADARBEZ`)
                        radar.spawn((
                            ImageNode::new(radar_bezel),
                            Node {
                                position_type: PositionType::Absolute,
                                left: px(0),
                                top: px(0),
                                width: px(168),
                                height: px(168),
                                ..default()
                            },
                        ));
                        // 2. Inner dark circular radar lens (`RADAR_LENSE`)
                        radar.spawn((
                            ImageNode::new(radar_lense)
                                .with_color(Color::srgba(0.22, 0.55, 0.42, 0.55)),
                            Node {
                                position_type: PositionType::Absolute,
                                left: px(24),
                                top: px(24),
                                width: px(120),
                                height: px(120),
                                ..default()
                            },
                        ));
                        // 3. Camera FOV View Cone (`RADAR_VCONE`) pointing forward (up on rotating radar)
                        radar.spawn((
                            ImageNode::new(radar_vcone)
                                .with_color(Color::srgba(0.55, 0.95, 0.78, 0.42)),
                            Node {
                                position_type: PositionType::Absolute,
                                left: px(36),
                                top: px(22),
                                width: px(96),
                                height: px(64),
                                ..default()
                            },
                        ));
                        // 4. Dynamic Radar Blips on top of lens & view cone:
                        // Blip 0 = Active Objective Gold Star (`obj_star_primary`)
                        // Blip 1 = Player Center Dot
                        // Blips 2..24 = Friendly Solar Empire & Enemy Iron Legion Units
                        for idx in 0..24 {
                            let is_star = idx == 0;
                            let size = if is_star {
                                18.0
                            } else if idx == 1 {
                                10.0
                            } else {
                                8.0
                            };
                            radar.spawn((
                                RadarBlip(idx),
                                ImageNode::new(if is_star {
                                    obj_star.clone()
                                } else {
                                    radar_blip_dot.clone()
                                }),
                                Node {
                                    position_type: PositionType::Absolute,
                                    left: px(84.0 - size * 0.5),
                                    top: px(84.0 - size * 0.5),
                                    width: px(size),
                                    height: px(size),
                                    ..default()
                                },
                                Visibility::Hidden,
                            ));
                        }
                        // 5. Orbiting North ("N") Indicator on the Compass Ring
                        radar.spawn((
                            UiTextSlot::RadarNorthBadge,
                            Text::new("N"),
                            TextFont {
                                font_size: FontSize::Px(14.0),
                                ..default()
                            },
                            TextColor(Color::srgb(1.0, 0.86, 0.25)),
                            BackgroundColor(Color::srgba(0.05, 0.08, 0.14, 0.88)),
                            Node {
                                position_type: PositionType::Absolute,
                                left: px(76),
                                top: px(4),
                                padding: UiRect::axes(px(4), px(1)),
                                ..default()
                            },
                        ));
                    });

                // Gold Star Compass Distance Readout directly under Radar
                radar_col
                    .spawn((
                        Node {
                            padding: UiRect::axes(px(10), px(3)),
                            border: UiRect::all(px(1)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.02, 0.06, 0.11, 0.88)),
                        BorderColor::all(Color::srgb(0.88, 0.72, 0.24)),
                    ))
                    .with_children(|dist_pill| {
                        dist_pill.spawn((
                            UiTextSlot::RadarDistanceLabel,
                            Text::new("STAR: 27m AHEAD"),
                            TextFont {
                                font_size: FontSize::Px(12.0),
                                ..default()
                            },
                            TextColor(Color::srgb(0.98, 0.90, 0.45)),
                        ));
                    });
            });

            // =================================================================
            // 3. BOTTOM HUD: ACTIVE PLAYER UNIT HUB + HORIZONTAL SQUAD COMMAND BAR
            // =================================================================
            root.spawn((
                UiPanelSlot::SquadCommand,
                Node {
                    position_type: PositionType::Absolute,
                    bottom: px(18),
                    left: px(22),
                    right: px(22),
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::FlexEnd,
                    ..default()
                },
            ))
            .with_children(|bottom_hud| {
                // Left + Center-Left: Active Player Unit Hub (`mCBPlayerScreenPos`) + 5 Squad Command Slots (`mCBScreenPos`)
                bottom_hud
                    .spawn(Node {
                        column_gap: px(18),
                        align_items: AlignItems::FlexEnd,
                        ..default()
                    })
                    .with_children(|left_cluster| {
                        // Active Player Unit Hub (`SKIPSE_Hub1` + `HUD_P1` + `HUD_healthbar`)
                        left_cluster
                            .spawn((
                                Node {
                                    padding: UiRect::all(px(8)),
                                    column_gap: px(10),
                                    align_items: AlignItems::Center,
                                    border: UiRect::all(px(2)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgba(0.03, 0.08, 0.14, 0.90)),
                                BorderColor::all(Color::srgb(0.95, 0.78, 0.28)),
                            ))
                            .with_children(|player_hub| {
                                player_hub
                                    .spawn(Node {
                                        width: px(74),
                                        height: px(74),
                                        align_items: AlignItems::Center,
                                        justify_content: JustifyContent::Center,
                                        ..default()
                                    })
                                    .with_children(|frame| {
                                        frame.spawn((
                                            ImageNode::new(hub_glow.clone())
                                                .with_color(Color::srgba(1.0, 0.9, 0.4, 0.45)),
                                            Node {
                                                position_type: PositionType::Absolute,
                                                width: px(74),
                                                height: px(74),
                                                ..default()
                                            },
                                        ));
                                        frame.spawn((
                                            ImageNode::new(skipse_hub1.clone()),
                                            Node {
                                                position_type: PositionType::Absolute,
                                                width: px(72),
                                                height: px(72),
                                                ..default()
                                            },
                                        ));
                                        frame.spawn((
                                            UiImageSlot::PlayerUnitPortrait,
                                            ImageNode::new(grunt_icon.clone()),
                                            Node {
                                                width: px(54),
                                                height: px(54),
                                                ..default()
                                            },
                                        ));
                                        frame.spawn((
                                            ImageNode::new(hud_p1),
                                            Node {
                                                position_type: PositionType::Absolute,
                                                left: px(2),
                                                top: px(2),
                                                width: px(26),
                                                height: px(26),
                                                ..default()
                                            },
                                        ));
                                    });

                                player_hub
                                    .spawn(Node {
                                        flex_direction: FlexDirection::Column,
                                        row_gap: px(5),
                                        ..default()
                                    })
                                    .with_children(|pcol| {
                                        let _ = &idtab_box;
                                        pcol.spawn((
                                            UiTextSlot::PlayerStatus,
                                            Text::new("SOLAR EMPIRE VET"),
                                            TextFont {
                                                font_size: FontSize::Px(13.0),
                                                ..default()
                                            },
                                            TextColor(Color::srgb(0.98, 0.90, 0.48)),
                                        ));

                                        // Framed Player HUD healthbar track + dynamic solid fill
                                        pcol.spawn((
                                            Node {
                                                width: px(136),
                                                height: px(13),
                                                padding: UiRect::all(px(1)),
                                                border: UiRect::all(px(1)),
                                                align_items: AlignItems::Stretch,
                                                justify_content: JustifyContent::FlexStart,
                                                overflow: Overflow::clip(),
                                                ..default()
                                            },
                                            BackgroundColor(Color::srgba(0.04, 0.08, 0.14, 0.94)),
                                            BorderColor::all(Color::srgb(0.76, 0.84, 0.92)),
                                        ))
                                        .with_children(|hb| {
                                            hb.spawn((
                                                PlayerHealthBarFill,
                                                BackgroundColor(Color::srgb(0.22, 0.95, 0.45)),
                                                Node {
                                                    width: percent(100.0),
                                                    min_width: px(0),
                                                    height: percent(100.0),
                                                    ..default()
                                                },
                                            ));
                                        });

                                        pcol.spawn((
                                            UiTextSlot::PlayerHpText,
                                            Text::new("100%"),
                                            TextFont {
                                                font_size: FontSize::Px(12.0),
                                                ..default()
                                            },
                                            TextColor(Color::srgb(0.78, 0.94, 1.0)),
                                        ));
                                    });
                            });

                        // Horizontal Squad Command Bar (`ICON_all_S`, `S_GRUNT01`, `S_HOSE01`, `S_ANTIARM01`, `S_HTNK01`)
                        left_cluster
                            .spawn(Node {
                                column_gap: px(8),
                                align_items: AlignItems::FlexEnd,
                                ..default()
                            })
                            .with_children(|squad_row| {
                                let slot_icons = [
                                    icon_all_s,
                                    grunt_icon,
                                    hose_icon,
                                    baz_icon,
                                    htnk_icon,
                                ];
                                let slot_defaults = ["ALL (0)", "RIFLE x0", "FLAME x0", "BAZ x0", "HEAVY x0"];
                                for (idx, icon_handle) in slot_icons.into_iter().enumerate() {
                                    squad_row
                                        .spawn(Node {
                                            flex_direction: FlexDirection::Column,
                                            align_items: AlignItems::Center,
                                            row_gap: px(3),
                                            ..default()
                                        })
                                        .with_children(|slot_col| {
                                            // Upper Command Stack Indicator (`Select_stack` + count + mini health bar)
                                            slot_col
                                                .spawn((
                                                    Node {
                                                        width: px(78),
                                                        padding: UiRect::axes(px(5), px(3)),
                                                        flex_direction: FlexDirection::Column,
                                                        align_items: AlignItems::Center,
                                                        row_gap: px(2),
                                                        border: UiRect::all(px(1)),
                                                        ..default()
                                                    },
                                                    BackgroundColor(Color::srgba(
                                                        0.03, 0.08, 0.14, 0.86,
                                                    )),
                                                    BorderColor::all(if idx == 0 {
                                                        Color::srgb(0.95, 0.80, 0.28)
                                                    } else {
                                                        Color::srgb(0.35, 0.58, 0.76)
                                                    }),
                                                ))
                                                .with_children(|stack_top| {
                                                    stack_top.spawn((
                                                        ImageNode::new(select_stack.clone())
                                                            .with_color(Color::srgba(
                                                                0.6, 0.85, 1.0, 0.22,
                                                            )),
                                                        Node {
                                                            position_type: PositionType::Absolute,
                                                            left: px(0),
                                                            top: px(0),
                                                            width: percent(100),
                                                            height: percent(100),
                                                            ..default()
                                                        },
                                                    ));
                                                    stack_top.spawn((
                                                        UiTextSlot::SquadSlotCount(idx),
                                                        Text::new(slot_defaults[idx]),
                                                        TextFont {
                                                            font_size: FontSize::Px(11.0),
                                                            ..default()
                                                        },
                                                        TextColor(Color::srgb(0.98, 0.92, 0.65)),
                                                    ));
                                                    stack_top
                                                        .spawn((
                                                            Node {
                                                                width: px(64),
                                                                height: px(7),
                                                                padding: UiRect::all(px(1)),
                                                                border: UiRect::all(px(1)),
                                                                align_items: AlignItems::Stretch,
                                                                justify_content:
                                                                    JustifyContent::FlexStart,
                                                                overflow: Overflow::clip(),
                                                                ..default()
                                                            },
                                                            BackgroundColor(Color::srgba(
                                                                0.04, 0.08, 0.14, 0.90,
                                                            )),
                                                            BorderColor::all(Color::srgb(
                                                                0.58, 0.72, 0.86,
                                                            )),
                                                        ))
                                                        .with_children(|shb| {
                                                            shb.spawn((
                                                                SquadSlotHealthFill(idx),
                                                                BackgroundColor(Color::NONE),
                                                                Node {
                                                                    width: percent(0.0),
                                                                    min_width: px(0),
                                                                    height: percent(100.0),
                                                                    ..default()
                                                                },
                                                            ));
                                                        });
                                                });

                                            // Lower Unit Hub (`SKIPSE_Hub1` / `SKIPSE_Hub2` + Unit Portrait + Order Badge `ICON_Follow`/`Attack`/`Sentry`)
                                            slot_col
                                                .spawn(Node {
                                                    width: px(78),
                                                    height: px(52),
                                                    align_items: AlignItems::Center,
                                                    justify_content: JustifyContent::Center,
                                                    ..default()
                                                })
                                                .with_children(|hub| {
                                                    hub.spawn((
                                                        UiImageSlot::SquadSlotHub(idx),
                                                        ImageNode::new(if idx == 0 {
                                                            skipse_hub2.clone()
                                                        } else {
                                                            skipse_hub1.clone()
                                                        }),
                                                        Node {
                                                            position_type: PositionType::Absolute,
                                                            width: px(78),
                                                            height: px(52),
                                                            ..default()
                                                        },
                                                    ));
                                                    hub.spawn((
                                                        UiImageSlot::SquadSlotIcon(idx),
                                                        ImageNode::new(icon_handle),
                                                        Node {
                                                            width: px(42),
                                                            height: px(42),
                                                            ..default()
                                                        },
                                                    ));
                                                    hub.spawn((
                                                        UiImageSlot::SquadSlotOrder(idx),
                                                        ImageNode::new(follow_icon.clone()),
                                                        Node {
                                                            position_type: PositionType::Absolute,
                                                            right: px(2),
                                                            top: px(2),
                                                            width: px(22),
                                                            height: px(22),
                                                            ..default()
                                                        },
                                                    ));
                                                });
                                        });
                                }
                            });
                    });

                // Bottom-Right Wii Control Prompts (`Wii_Abutton`, `Wii_bbutton`, `CURSOR_01` + Order Status)
                bottom_hud
                    .spawn((
                        Node {
                            padding: UiRect::axes(px(12), px(8)),
                            column_gap: px(10),
                            align_items: AlignItems::Center,
                            border: UiRect::all(px(2)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.02, 0.06, 0.11, 0.88)),
                        BorderColor::all(Color::srgb(0.42, 0.72, 0.90)),
                    ))
                    .with_children(|ctrl_box| {
                        ctrl_box
                            .spawn(Node {
                                column_gap: px(4),
                                align_items: AlignItems::Center,
                                ..default()
                            })
                            .with_children(|btns| {
                                for icon in [wii_a, wii_b, cursor_01] {
                                    btns.spawn((
                                        ImageNode::new(icon),
                                        Node {
                                            width: px(28),
                                            height: px(28),
                                            ..default()
                                        },
                                    ));
                                }
                            });
                        ctrl_box.spawn((
                            UiTextSlot::SquadStatus,
                            Text::new(
                                "ORDER: FOLLOW\n[C/F] Call  [MMB/G] Attack  [X] Wait  [T] Transfer  [P] Pause",
                            ),
                            TextFont {
                                font_size: FontSize::Px(12.0),
                                ..default()
                            },
                            TextColor(Color::srgb(0.86, 0.94, 1.0)),
                        ));
                    });
            });

            root.spawn((
                UiPanelSlot::TutorialModal,
                Node {
                    position_type: PositionType::Absolute,
                    top: percent(26.),
                    left: percent(22.),
                    width: percent(56.),
                    padding: UiRect::all(px(20)),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(12),
                    border: UiRect::all(px(3)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.02, 0.04, 0.08, 0.96)),
                BorderColor::all(Color::srgb(0.95, 0.78, 0.25)),
                Visibility::Hidden,
            ))
            .with_children(|modal| {
                modal.spawn((
                    UiTextSlot::TutorialModal,
                    Text::new(""),
                    TextFont {
                        font_size: FontSize::Px(18.0),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
            });
        });

    let bg_map = view.ui_handles.get("bg_map03").cloned().unwrap_or_default();
    let bwii_logo = view
        .ui_handles
        .get("bwii_logo")
        .cloned()
        .unwrap_or_default();
    let solar_logo = view
        .ui_handles
        .get("logo_s_01")
        .cloned()
        .unwrap_or_default();
    let sleiqo_portrait = view
        .ui_handles
        .get("co_sleiqo_h")
        .cloned()
        .unwrap_or_default();
    let ferrok_portrait = view
        .ui_handles
        .get("co_l_ferrok_h")
        .cloned()
        .unwrap_or_default();
    let grade_s = view
        .ui_handles
        .get("gradeph_s")
        .cloned()
        .unwrap_or_default();

    commands
        .spawn((
            UiPanelSlot::MenuRoot,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::all(px(28)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.03, 0.05, 0.09, 0.42)),
            GlobalZIndex(200),
        ))
        .with_children(|menu| {
            menu.spawn((
                ImageNode::new(bg_map).with_color(Color::srgba(0.92, 0.95, 1.0, 0.24)),
                Node {
                    position_type: PositionType::Absolute,
                    top: px(0),
                    left: px(0),
                    width: percent(100),
                    height: percent(100),
                    ..default()
                },
            ));

            menu.spawn((
                Node {
                    width: percent(100),
                    padding: UiRect::axes(px(18), px(12)),
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    border: UiRect::all(px(2)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.02, 0.04, 0.08, 0.84)),
                BorderColor::all(Color::srgb(0.88, 0.68, 0.22)),
            ))
            .with_children(|header| {
                header
                    .spawn(Node {
                        column_gap: px(14),
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|left| {
                        left.spawn((
                            ImageNode::new(bwii_logo),
                            Node {
                                width: px(150),
                                height: px(56),
                                ..default()
                            },
                        ));
                        left.spawn((
                            ImageNode::new(solar_logo),
                            Node {
                                width: px(48),
                                height: px(48),
                                ..default()
                            },
                        ));
                    });
                header.spawn((
                    UiTextSlot::MenuHeader,
                    Text::new("BATTALION WARS 2 | TITLE SCREEN"),
                    TextFont {
                        font_size: FontSize::Px(20.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.96, 0.84, 0.42)),
                ));
            });

            menu.spawn(Node {
                width: percent(100),
                flex_grow: 1.0,
                margin: UiRect::vertical(px(18)),
                column_gap: px(22),
                ..default()
            })
            .with_children(|body| {
                body.spawn((
                    Node {
                        width: percent(44),
                        padding: UiRect::all(px(18)),
                        flex_direction: FlexDirection::Column,
                        row_gap: px(10),
                        border: UiRect::all(px(2)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.02, 0.04, 0.08, 0.78)),
                    BorderColor::all(Color::srgb(0.45, 0.65, 0.82)),
                ))
                .with_children(|left| {
                    left.spawn((
                        UiTextSlot::MenuLeftTitle,
                        Text::new("SELECT OPTION"),
                        TextFont {
                            font_size: FontSize::Px(20.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.96, 0.82, 0.35)),
                    ));
                    for idx in 0..6 {
                        left.spawn((
                            Button,
                            MenuItemButton(idx),
                            Node {
                                width: percent(100),
                                padding: UiRect::axes(px(14), px(11)),
                                border: UiRect::all(px(2)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.06, 0.11, 0.18, 0.92)),
                            BorderColor::all(Color::srgb(0.32, 0.48, 0.65)),
                        ))
                        .with_children(|btn| {
                            btn.spawn((
                                UiTextSlot::MenuItem(idx),
                                Text::new(""),
                                TextFont {
                                    font_size: FontSize::Px(16.0),
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                            ));
                        });
                    }
                });

                body.spawn((
                    Node {
                        width: percent(56),
                        padding: UiRect::all(px(20)),
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::SpaceBetween,
                        border: UiRect::all(px(2)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.02, 0.04, 0.08, 0.80)),
                    BorderColor::all(Color::srgb(0.88, 0.68, 0.22)),
                ))
                .with_children(|right| {
                    right
                        .spawn(Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: px(12),
                            ..default()
                        })
                        .with_children(|top| {
                            top.spawn(Node {
                                column_gap: px(16),
                                align_items: AlignItems::Center,
                                ..default()
                            })
                            .with_children(|portraits| {
                                portraits.spawn((
                                    UiImageSlot::MenuPortraitLeft,
                                    ImageNode::new(sleiqo_portrait),
                                    Node {
                                        width: px(84),
                                        height: px(84),
                                        ..default()
                                    },
                                ));
                                portraits.spawn((
                                    UiImageSlot::MenuPortraitRight,
                                    ImageNode::new(ferrok_portrait),
                                    Node {
                                        width: px(84),
                                        height: px(84),
                                        ..default()
                                    },
                                ));
                                portraits.spawn((
                                    UiImageSlot::MenuGradeBadge,
                                    ImageNode::new(grade_s.clone()),
                                    Node {
                                        width: px(72),
                                        height: px(72),
                                        ..default()
                                    },
                                    Visibility::Hidden,
                                ));
                                portraits.spawn((
                                    UiImageSlot::MovieScreen,
                                    ImageNode::new(grade_s),
                                    Node {
                                        width: px(360),
                                        height: px(216),
                                        ..default()
                                    },
                                    Visibility::Hidden,
                                ));
                            });

                            top.spawn((
                                UiTextSlot::MenuRightTitle,
                                Text::new("BATTALION WARS 2"),
                                TextFont {
                                    font_size: FontSize::Px(22.0),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.98, 0.86, 0.38)),
                            ));
                            top.spawn((
                                UiTextSlot::MenuRightBody,
                                Text::new(""),
                                TextFont {
                                    font_size: FontSize::Px(16.0),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.90, 0.94, 0.98)),
                            ));
                            top.spawn((
                                UiTextSlot::MenuRightStatus,
                                Text::new(""),
                                TextFont {
                                    font_size: FontSize::Px(15.0),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.35, 0.95, 0.68)),
                            ));
                        });

                    right
                        .spawn(Node {
                            column_gap: px(14),
                            margin: UiRect::top(px(12)),
                            ..default()
                        })
                        .with_children(|actions| {
                            actions
                                .spawn((
                                    Button,
                                    MenuActionBtn::Primary,
                                    Node {
                                        padding: UiRect::axes(px(20), px(12)),
                                        border: UiRect::all(px(2)),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgb(0.14, 0.42, 0.24)),
                                    BorderColor::all(Color::srgb(0.95, 0.82, 0.32)),
                                ))
                                .with_children(|btn| {
                                    btn.spawn((
                                        UiTextSlot::MenuPrimaryLabel,
                                        Text::new("CONFIRM [ENTER]"),
                                        TextFont {
                                            font_size: FontSize::Px(16.0),
                                            ..default()
                                        },
                                        TextColor(Color::WHITE),
                                    ));
                                });
                            actions
                                .spawn((
                                    Button,
                                    MenuActionBtn::Secondary,
                                    Node {
                                        padding: UiRect::axes(px(18), px(12)),
                                        border: UiRect::all(px(2)),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgb(0.18, 0.12, 0.14)),
                                    BorderColor::all(Color::srgb(0.65, 0.45, 0.48)),
                                ))
                                .with_children(|btn| {
                                    btn.spawn((
                                        UiTextSlot::MenuSecondaryLabel,
                                        Text::new("BACK [ESC]"),
                                        TextFont {
                                            font_size: FontSize::Px(15.0),
                                            ..default()
                                        },
                                        TextColor(Color::srgb(0.92, 0.86, 0.86)),
                                    ));
                                });
                        });
                });
            });
        });
}

fn start_selected_movie(view: &mut View, images: &mut Assets<Image>) {
    let Some(meta) = FMV_MOVIES.get(view.movie_index) else {
        return;
    };
    let path = view.data_dir.join("Video").join(meta.filename);
    let Ok(info) = bw_rust::movie::Info::probe(&path) else {
        return;
    };
    let Ok(wav) = info.audio_wav() else {
        return;
    };
    let Ok(decoder) = bw_rust::movie::Decoder::start(info.clone()) else {
        return;
    };
    let img = Image::new_fill(
        bevy::render::render_resource::Extent3d {
            width: info.width,
            height: info.height,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        &[0, 0, 0, 255],
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::all(),
    );
    let handle = images.add(img);
    let _ = view.fe_runtime.global_call(
        "PlayVideo",
        &[
            bw_rust::lua50::Value::Number(1.0),
            bw_rust::lua50::Value::String(meta.filename.to_string()),
        ],
        10_000,
    );
    view.active_movie = Some(ActiveMoviePlayer {
        filename: meta.filename.to_string(),
        decoder: std::sync::Mutex::new(decoder),
        info,
        wav: Some(wav),
        image: handle,
        pending: None,
        started: false,
        elapsed: 0.0,
        last_frame: 0,
    });
}

fn handle_menu_confirm(commands: &mut Commands, view: &mut View) {
    match view.stage {
        FrontendStage::Title => {
            let _ = view
                .fe_runtime
                .dispatch_page_callback("Start", "eGUIEvent_OnAB", &[], 100_000);
            play_sfx(commands, view, "UI_FE_Open");
            view.stage = FrontendStage::SaveSelect;
        }
        FrontendStage::SaveSelect => {
            play_sfx(commands, view, "UI_FE_Open");
            if view.slots[view.selected_slot].is_some() {
                view.save_to_disk();
                view.stage = FrontendStage::MainMenu;
            } else {
                if view.name_buffer.is_empty() {
                    view.name_buffer = "COMMANDER".into();
                }
                view.stage = FrontendStage::NameEntry;
            }
        }
        FrontendStage::NameEntry => {
            let trimmed = view.name_buffer.trim();
            let final_name = if trimmed.is_empty() {
                "COMMANDER".to_string()
            } else {
                trimmed.to_string()
            };
            view.slots[view.selected_slot] = Some(SaveProfile {
                name: final_name,
                levels_done: if view.menu_smoke { 0 } else { 20 },
                sp_0_1_scores: None,
                sp_0_1_grade: None,
                mission_scores: [None; 20],
                mission_grades: std::array::from_fn(|_| None),
            });
            view.save_to_disk();
            play_sfx(commands, view, "UI_FE_Open");
            view.main_menu_index = 0;
            view.stage = FrontendStage::MainMenu;
        }
        FrontendStage::MainMenu => match view.main_menu_index {
            0 => {
                play_sfx(commands, view, "UI_FE_Page");
                view.mission2_selected_notice.clear();
                view.stage = FrontendStage::CampaignSelect;
            }
            1 => {
                play_sfx(commands, view, "UI_FE_Page");
                view.multiplayer_index = 0;
                view.stage = FrontendStage::MultiplayerInfo;
            }
            2 => {
                play_sfx(commands, view, "UI_FE_Page");
                view.options_index = 0;
                view.stage = FrontendStage::Options;
            }
            3 => {
                play_sfx(commands, view, "UI_FE_Page");
                view.extras_index = 0;
                view.stage = FrontendStage::ExtrasMenu;
            }
            4 => {
                play_sfx(commands, view, "UI_FE_Close");
                view.stage = FrontendStage::SaveSelect;
            }
            5 => {
                view.should_exit = true;
            }
            _ => {}
        },
        FrontendStage::CampaignSelect => {
            let levels_done = view.slots[view.selected_slot]
                .as_ref()
                .map_or(0, |p| p.levels_done);
            if view.menu_smoke && view.selected_mission == 1 && levels_done >= 1 {
                play_sfx(commands, view, "UI_MissionHighlight");
                view.mission2_selected_notice =
                    "MISSION 2 (1. ANGLO ATTACK / SP_1.1) SELECTED IN CAMPAIGN MENU - Ready & gated so selecting Mission 2 does not launch it."
                        .into();
                println!(
                    "SECOND_MISSION_SELECTED_WITHOUT_START level=SP_1.1 title=\"1. ANGLO ATTACK\" stage={:?}",
                    view.stage
                );
            } else if let Some(meta) = CAMPAIGN_1_MISSIONS.get(view.selected_mission) {
                play_sfx(commands, view, "bugle_03");
                if view.current_level != meta.code {
                    view.pending_level = Some(meta.code.to_string());
                } else {
                    view.reset_mission_state();
                    view.stage = FrontendStage::InMission;
                }
            }
        }
        FrontendStage::MultiplayerInfo => {
            if let Some(meta) = MULTIPLAYER_MAPS.get(view.multiplayer_index) {
                play_sfx(commands, view, "bugle_03");
                if view.current_level != meta.code {
                    view.pending_level = Some(meta.code.to_string());
                } else {
                    view.reset_mission_state();
                    view.stage = FrontendStage::InMission;
                }
            }
        }
        FrontendStage::Options => {
            play_sfx(commands, view, "UI_FE_Popup");
            match view.options_index {
                0 => {
                    view.music_volume = if view.music_volume >= 0.99 {
                        0.0
                    } else {
                        (view.music_volume + 0.25).min(1.0)
                    };
                }
                1 => {
                    view.sfx_volume = if view.sfx_volume >= 0.99 {
                        0.0
                    } else {
                        (view.sfx_volume + 0.25).min(1.0)
                    };
                }
                2 => {
                    view.mouse_sensitivity = if view.mouse_sensitivity >= 1.99 {
                        0.5
                    } else {
                        (view.mouse_sensitivity + 0.25).min(2.0)
                    };
                }
                3 => {
                    view.aim_assist_enabled = !view.aim_assist_enabled;
                }
                4 => {
                    view.music_volume = 1.0;
                    view.sfx_volume = 1.0;
                    view.mouse_sensitivity = 1.0;
                    view.aim_assist_enabled = true;
                }
                _ => {
                    view.stage = FrontendStage::MainMenu;
                }
            }
            view.save_to_disk();
        }
        FrontendStage::ExtrasMenu => match view.extras_index {
            0 => {
                play_sfx(commands, view, "UI_FE_Page");
                view.dossier_faction = 0;
                view.dossier_unit = 0;
                view.stage = FrontendStage::UnitDossier;
            }
            1 => {
                play_sfx(commands, view, "UI_FE_Page");
                view.concept_index = 0;
                view.stage = FrontendStage::ConceptGallery;
            }
            2 => {
                play_sfx(commands, view, "UI_FE_Page");
                view.movie_index = 0;
                view.active_movie = None;
                view.stage = FrontendStage::MovieTheater;
            }
            3 => {
                play_sfx(commands, view, "UI_FE_Page");
                view.credits_page = 0;
                view.stage = FrontendStage::Credits;
            }
            _ => {
                play_sfx(commands, view, "UI_FE_Close");
                view.stage = FrontendStage::MainMenu;
            }
        },
        FrontendStage::UnitDossier => {
            play_sfx(commands, view, "UI_FE_Navigate");
            let unit_count = FACTION_DOSSIERS[view.dossier_faction].units.len().max(1);
            view.dossier_unit = (view.dossier_unit + 1) % unit_count;
        }
        FrontendStage::ConceptGallery => {
            play_sfx(commands, view, "UI_FE_Navigate");
            view.concept_index = (view.concept_index + 1) % CONCEPT_GALLERY_PLATES.len();
        }
        FrontendStage::MovieTheater => {
            play_sfx(commands, view, "UI_FE_Open");
            // Signal movie start (handled in update where Assets<Image> is available)
            view.active_movie = None;
        }
        FrontendStage::Credits => {
            play_sfx(commands, view, "UI_FE_Navigate");
            let max_pages = (view.credits_lines.len().div_ceil(14)).max(1);
            view.credits_page = (view.credits_page + 1) % max_pages;
        }
        FrontendStage::Victory => {
            play_sfx(commands, view, "UI_FE_Page");
            let next_idx = (view.current_mission_index() + 1).min(CAMPAIGN_1_MISSIONS.len() - 1);
            view.selected_mission = next_idx;
            if let Some(meta) = CAMPAIGN_1_MISSIONS.get(next_idx) {
                view.mission2_selected_notice = format!(
                    "NEW MISSION UNLOCKED: {} ({}) is now selectable in the menu!",
                    meta.short_title, meta.code
                );
            }
            view.stage = FrontendStage::CampaignSelect;
        }
        FrontendStage::Defeat => {
            play_sfx(commands, view, "bugle_03");
            view.reset_mission_state();
            view.stage = FrontendStage::InMission;
        }
        FrontendStage::InMission => {}
    }
}

fn handle_menu_back(commands: &mut Commands, view: &mut View) {
    match view.stage {
        FrontendStage::Title => {
            view.should_exit = true;
        }
        FrontendStage::SaveSelect => {
            play_sfx(commands, view, "UI_FE_Close");
            view.stage = FrontendStage::Title;
        }
        FrontendStage::NameEntry => {
            play_sfx(commands, view, "UI_FE_Close");
            view.stage = FrontendStage::SaveSelect;
        }
        FrontendStage::MainMenu => {
            play_sfx(commands, view, "UI_FE_Close");
            view.stage = FrontendStage::SaveSelect;
        }
        FrontendStage::CampaignSelect
        | FrontendStage::MultiplayerInfo
        | FrontendStage::Options
        | FrontendStage::ExtrasMenu => {
            play_sfx(commands, view, "UI_FE_Close");
            view.stage = FrontendStage::MainMenu;
        }
        FrontendStage::UnitDossier
        | FrontendStage::ConceptGallery
        | FrontendStage::MovieTheater
        | FrontendStage::Credits => {
            play_sfx(commands, view, "UI_FE_Close");
            view.active_movie = None;
            view.stage = FrontendStage::ExtrasMenu;
        }
        FrontendStage::Victory | FrontendStage::Defeat => {
            play_sfx(commands, view, "UI_FE_Page");
            view.stage = FrontendStage::CampaignSelect;
        }
        FrontendStage::InMission => {}
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn update(
    mut commands: Commands,
    mut view: Option<ResMut<View>>,
    loaded: Res<Loaded>,
    time: Res<Time>,
    input_res: (
        Res<ButtonInput<KeyCode>>,
        Res<ButtonInput<MouseButton>>,
        Option<Res<bevy::input::mouse::AccumulatedMouseMotion>>,
        Query<&Gamepad>,
        Option<Res<bw_rust::controls::Controls>>,
    ),
    mut asset_res: (
        ResMut<Assets<Mesh>>,
        ResMut<Assets<Image>>,
        ResMut<Assets<AudioSource>>,
        Query<(Entity, Option<&mut AudioSink>), With<MusicTrackPlayer>>,
        Query<Entity, With<VoiceTrackPlayer>>,
        Query<&mut bevy::window::CursorOptions, With<bevy::window::PrimaryWindow>>,
    ),
    mut actors: Query<(&Part, &mut Transform, &mut Visibility), Without<Camera3d>>,
    mut world_bars: Query<
        (&WorldHealthBar, &mut Transform, &mut Visibility),
        (Without<Camera3d>, Without<Part>, Without<TargetLockBracket>),
    >,
    mut lock_bracket: Query<
        (&mut Transform, &mut Visibility),
        (With<TargetLockBracket>, Without<Camera3d>, Without<Part>),
    >,
    mut cameras: Query<(&mut Transform, &mut Projection), With<Camera3d>>,
    mut hud: Query<&mut Text, With<Hud>>,
    mut fade: Query<
        &mut BackgroundColor,
        (
            With<Fade>,
            Without<PlayerHealthBarFill>,
            Without<TargetHealthBarFill>,
            Without<SquadSlotHealthFill>,
        ),
    >,
    mut ui_queries: (
        Query<
            (&UiPanelSlot, &mut Visibility),
            (
                Without<Part>,
                Without<WorldHealthBar>,
                Without<TargetLockBracket>,
                Without<UiImageSlot>,
                Without<MenuItemButton>,
                Without<RadarBlip>,
            ),
        >,
        Query<(&UiTextSlot, &mut Text), Without<Hud>>,
        Query<
            (&UiImageSlot, &mut ImageNode, &mut Visibility),
            (
                Without<UiPanelSlot>,
                Without<Part>,
                Without<WorldHealthBar>,
                Without<TargetLockBracket>,
                Without<MenuItemButton>,
                Without<RadarBlip>,
                Without<PlayerHealthBarFill>,
                Without<TargetHealthBarFill>,
                Without<SquadSlotHealthFill>,
            ),
        >,
        Query<
            (&RadarBlip, &mut Node, &mut ImageNode, &mut Visibility),
            (
                Without<UiPanelSlot>,
                Without<UiImageSlot>,
                Without<Part>,
                Without<WorldHealthBar>,
                Without<TargetLockBracket>,
                Without<MenuItemButton>,
                Without<PlayerHealthBarFill>,
                Without<TargetHealthBarFill>,
                Without<SquadSlotHealthFill>,
            ),
        >,
        (
            Query<
                (&mut Node, &mut BackgroundColor),
                (
                    With<PlayerHealthBarFill>,
                    Without<TargetHealthBarFill>,
                    Without<SquadSlotHealthFill>,
                    Without<RadarBlip>,
                    Without<UiImageSlot>,
                    Without<MenuItemButton>,
                    Without<MenuActionBtn>,
                    Without<Fade>,
                ),
            >,
            Query<
                (&mut Node, &mut BackgroundColor),
                (
                    With<TargetHealthBarFill>,
                    Without<PlayerHealthBarFill>,
                    Without<SquadSlotHealthFill>,
                    Without<RadarBlip>,
                    Without<UiImageSlot>,
                    Without<MenuItemButton>,
                    Without<MenuActionBtn>,
                    Without<Fade>,
                ),
            >,
            Query<
                (&SquadSlotHealthFill, &mut Node, &mut BackgroundColor),
                (
                    Without<PlayerHealthBarFill>,
                    Without<TargetHealthBarFill>,
                    Without<RadarBlip>,
                    Without<UiImageSlot>,
                    Without<MenuItemButton>,
                    Without<MenuActionBtn>,
                    Without<Fade>,
                ),
            >,
        ),
        Query<
            (
                &MenuItemButton,
                &Interaction,
                &mut BackgroundColor,
                &mut BorderColor,
                &mut Visibility,
            ),
            (
                Without<UiPanelSlot>,
                Without<UiImageSlot>,
                Without<Part>,
                Without<WorldHealthBar>,
                Without<TargetLockBracket>,
                Without<Fade>,
                Without<RadarBlip>,
                Without<PlayerHealthBarFill>,
                Without<TargetHealthBarFill>,
                Without<SquadSlotHealthFill>,
            ),
        >,
        Query<
            (&MenuActionBtn, &Interaction, &mut BackgroundColor),
            (
                Without<MenuItemButton>,
                Without<Fade>,
                Without<PlayerHealthBarFill>,
                Without<TargetHealthBarFill>,
                Without<SquadSlotHealthFill>,
            ),
        >,
    ),
) {
    let Some(view) = view.as_mut() else {
        return;
    };
    view.frames += 1;
    let (keys, mouse, mouse_motion, pads, controls) = input_res;
    let (
        ref mut meshes,
        ref mut images,
        ref mut audio_assets,
        ref mut music_sinks,
        ref voice_sinks,
        ref mut cursor_options,
    ) = asset_res;
    let (
        ref mut ui_panels,
        ref mut ui_texts,
        ref mut ui_images,
        ref mut radar_blips,
        (ref mut player_hp_fills, ref mut target_hp_fills, ref mut squad_hp_fills),
        ref mut item_buttons,
        ref mut action_buttons,
    ) = ui_queries;

    let Some(controls) = controls else {
        view.error = Some("Missing controls".into());
        return;
    };

    let interactive_gameplay = view.stage == FrontendStage::InMission
        && !view.pause_menu_open
        && !view.session.program.host.world.frozen
        && !view
            .session
            .program
            .host
            .world
            .tutorial
            .as_ref()
            .is_some_and(|t| t.paused)
        && !view.verify
        && view.replay.is_none()
        && std::env::var_os("BW_OFFSCREEN").is_none()
        && std::env::var_os("BW_SMOKE").is_none();
    for mut cursor in cursor_options.iter_mut() {
        let want_grab = if interactive_gameplay {
            bevy::window::CursorGrabMode::Locked
        } else {
            bevy::window::CursorGrabMode::None
        };
        let want_vis = !interactive_gameplay;
        if cursor.grab_mode != want_grab {
            cursor.grab_mode = want_grab;
        }
        if cursor.visible != want_vis {
            cursor.visible = want_vis;
        }
    }

    let mut set_panel_vis = |slot: UiPanelSlot, vis: Visibility| {
        for (s, mut v) in ui_panels.iter_mut() {
            if *s == slot && *v != vis {
                *v = vis;
            }
        }
    };
    let mut set_text = |slot: UiTextSlot, value: String| {
        for (s, mut t) in ui_texts.iter_mut() {
            if *s == slot && **t != value {
                **t = value.clone();
            }
        }
    };

    // Automated BW_MENU_SMOKE end-to-end progression check:
    // Title -> SaveSlot -> NameEntry -> MainMenu -> CampaignSelect -> Start SP_0.1 -> Complete SP_0.1 -> Victory -> Unlock & Select Mission 2 (SP_1.1) without starting!
    if view.menu_smoke {
        match view.frames {
            15 => {
                assert_eq!(view.stage, FrontendStage::Title);
                handle_menu_confirm(&mut commands, view);
                assert_eq!(view.stage, FrontendStage::SaveSelect);
                assert_eq!(
                    view.fe_runtime.stack_state().unwrap().top.as_deref(),
                    Some("LoadCampaignData")
                );
            }
            30 => {
                assert_eq!(view.stage, FrontendStage::SaveSelect);
                handle_menu_confirm(&mut commands, view);
                assert_eq!(view.stage, FrontendStage::NameEntry);
            }
            45 => {
                assert_eq!(view.stage, FrontendStage::NameEntry);
                handle_menu_confirm(&mut commands, view);
                assert_eq!(view.stage, FrontendStage::MainMenu);
            }
            60 => {
                assert_eq!(view.stage, FrontendStage::MainMenu);
                handle_menu_confirm(&mut commands, view);
                assert_eq!(view.stage, FrontendStage::CampaignSelect);
                assert_eq!(
                    view.slots[view.selected_slot].as_ref().unwrap().levels_done,
                    0
                );
            }
            80 => {
                assert_eq!(view.stage, FrontendStage::CampaignSelect);
                view.selected_mission = 0;
                handle_menu_confirm(&mut commands, view);
                assert_eq!(view.stage, FrontendStage::InMission);
            }
            165 => {
                assert_eq!(view.stage, FrontendStage::InMission);
                view.complete_mission_and_open_victory(100.0, 92.0, 88.0, "GRADE_S");
                assert_eq!(view.stage, FrontendStage::Victory);
                assert_eq!(
                    view.slots[view.selected_slot].as_ref().unwrap().levels_done,
                    1
                );
            }
            195 => {
                assert_eq!(view.stage, FrontendStage::Victory);
                handle_menu_confirm(&mut commands, view);
                assert_eq!(view.stage, FrontendStage::CampaignSelect);
                assert_eq!(view.selected_mission, 1);
            }
            215 => {
                assert_eq!(view.stage, FrontendStage::CampaignSelect);
                assert_eq!(view.selected_mission, 1);
                assert_eq!(
                    view.campaign_levels.get(1).map(String::as_str),
                    Some("SP_1.1")
                );
                handle_menu_confirm(&mut commands, view);
                assert_eq!(view.stage, FrontendStage::CampaignSelect);
                assert!(!view.mission2_selected_notice.is_empty());
                println!(
                    "MENU_CAMPAIGN_FLOW_PASS profile={} levels_done=1 selected_mission=1 level=SP_1.1 started=false",
                    view.slots[view.selected_slot].as_ref().unwrap().name
                );
            }
            _ => {}
        }
    }

    let in_mission = view.stage == FrontendStage::InMission;
    set_panel_vis(
        UiPanelSlot::MenuRoot,
        if in_mission {
            Visibility::Hidden
        } else {
            Visibility::Visible
        },
    );
    set_panel_vis(
        UiPanelSlot::InGameHudRoot,
        if in_mission {
            Visibility::Visible
        } else {
            Visibility::Hidden
        },
    );

    if !in_mission {
        set_panel_vis(UiPanelSlot::Crosshair, Visibility::Hidden);
        set_panel_vis(UiPanelSlot::CinematicTop, Visibility::Hidden);
        set_panel_vis(UiPanelSlot::CinematicBottom, Visibility::Hidden);
        set_panel_vis(UiPanelSlot::CoRadio, Visibility::Hidden);
        set_panel_vis(UiPanelSlot::RadarPanel, Visibility::Hidden);
        set_panel_vis(UiPanelSlot::TargetInfoPill, Visibility::Hidden);
        set_panel_vis(UiPanelSlot::SquadCommand, Visibility::Hidden);
        set_panel_vis(UiPanelSlot::TutorialModal, Visibility::Hidden);
        for mut c in &mut fade {
            if c.0 != Color::NONE {
                *c = BackgroundColor(Color::NONE);
            }
        }
        let angle = view.frames as f32 * 0.003;
        let menu_center = Vec3::new(-415.0, 58.0, 260.0);
        let menu_eye = menu_center + Vec3::new(angle.cos() * 32.0, 14.0, angle.sin() * 32.0);
        for (mut transform, _) in &mut cameras {
            *transform = Transform::from_translation(menu_eye).looking_at(menu_center, Vec3::Y);
        }

        if view.stage == FrontendStage::NameEntry {
            if keys.just_pressed(KeyCode::Backspace) {
                view.name_buffer.pop();
                play_sfx(&mut commands, view, "UI_FE_Navigate");
            }
            for (code, ch) in [
                (KeyCode::KeyA, 'A'),
                (KeyCode::KeyB, 'B'),
                (KeyCode::KeyC, 'C'),
                (KeyCode::KeyD, 'D'),
                (KeyCode::KeyE, 'E'),
                (KeyCode::KeyF, 'F'),
                (KeyCode::KeyG, 'G'),
                (KeyCode::KeyH, 'H'),
                (KeyCode::KeyI, 'I'),
                (KeyCode::KeyJ, 'J'),
                (KeyCode::KeyK, 'K'),
                (KeyCode::KeyL, 'L'),
                (KeyCode::KeyM, 'M'),
                (KeyCode::KeyN, 'N'),
                (KeyCode::KeyO, 'O'),
                (KeyCode::KeyP, 'P'),
                (KeyCode::KeyQ, 'Q'),
                (KeyCode::KeyR, 'R'),
                (KeyCode::KeyS, 'S'),
                (KeyCode::KeyT, 'T'),
                (KeyCode::KeyU, 'U'),
                (KeyCode::KeyV, 'V'),
                (KeyCode::KeyW, 'W'),
                (KeyCode::KeyX, 'X'),
                (KeyCode::KeyY, 'Y'),
                (KeyCode::KeyZ, 'Z'),
                (KeyCode::Digit0, '0'),
                (KeyCode::Digit1, '1'),
                (KeyCode::Digit2, '2'),
                (KeyCode::Digit3, '3'),
                (KeyCode::Digit4, '4'),
                (KeyCode::Digit5, '5'),
                (KeyCode::Digit6, '6'),
                (KeyCode::Digit7, '7'),
                (KeyCode::Digit8, '8'),
                (KeyCode::Digit9, '9'),
            ] {
                if keys.just_pressed(code) && view.name_buffer.len() < 14 {
                    view.name_buffer.push(ch);
                    play_sfx(&mut commands, view, "UI_FE_Navigate");
                }
            }
        } else {
            let up = keys.just_pressed(KeyCode::ArrowUp) || keys.just_pressed(KeyCode::KeyW);
            let down = keys.just_pressed(KeyCode::ArrowDown) || keys.just_pressed(KeyCode::KeyS);
            let left_key =
                keys.just_pressed(KeyCode::ArrowLeft) || keys.just_pressed(KeyCode::KeyA);
            let right_key =
                keys.just_pressed(KeyCode::ArrowRight) || keys.just_pressed(KeyCode::KeyD);
            if up || down || left_key || right_key {
                play_sfx(&mut commands, view, "UI_FE_Navigate");
                match view.stage {
                    FrontendStage::SaveSelect if up || down => {
                        view.selected_slot = if down {
                            (view.selected_slot + 1) % 3
                        } else {
                            (view.selected_slot + 2) % 3
                        };
                    }
                    FrontendStage::MainMenu if up || down => {
                        view.main_menu_index = if down {
                            (view.main_menu_index + 1) % 6
                        } else {
                            (view.main_menu_index + 5) % 6
                        };
                    }
                    FrontendStage::CampaignSelect => {
                        let count = CAMPAIGN_1_MISSIONS.len();
                        if up || down {
                            view.selected_mission = if down {
                                (view.selected_mission + 1) % count
                            } else {
                                (view.selected_mission + count - 1) % count
                            };
                        } else if left_key || right_key {
                            view.selected_mission = if right_key {
                                (view.selected_mission + 6) % count
                            } else {
                                (view.selected_mission + count - 6) % count
                            };
                        }
                    }
                    FrontendStage::MultiplayerInfo => {
                        let count = MULTIPLAYER_MAPS.len();
                        if up || down {
                            view.multiplayer_index = if down {
                                (view.multiplayer_index + 1) % count
                            } else {
                                (view.multiplayer_index + count - 1) % count
                            };
                        } else if left_key || right_key {
                            view.multiplayer_index = if right_key {
                                (view.multiplayer_index + 6) % count
                            } else {
                                (view.multiplayer_index + count - 6) % count
                            };
                        }
                    }
                    FrontendStage::Options => {
                        if up || down {
                            view.options_index = if down {
                                (view.options_index + 1) % 6
                            } else {
                                (view.options_index + 5) % 6
                            };
                        } else if left_key || right_key {
                            let step = if right_key { 0.1 } else { -0.1 };
                            match view.options_index {
                                0 => {
                                    view.music_volume = (view.music_volume + step).clamp(0.0, 1.0);
                                }
                                1 => {
                                    view.sfx_volume = (view.sfx_volume + step).clamp(0.0, 1.0);
                                }
                                2 => {
                                    view.mouse_sensitivity =
                                        (view.mouse_sensitivity + step).clamp(0.25, 2.5);
                                }
                                3 => {
                                    view.aim_assist_enabled = !view.aim_assist_enabled;
                                }
                                _ => {}
                            }
                            view.save_to_disk();
                        }
                    }
                    FrontendStage::ExtrasMenu if up || down => {
                        view.extras_index = if down {
                            (view.extras_index + 1) % 5
                        } else {
                            (view.extras_index + 4) % 5
                        };
                    }
                    FrontendStage::UnitDossier => {
                        if up || down {
                            view.dossier_faction = if down {
                                (view.dossier_faction + 1) % FACTION_DOSSIERS.len()
                            } else {
                                (view.dossier_faction + FACTION_DOSSIERS.len() - 1)
                                    % FACTION_DOSSIERS.len()
                            };
                            view.dossier_unit = 0;
                        } else if left_key || right_key {
                            let unit_count =
                                FACTION_DOSSIERS[view.dossier_faction].units.len().max(1);
                            view.dossier_unit = if right_key {
                                (view.dossier_unit + 1) % unit_count
                            } else {
                                (view.dossier_unit + unit_count - 1) % unit_count
                            };
                        }
                    }
                    FrontendStage::ConceptGallery => {
                        let count = CONCEPT_GALLERY_PLATES.len();
                        view.concept_index = if down || right_key {
                            (view.concept_index + 1) % count
                        } else {
                            (view.concept_index + count - 1) % count
                        };
                    }
                    FrontendStage::MovieTheater => {
                        let count = FMV_MOVIES.len();
                        if up || down {
                            view.movie_index = if down {
                                (view.movie_index + 1) % count
                            } else {
                                (view.movie_index + count - 1) % count
                            };
                        } else if left_key || right_key {
                            view.movie_index = if right_key {
                                (view.movie_index + 6) % count
                            } else {
                                (view.movie_index + count - 6) % count
                            };
                        }
                        view.active_movie = None;
                    }
                    FrontendStage::Credits => {
                        let max_pages = (view.credits_lines.len().div_ceil(14)).max(1);
                        view.credits_page = if down || right_key {
                            (view.credits_page + 1) % max_pages
                        } else {
                            (view.credits_page + max_pages - 1) % max_pages
                        };
                    }
                    _ => {}
                }
            }
        }

        let mut clicked_item = None;
        for (btn, interaction, _, _, _) in item_buttons.iter_mut() {
            if *interaction == Interaction::Pressed {
                clicked_item = Some(btn.0);
            }
        }
        if let Some(idx) = clicked_item {
            match view.stage {
                FrontendStage::Title => {
                    handle_menu_confirm(&mut commands, view);
                }
                FrontendStage::SaveSelect if idx < 3 => {
                    view.selected_slot = idx;
                    handle_menu_confirm(&mut commands, view);
                }
                FrontendStage::NameEntry if idx == 0 => {
                    handle_menu_confirm(&mut commands, view);
                }
                FrontendStage::MainMenu if idx < 6 => {
                    view.main_menu_index = idx;
                    handle_menu_confirm(&mut commands, view);
                }
                FrontendStage::CampaignSelect => {
                    let page_start = (view.selected_mission / 6) * 6;
                    let target = page_start + idx;
                    if target < CAMPAIGN_1_MISSIONS.len() {
                        if view.selected_mission != target {
                            view.selected_mission = target;
                            play_sfx(&mut commands, view, "UI_MissionHighlight");
                            view.mission2_selected_notice.clear();
                        } else {
                            handle_menu_confirm(&mut commands, view);
                        }
                    }
                }
                FrontendStage::MultiplayerInfo => {
                    let page_start = (view.multiplayer_index / 6) * 6;
                    let target = page_start + idx;
                    if target < MULTIPLAYER_MAPS.len() {
                        if view.multiplayer_index != target {
                            view.multiplayer_index = target;
                            play_sfx(&mut commands, view, "UI_FE_Navigate");
                        } else {
                            handle_menu_confirm(&mut commands, view);
                        }
                    }
                }
                FrontendStage::Options if idx < 6 => {
                    view.options_index = idx;
                    handle_menu_confirm(&mut commands, view);
                }
                FrontendStage::ExtrasMenu if idx < 5 => {
                    view.extras_index = idx;
                    handle_menu_confirm(&mut commands, view);
                }
                FrontendStage::UnitDossier if idx < FACTION_DOSSIERS.len() => {
                    if view.dossier_faction != idx {
                        view.dossier_faction = idx;
                        view.dossier_unit = 0;
                        play_sfx(&mut commands, view, "UI_FE_Navigate");
                    } else {
                        handle_menu_confirm(&mut commands, view);
                    }
                }
                FrontendStage::ConceptGallery if idx < CONCEPT_GALLERY_PLATES.len() => {
                    view.concept_index = idx;
                    play_sfx(&mut commands, view, "UI_FE_Navigate");
                }
                FrontendStage::MovieTheater => {
                    let page_start = (view.movie_index / 6) * 6;
                    let target = page_start + idx;
                    if target < FMV_MOVIES.len() {
                        view.movie_index = target;
                        view.active_movie = None;
                        start_selected_movie(view, images);
                    }
                }
                FrontendStage::Credits => {
                    if idx < 5 {
                        view.credits_page = idx;
                    } else {
                        handle_menu_confirm(&mut commands, view);
                    }
                }
                FrontendStage::Victory | FrontendStage::Defeat if idx == 0 => {
                    handle_menu_confirm(&mut commands, view);
                }
                _ => {}
            }
        }

        let mut primary_clicked = false;
        let mut secondary_clicked = false;
        for (kind, interaction, mut bg) in action_buttons.iter_mut() {
            let target_bg = match (*kind, *interaction) {
                (MenuActionBtn::Primary, Interaction::Pressed) => {
                    primary_clicked = true;
                    Color::srgb(0.22, 0.56, 0.32)
                }
                (MenuActionBtn::Primary, Interaction::Hovered) => Color::srgb(0.22, 0.56, 0.32),
                (MenuActionBtn::Primary, Interaction::None) => Color::srgb(0.14, 0.42, 0.24),
                (MenuActionBtn::Secondary, Interaction::Pressed) => {
                    secondary_clicked = true;
                    Color::srgb(0.32, 0.18, 0.22)
                }
                (MenuActionBtn::Secondary, Interaction::Hovered) => Color::srgb(0.32, 0.18, 0.22),
                (MenuActionBtn::Secondary, Interaction::None) => Color::srgb(0.18, 0.12, 0.14),
            };
            if bg.0 != target_bg {
                *bg = BackgroundColor(target_bg);
            }
        }

        let confirm_key = keys.just_pressed(KeyCode::Enter)
            || (view.stage != FrontendStage::NameEntry && keys.just_pressed(KeyCode::Space));
        let back_key = keys.just_pressed(KeyCode::Escape);
        if primary_clicked || confirm_key {
            if view.stage == FrontendStage::MovieTheater {
                play_sfx(&mut commands, view, "UI_FE_Open");
                start_selected_movie(view, images);
            } else {
                handle_menu_confirm(&mut commands, view);
            }
        } else if secondary_clicked || back_key {
            handle_menu_back(&mut commands, view);
        }

        if view.stage == FrontendStage::InMission {
            if view.active_briefing_vo.is_some() {
                for entity in voice_sinks.iter() {
                    commands.entity(entity).despawn();
                }
                view.active_briefing_vo = None;
            }
            for (slot, _, mut vis) in ui_images.iter_mut() {
                if matches!(
                    *slot,
                    UiImageSlot::MenuPortraitLeft
                        | UiImageSlot::MenuPortraitRight
                        | UiImageSlot::MenuGradeBadge
                        | UiImageSlot::MovieScreen
                ) && *vis != Visibility::Hidden
                {
                    *vis = Visibility::Hidden;
                }
            }
            for (_, _, _, _, mut vis) in item_buttons.iter_mut() {
                if *vis != Visibility::Hidden {
                    *vis = Visibility::Hidden;
                }
            }
            set_panel_vis(UiPanelSlot::MenuRoot, Visibility::Hidden);
            set_panel_vis(UiPanelSlot::InGameHudRoot, Visibility::Visible);
            return;
        }

        // Mission Briefing Voiceover (.adp) in CampaignSelect
        if view.stage == FrontendStage::CampaignSelect {
            if view.active_briefing_vo != Some(view.selected_mission) {
                view.active_briefing_vo = Some(view.selected_mission);
                for entity in voice_sinks.iter() {
                    commands.entity(entity).despawn();
                }
                if std::env::var_os("BW_OFFSCREEN").is_none()
                    && let Some(meta) = CAMPAIGN_1_MISSIONS.get(view.selected_mission)
                {
                    let vo_stem = meta
                        .code
                        .strip_prefix("SP_")
                        .unwrap_or("0.1")
                        .replace('.', "_");
                    let vo_path = view
                        .data_dir
                        .join("Audio/Voiceover/Mission_Briefs")
                        .join(format!("{vo_stem}.adp"));
                    let _ = view.fe_runtime.global_call(
                        "StartVO",
                        &[
                            bw_rust::lua50::Value::Number(3.0),
                            bw_rust::lua50::Value::String(
                                "Data/Audio/Voiceover/Mission_Briefs/".into(),
                            ),
                            bw_rust::lua50::Value::String(vo_stem),
                        ],
                        10_000,
                    );
                    if let Ok(bytes) = std::fs::read(vo_path)
                        && let Ok(audio) = bw_rust::dsp_audio::Audio::decode(&bytes)
                    {
                        let wav = pcm_to_wav(audio.sample_rate, 1, &audio.pcm);
                        let handle = audio_assets.add(AudioSource { bytes: wav.into() });
                        commands.spawn((
                            AudioPlayer::new(handle),
                            PlaybackSettings::DESPAWN
                                .with_volume(bevy::audio::Volume::Linear(view.sfx_volume)),
                            VoiceTrackPlayer,
                        ));
                    }
                }
            }
        } else if view.active_briefing_vo.is_some() {
            view.active_briefing_vo = None;
            if view.stage != FrontendStage::MovieTheater {
                for entity in voice_sinks.iter() {
                    commands.entity(entity).despawn();
                }
            }
        }

        // Stream .THP video & audio in MovieTheater
        if view.stage == FrontendStage::MovieTheater {
            if view.active_movie.is_none() {
                start_selected_movie(view, images);
            }
            let sfx_vol = view.sfx_volume;
            let dt = time.delta_secs_f64().min(0.1);
            if let Some(movie) = view.active_movie.as_mut() {
                if !movie.started {
                    if let Ok(mut dec) = movie.decoder.lock()
                        && let Ok(Some(frame)) = dec.try_frame()
                    {
                        movie.pending = Some(frame);
                        movie.started = true;
                        for entity in voice_sinks.iter() {
                            commands.entity(entity).despawn();
                        }
                        if std::env::var_os("BW_OFFSCREEN").is_none()
                            && let Some(wav) = movie.wav.take()
                        {
                            let handle = audio_assets.add(AudioSource { bytes: wav.into() });
                            commands.spawn((
                                AudioPlayer::new(handle),
                                PlaybackSettings::DESPAWN
                                    .with_volume(bevy::audio::Volume::Linear(sfx_vol)),
                                VoiceTrackPlayer,
                            ));
                        }
                    }
                } else {
                    movie.elapsed += dt;
                    for _ in 0..8 {
                        if movie.pending.is_none()
                            && let Ok(mut dec) = movie.decoder.lock()
                        {
                            movie.pending = dec.try_frame().ok().flatten();
                        }
                        let Some(frame) = &movie.pending else {
                            break;
                        };
                        if frame.index as f64 / movie.info.frame_rate > movie.elapsed {
                            break;
                        }
                        let frame = movie.pending.take().unwrap();
                        if let Some(mut img) = images.get_mut(&movie.image) {
                            img.data = Some(frame.rgba);
                        }
                        movie.last_frame = frame.index;
                    }
                }
            }
        }

        let profile_badge = view.slots[view.selected_slot]
            .as_ref()
            .map(|p| format!("PROFILE: {} [SLOT {}]", p.name, view.selected_slot + 1))
            .unwrap_or_else(|| "NO PROFILE LOADED".into());
        let stage_title = match view.stage {
            FrontendStage::Title => "TITLE SCREEN",
            FrontendStage::SaveSelect => "SELECT SAVE SLOT",
            FrontendStage::NameEntry => "NEW GAME - ENTER NAME",
            FrontendStage::MainMenu => "MAIN MENU",
            FrontendStage::CampaignSelect => "SOLAR EMPIRE CAMPAIGN SELECT",
            FrontendStage::MultiplayerInfo => "NINTENDO WI-FI CONNECTION",
            FrontendStage::Options => "OPTIONS - AUDIO & CONTROLS",
            FrontendStage::ExtrasMenu => "EXTRAS - ARCHIVES & DOSSIER",
            FrontendStage::UnitDossier => "UNIT DOSSIER - FACTION ENCYCLOPEDIA",
            FrontendStage::ConceptGallery => "PRODUCTION CONCEPT ART GALLERY",
            FrontendStage::MovieTheater => "FMV CUTSCENE THEATER (.THP MOVIES)",
            FrontendStage::Credits => "BATTALION WARS 2 CREDITS",
            FrontendStage::Victory => "MISSION DEBRIEFING - VICTORY!",
            FrontendStage::Defeat => "MISSION DEBRIEFING - DEFEAT",
            FrontendStage::InMission => "IN MISSION",
        };
        set_text(
            UiTextSlot::MenuHeader,
            format!("BATTALION WARS 2 | {stage_title} | {profile_badge}"),
        );

        let sleiqo = view
            .ui_handles
            .get("co_sleiqo_h")
            .cloned()
            .unwrap_or_default();
        let ferrok = view
            .ui_handles
            .get("co_l_ferrok_h")
            .cloned()
            .unwrap_or_default();

        let mut item_texts = [
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
        ];
        let mut active_count = 0usize;
        let mut selected_idx = 0usize;
        let mut left_header = String::new();
        let mut right_header = String::new();
        let mut right_desc = String::new();
        let mut right_status = String::new();
        let mut primary_text = String::new();
        let mut secondary_text = String::new();
        let mut left_portrait = sleiqo.clone();
        let mut right_portrait = ferrok.clone();
        let mut show_grade: Option<String> = None;

        match view.stage {
            FrontendStage::Title => {
                left_header = "BATTALION WARS 2".into();
                active_count = 1;
                selected_idx = 0;
                item_texts[0] = "> PRESS A & B / CLICK OR PRESS ENTER TO BEGIN".into();
                right_header = "ALL 5 CAMPAIGNS & 16 MULTIPLAYER MAPS".into();
                right_desc = "Welcome to Battalion Wars 2!\n\n* Full 1-to-1 skeletal animations, textures, water physics, voiceover, music & SFX\n* Complete Wii Remote + Nunchuk controls remapped to Mouse & Keyboard\n* All 20 Single-Player Campaign Missions (SP_0.1 .. SP_5.4) + All 16 Multiplayer Maps (MP1 .. MP17)\n* 3D Interactive World Globe, Briefing Voiceovers, 15 .THP FMV Movies & Local Save Persistence".into();
                right_status = "PLEASE PRESS [ENTER], [SPACE], OR CLICK TO OPEN SAVE SLOTS".into();
                primary_text = "START GAME [ENTER]".into();
                secondary_text = "QUIT [ESC]".into();
            }
            FrontendStage::SaveSelect => {
                left_header = "SELECT A SAVE SLOT (Up/Down or Click)".into();
                active_count = 3;
                selected_idx = view.selected_slot;
                for (i, item_text) in item_texts.iter_mut().enumerate().take(3) {
                    *item_text = match &view.slots[i] {
                        Some(p) => format!(
                            "SLOT {} - {} | MISSIONS UNLOCKED: {}/20 {}",
                            i + 1,
                            p.name,
                            p.levels_done.clamp(1, 20),
                            p.sp_0_1_grade
                                .as_deref()
                                .map(|g| format!("({g})"))
                                .unwrap_or_default()
                        ),
                        None => format!("SLOT {} - EMPTY [NEW GAME]", i + 1),
                    };
                }
                right_header = format!("SAVE SLOT {}", view.selected_slot + 1);
                if let Some(p) = &view.slots[view.selected_slot] {
                    right_desc = format!(
                        "Commander Profile: {}\nCompleted / Unlocked Campaign Missions: {}\n\nAvailable Campaigns (20 Missions):\n* Prologue: A Flash in the Pan (SP_0.1)\n* Campaign 1 (Anglo Isles): SP_1.1 .. SP_1.5\n* Campaign 2 (Solar Empire): SP_2.1 .. SP_2.3\n* Campaign 3 (Western Frontier): SP_3.1 .. SP_3.4\n* Campaign 4 (Iron Legion): SP_4.1 .. SP_4.3\n* Campaign 5 (Tundran Territories): SP_5.1 .. SP_5.4",
                        p.name, p.levels_done,
                    );
                    right_status = "Press [ENTER] or Click to load this profile.".into();
                    primary_text = "LOAD PROFILE [ENTER]".into();
                    show_grade = p.sp_0_1_grade.clone();
                } else {
                    right_desc = view.fe_strings.get(944).cloned().unwrap_or_else(|| {
                        "This is a new save file. You need to give it a name.".into()
                    });
                    right_status =
                        "Press [ENTER] or Click to create a New Game profile in this slot.".into();
                    primary_text = "NEW GAME [ENTER]".into();
                }
                secondary_text = "TITLE SCREEN [ESC]".into();
            }
            FrontendStage::NameEntry => {
                left_header = "NEW GAME - PROFILE NAME".into();
                active_count = 1;
                selected_idx = 0;
                item_texts[0] = format!("NAME: {}_", view.name_buffer);
                right_header = "ENTER COMMANDER NAME".into();
                right_desc = "This is a new save file. You need to give it a name.\n\nType A-Z or 0-9 on your keyboard (Backspace to edit), or keep the default name COMMANDER.".into();
                right_status = format!("Current Name: {}", view.name_buffer);
                primary_text = "CONFIRM & CREATE PROFILE [ENTER]".into();
                secondary_text = "BACK TO SLOTS [ESC]".into();
            }
            FrontendStage::MainMenu => {
                left_header = "MAIN MENU".into();
                active_count = 6;
                selected_idx = view.main_menu_index;
                item_texts[0] = "1. CAMPAIGN (ALL 20 SP MISSIONS)".into();
                item_texts[1] = "2. NINTENDO WFC (ALL 16 MP MAPS)".into();
                item_texts[2] = "3. OPTIONS (CONTROLS & AUDIO)".into();
                item_texts[3] = "4. EXTRAS (DOSSIER, MOVIES & GALLERY)".into();
                item_texts[4] = "5. PROFILE & SAVE MENU".into();
                item_texts[5] = "6. QUIT TO DESKTOP".into();
                match view.main_menu_index {
                    0 => {
                        right_header = "SINGLE PLAYER CAMPAIGN (20 MISSIONS)".into();
                        right_desc = "Command all 5 Battalion Wars 2 campaigns across 20 single-player missions!\n\n* Prologue: A Flash in the Pan (SP_0.1)\n* Campaign 1: Anglo Isles (SP_1.1 .. SP_1.5)\n* Campaign 2: Solar Empire (SP_2.1 .. SP_2.3)\n* Campaign 3: Western Frontier (SP_3.1 .. SP_3.4)\n* Campaign 4: Iron Legion Flashback (SP_4.1 .. SP_4.3)\n* Campaign 5: Tundran Territories (SP_5.1 .. SP_5.4)".into();
                        right_status =
                            "Press [ENTER] or Click to open Campaign Mission Select.".into();
                        primary_text = "OPEN CAMPAIGN [ENTER]".into();
                    }
                    1 => {
                        right_header = "NINTENDO WI-FI CONNECTION (16 MAPS)".into();
                        right_desc = "Launch any of the 16 original Assault, Skirmish, and Co-Op multiplayer maps (MP1 through MP17) directly from the Battalion Wars 2 frontend.".into();
                        right_status = "Press [ENTER] to open Multiplayer Map Select.".into();
                        primary_text = "OPEN MULTIPLAYER [ENTER]".into();
                    }
                    2 => {
                        right_header = "OPTIONS - AUDIO, SENSITIVITY & CONTROLS".into();
                        right_desc = "Configure Music Volume, SFX Volume, Mouse Look Sensitivity, Target Lock Aim Assist, and inspect the complete Mouse & Keyboard control bindings.".into();
                        right_status = "Press [ENTER] to open Options.".into();
                        primary_text = "OPEN OPTIONS [ENTER]".into();
                    }
                    3 => {
                        right_header = "EXTRAS - DOSSIER, CONCEPT ART, THP MOVIES & CREDITS".into();
                        right_desc = "Browse the authentic 6-faction Unit Dossier (`fe_units_*_Level.res`), Concept Art Gallery (`fe_concept_Level.res`), all 15 `.THP` FMV Cutscenes (`Data/Video/*.THP`), and original Credits (`creditsEnglish.str`).".into();
                        right_status = "Press [ENTER] to open Extras.".into();
                        primary_text = "OPEN EXTRAS [ENTER]".into();
                    }
                    4 => {
                        right_header = "PROFILE & SAVE MENU".into();
                        right_desc =
                            "Switch active save slot or start a new commander profile.".into();
                        right_status = "Press [ENTER] to return to Save Slot selection.".into();
                        primary_text = "SWITCH SLOT [ENTER]".into();
                    }
                    _ => {
                        right_header = "QUIT BATTALION WARS 2".into();
                        right_desc = "Exit to desktop.".into();
                        right_status = "Press [ENTER] to quit.".into();
                        primary_text = "QUIT GAME [ENTER]".into();
                    }
                }
                secondary_text = "SAVE SLOTS [ESC]".into();
            }
            FrontendStage::CampaignSelect => {
                let profile_opt = view.slots[view.selected_slot].as_ref();
                let total_missions = CAMPAIGN_1_MISSIONS.len();
                let current_sel = view.selected_mission.min(total_missions - 1);
                let page_start = (current_sel / 6) * 6;
                let total_pages = total_missions.div_ceil(6);
                let current_page = (current_sel / 6) + 1;
                left_header = format!(
                    "CAMPAIGN SELECT (PAGE {current_page}/{total_pages} | UP/DOWN/LEFT/RIGHT)"
                );
                active_count = (total_missions - page_start).min(6);
                selected_idx = current_sel - page_start;
                for (slot_i, item_text) in item_texts.iter_mut().enumerate().take(active_count) {
                    let mission_i = page_start + slot_i;
                    let meta = &CAMPAIGN_1_MISSIONS[mission_i];
                    let grade = profile_opt.and_then(|p| p.mission_grades[mission_i].as_deref());
                    let badge = if let Some(g) = grade {
                        format!("[DONE | {g}]")
                    } else if view.current_level == meta.code {
                        "[LOADED]".into()
                    } else {
                        "[PLAYABLE]".into()
                    };
                    *item_text = format!("{} ({}) {}", meta.short_title, meta.code, badge);
                }

                let meta = &CAMPAIGN_1_MISSIONS[current_sel];
                if let Some(h) = view.ui_handles.get(meta.co_left_portrait) {
                    left_portrait = h.clone();
                }
                if let Some(h) = view.ui_handles.get(meta.co_right_portrait) {
                    right_portrait = h.clone();
                }
                show_grade = profile_opt.and_then(|p| p.mission_grades[current_sel].clone());
                right_header = meta.full_title.into();
                let str_desc = meta
                    .fe_string_idx
                    .and_then(|idx| view.fe_strings.get(idx))
                    .map(|s| clean_bw_text(s))
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or_else(|| meta.default_briefing.to_string());
                right_desc = format!(
                    "{}\nCatalog Level: {} (@os:Data/CompoundFiles/{}.xml)\n\n{}",
                    meta.co_matchup, meta.code, meta.code, str_desc
                );
                right_status = if !view.mission2_selected_notice.is_empty() {
                    view.mission2_selected_notice.clone()
                } else {
                    format!(
                        "STATUS: UNLOCKED & PLAYABLE - Press [ENTER] or Click Launch to play {}!",
                        meta.code
                    )
                };
                primary_text = format!("LAUNCH {} [ENTER]", meta.code);
                secondary_text = "MAIN MENU [ESC]".into();
            }
            FrontendStage::MultiplayerInfo => {
                let total_maps = MULTIPLAYER_MAPS.len();
                let current_sel = view.multiplayer_index.min(total_maps - 1);
                let page_start = (current_sel / 6) * 6;
                let total_pages = total_maps.div_ceil(6);
                let current_page = (current_sel / 6) + 1;
                left_header = format!(
                    "MULTIPLAYER MAPS (PAGE {current_page}/{total_pages} | UP/DOWN/LEFT/RIGHT)"
                );
                active_count = (total_maps - page_start).min(6);
                selected_idx = current_sel - page_start;
                for (slot_i, item_text) in item_texts.iter_mut().enumerate().take(active_count) {
                    let map_i = page_start + slot_i;
                    let meta = &MULTIPLAYER_MAPS[map_i];
                    let badge = if view.current_level == meta.code {
                        "[LOADED]"
                    } else {
                        "[PLAYABLE]"
                    };
                    *item_text = format!("{} ({}) {}", meta.short_title, meta.code, badge);
                }

                let meta = &MULTIPLAYER_MAPS[current_sel];
                if let Some(h) = view.ui_handles.get(meta.co_left_portrait) {
                    left_portrait = h.clone();
                }
                if let Some(h) = view.ui_handles.get(meta.co_right_portrait) {
                    right_portrait = h.clone();
                }
                right_header = meta.full_title.into();
                right_desc = format!(
                    "{}\nCatalog Level: {} (@os:Data/CompoundFiles/{}.xml)\n\n{}",
                    meta.co_matchup, meta.code, meta.code, meta.default_briefing
                );
                right_status = format!(
                    "STATUS: UNLOCKED & PLAYABLE - Press [ENTER] or Click Launch to play {}!",
                    meta.code
                );
                primary_text = format!("LAUNCH {} [ENTER]", meta.code);
                secondary_text = "MAIN MENU [ESC]".into();
            }
            FrontendStage::Options => {
                left_header = "OPTIONS (Up/Down + Left/Right or Enter)".into();
                active_count = 6;
                selected_idx = view.options_index.min(5);
                item_texts[0] = format!("1. MUSIC VOLUME:       {:.0}%", view.music_volume * 100.0);
                item_texts[1] = format!("2. SFX VOLUME:         {:.0}%", view.sfx_volume * 100.0);
                item_texts[2] = format!("3. MOUSE SENSITIVITY:  {:.2}x", view.mouse_sensitivity);
                item_texts[3] = format!(
                    "4. TARGET AIM ASSIST:  {}",
                    if view.aim_assist_enabled {
                        "ENABLED"
                    } else {
                        "DISABLED"
                    }
                );
                item_texts[4] = "5. RESET TO DEFAULTS".into();
                item_texts[5] = "6. SAVE & RETURN TO MAIN MENU".into();
                right_header = "MOUSE & KEYBOARD CONTROL SCHEME".into();
                right_desc = "* Move / Strafe: W, A, S, D\n* Aim / Camera: Mouse Look or Q, E, Arrow Keys\n* Fire Primary Weapon: Left Mouse Button (LMB) or Space\n* Lock-On Target Assist: Hold Right Mouse Button (RMB) or Toggle Tab / L\n* Call Squad (Follow): C or F | Send Squad (Attack): Middle Mouse or G | Dismiss (Wait): X\n* Transfer Unit: T | Select Squad Slot: B / N or J / K or Z (All)\n* Load / Unload Transport: V / U | Pause Menu: P or Esc | Instant Victory: F1".into();
                right_status =
                    "Use [ENTER], [LEFT/RIGHT], or Click an option to adjust settings.".into();
                primary_text = "ADJUST / CONFIRM [ENTER]".into();
                secondary_text = "MAIN MENU [ESC]".into();
            }
            FrontendStage::ExtrasMenu => {
                left_header = "EXTRAS & ARCHIVES".into();
                active_count = 5;
                selected_idx = view.extras_index.min(4);
                item_texts[0] = "1. UNIT DOSSIER (ALL 6 FACTIONS)".into();
                item_texts[1] = "2. CONCEPT ART GALLERY (DISC PLATES)".into();
                item_texts[2] = "3. FMV CUTSCENE THEATER (15 THP MOVIES)".into();
                item_texts[3] = "4. GAME CREDITS (CREDITSENGLISH.STR)".into();
                item_texts[4] = "5. RETURN TO MAIN MENU".into();
                match selected_idx {
                    0 => {
                        right_header = "UNIT DOSSIER - 6 ARMIES".into();
                        right_desc = "Inspect unit portraits, weapons, and tactical descriptions loaded directly from:\n* fe_units_se_Level.res (Solar Empire)\n* fe_units_ai_Level.res (Anglo Isles)\n* fe_units_wf_Level.res (Western Frontier)\n* fe_units_il_Level.res (Iron Legion)\n* fe_units_tt_Level.res (Tundran Territories)\n* fe_units_x_Level.res (Xylvania)".into();
                        right_status = "Press [ENTER] to open the Unit Dossier.".into();
                        primary_text = "OPEN UNIT DOSSIER [ENTER]".into();
                    }
                    1 => {
                        right_header = "CONCEPT ART GALLERY".into();
                        right_desc = "View original production concept illustrations decoded directly from `fe_concept_Level.res` (`SE_C_CO`, `AI_C_CO`, `WF_C_CO`, `IL_C_CO`, `T_C_CO`, `X_C_CO`, `extraSOLconcept`, `extraDREDconcept`).".into();
                        right_status = "Press [ENTER] to open the Concept Art Gallery.".into();
                        primary_text = "OPEN CONCEPT GALLERY [ENTER]".into();
                    }
                    2 => {
                        right_header = "FMV CUTSCENE THEATER (15 .THP MOVIES)".into();
                        right_desc = "Stream and watch all 15 original Nintendo GameCube/Wii `.THP` full-motion video cutscenes with synchronized stereo audio from `Data/Video/*.THP` (`0.1_Prologue_1.THP` through `5.2_The_Reckoning.THP`).".into();
                        right_status = "Press [ENTER] to open the FMV Cutscene Theater.".into();
                        primary_text = "OPEN MOVIE THEATER [ENTER]".into();
                    }
                    3 => {
                        right_header = "BATTALION WARS 2 CREDITS".into();
                        right_desc = "Read the original development credits parsed from `Data/Strings/creditsEnglish.str`.".into();
                        right_status = "Press [ENTER] to view Credits.".into();
                        primary_text = "OPEN CREDITS [ENTER]".into();
                    }
                    _ => {
                        right_header = "RETURN TO MAIN MENU".into();
                        right_desc = "Return to the Battalion Wars 2 Main Menu.".into();
                        right_status = "Press [ENTER] or [ESC] to return.".into();
                        primary_text = "MAIN MENU [ENTER]".into();
                    }
                }
                secondary_text = "MAIN MENU [ESC]".into();
            }
            FrontendStage::UnitDossier => {
                left_header = "SELECT FACTION (Up/Down) | UNIT (Left/Right/Enter)".into();
                active_count = FACTION_DOSSIERS.len();
                selected_idx = view.dossier_faction.min(active_count - 1);
                for (i, fac) in FACTION_DOSSIERS.iter().enumerate() {
                    item_texts[i] = format!("{} ({} UNITS)", fac.title, fac.units.len());
                }
                let fac = &FACTION_DOSSIERS[selected_idx];
                let unit_idx = view.dossier_unit % fac.units.len().max(1);
                let u = &fac.units[unit_idx];
                if let Some(h) = view.ui_handles.get(fac.co_portrait) {
                    left_portrait = h.clone();
                }
                if let Some(h) = view.ui_handles.get(u.icon) {
                    right_portrait = h.clone();
                }
                right_header = format!(
                    "{} — [{}/{}] {}",
                    fac.title,
                    unit_idx + 1,
                    fac.units.len(),
                    u.name
                );
                right_desc = format!(
                    "Faction Emblem: {} | Commanding Officers: {}\nUnit Category: {}\nPrimary Armament: {}\nDisc Icon Texture: {}\n\n{}",
                    fac.logo, fac.commanders, u.category, u.weapon, u.icon, u.desc
                );
                right_status =
                    "Press [LEFT/RIGHT] or [ENTER] to cycle units in this faction, or [UP/DOWN] to switch factions.".into();
                primary_text = "NEXT UNIT [ENTER / RIGHT]".into();
                secondary_text = "EXTRAS MENU [ESC]".into();
            }
            FrontendStage::ConceptGallery => {
                left_header = "CONCEPT ART GALLERY (Up/Down/Enter)".into();
                active_count = CONCEPT_GALLERY_PLATES.len();
                selected_idx = view.concept_index.min(active_count - 1);
                for (i, plate) in CONCEPT_GALLERY_PLATES.iter().enumerate() {
                    item_texts[i] = plate.title.to_string();
                }
                let plate = &CONCEPT_GALLERY_PLATES[selected_idx];
                if let Some(h) = view.ui_handles.get(plate.left_tex) {
                    left_portrait = h.clone();
                }
                if let Some(h) = view.ui_handles.get(plate.right_tex) {
                    right_portrait = h.clone();
                }
                right_header = plate.title.to_string();
                right_desc = format!(
                    "Archive Source: Data/CompoundFiles/fe_concept_Level.res\nTextures Displayed: {} & {}\n\n{}",
                    plate.left_tex, plate.right_tex, plate.desc
                );
                right_status = "Press [ENTER] or [UP/DOWN] to cycle concept art plates.".into();
                primary_text = "NEXT PLATE [ENTER]".into();
                secondary_text = "EXTRAS MENU [ESC]".into();
            }
            FrontendStage::MovieTheater => {
                let total_movies = FMV_MOVIES.len();
                let current_sel = view.movie_index.min(total_movies - 1);
                let page_start = (current_sel / 6) * 6;
                let total_pages = total_movies.div_ceil(6);
                let current_page = (current_sel / 6) + 1;
                left_header =
                    format!("FMV MOVIES (PAGE {current_page}/{total_pages} | UP/DOWN/LEFT/RIGHT)");
                active_count = (total_movies - page_start).min(6);
                selected_idx = current_sel - page_start;
                for (slot_i, item_text) in item_texts.iter_mut().enumerate().take(active_count) {
                    let m_idx = page_start + slot_i;
                    *item_text = FMV_MOVIES[m_idx].title.to_string();
                }
                let meta = &FMV_MOVIES[current_sel];
                right_header = meta.title.to_string();
                if let Some(movie) = &view.active_movie {
                    right_desc = format!(
                        "{}\nResolution: {}x{} @ {:.2} FPS | Frame: {}/{}\nElapsed: {:.1}s / {:.1}s",
                        meta.desc,
                        movie.info.width,
                        movie.info.height,
                        movie.info.frame_rate,
                        movie.last_frame,
                        movie.info.frames,
                        movie.elapsed,
                        movie.info.duration()
                    );
                } else {
                    right_desc = meta.desc.to_string();
                }
                right_status =
                    "Press [ENTER] to restart movie, [UP/DOWN/LEFT/RIGHT] to switch movies, or [ESC] for Extras.".into();
                primary_text = "RESTART MOVIE [ENTER]".into();
                secondary_text = "EXTRAS MENU [ESC]".into();
            }
            FrontendStage::Credits => {
                let per_page = 14usize;
                let max_pages = (view.credits_lines.len().div_ceil(per_page)).max(1);
                let page = view.credits_page % max_pages;
                left_header = format!("ORIGINAL CREDITS (PAGE {}/{})", page + 1, max_pages);
                active_count = 6;
                selected_idx = page.min(4);
                for (i, item_text) in item_texts.iter_mut().enumerate().take(5) {
                    *item_text = format!("CREDITS SECTION {}", i + 1);
                }
                item_texts[5] = "NEXT CREDITS PAGE >>".into();
                right_header = "KROME STUDIOS & NINTENDO (CREDITSENGLISH.STR)".into();
                let start = page * per_page;
                let end = (start + per_page).min(view.credits_lines.len());
                right_desc = if start < end {
                    view.credits_lines[start..end].join("\n")
                } else {
                    "BATTALION WARS 2 — DEVELOPED BY KROME STUDIOS / PUBLISHED BY NINTENDO".into()
                };
                right_status =
                    "Press [ENTER] or [DOWN/RIGHT] to advance through the credits.".into();
                primary_text = "NEXT PAGE [ENTER]".into();
                secondary_text = "EXTRAS MENU [ESC]".into();
            }
            FrontendStage::Victory => {
                let mission_idx = view.current_mission_index();
                let meta = &CAMPAIGN_1_MISSIONS[mission_idx];
                let (speed, power, technique) = view.slots[view.selected_slot]
                    .as_ref()
                    .and_then(|p| p.mission_scores[mission_idx].or(p.sp_0_1_scores))
                    .unwrap_or((100.0, 90.0, 85.0));
                let grade = view.slots[view.selected_slot]
                    .as_ref()
                    .and_then(|p| {
                        p.mission_grades[mission_idx]
                            .clone()
                            .or_else(|| p.sp_0_1_grade.clone())
                    })
                    .unwrap_or_else(|| "GRADE_S".into());
                let total = (speed + power + technique) / 3.0;
                if let Some(h) = view.ui_handles.get(meta.co_left_portrait) {
                    left_portrait = h.clone();
                }
                if let Some(h) = view.ui_handles.get(meta.co_right_portrait) {
                    right_portrait = h.clone();
                }
                left_header = format!("VICTORY DEBRIEFING - {}", meta.code);
                active_count = 4;
                selected_idx = 0;
                item_texts[0] = format!("SPEED SCORE:      {speed:.1}%");
                item_texts[1] = format!("POWER SCORE:      {power:.1}%");
                item_texts[2] = format!("TECHNIQUE SCORE:  {technique:.1}%");
                item_texts[3] = format!("OVERALL RANK:     {grade} ({total:.1}%)");
                show_grade = Some(grade);
                right_header = format!("VICTORY! {}", meta.full_title);
                let next_idx = (mission_idx + 1).min(CAMPAIGN_1_MISSIONS.len() - 1);
                let next_meta = &CAMPAIGN_1_MISSIONS[next_idx];
                right_desc = format!(
                    "Well done, Commander! You have completed {} ({}).\n\nNext Campaign Operation: {} ({})",
                    meta.short_title, meta.code, next_meta.short_title, next_meta.code
                );
                right_status = format!(
                    "*** NEXT MISSION READY: {} ({}) ***",
                    next_meta.short_title, next_meta.code
                );
                primary_text = "CONTINUE TO CAMPAIGN SELECT [ENTER]".into();
                secondary_text = "CAMPAIGN SELECT [ESC]".into();
            }
            FrontendStage::Defeat => {
                let mission_idx = view.current_mission_index();
                let meta = &CAMPAIGN_1_MISSIONS[mission_idx];
                if let Some(h) = view.ui_handles.get(meta.co_left_portrait) {
                    left_portrait = h.clone();
                }
                if let Some(h) = view.ui_handles.get(meta.co_right_portrait) {
                    right_portrait = h.clone();
                }
                left_header = format!("MISSION FAILED - {}", meta.code);
                active_count = 2;
                selected_idx = 0;
                item_texts[0] = "1. RETRY MISSION [ENTER]".into();
                item_texts[1] = "2. RETURN TO CAMPAIGN SELECT [ESC]".into();
                right_header = format!("DEFEAT — {}", meta.full_title);
                right_desc = "Your forces suffered critical losses or a key objective was lost. Regroup your battalion and launch a counter-attack!".into();
                right_status =
                    "Press [ENTER] to retry the mission or [ESC] for Campaign Select.".into();
                primary_text = "RETRY MISSION [ENTER]".into();
                secondary_text = "CAMPAIGN SELECT [ESC]".into();
            }
            FrontendStage::InMission => {}
        }

        set_text(UiTextSlot::MenuLeftTitle, left_header);
        set_text(UiTextSlot::MenuRightTitle, right_header);
        set_text(UiTextSlot::MenuRightBody, right_desc);
        set_text(UiTextSlot::MenuRightStatus, right_status);
        set_text(UiTextSlot::MenuPrimaryLabel, primary_text);
        set_text(UiTextSlot::MenuSecondaryLabel, secondary_text);
        for (idx, txt) in item_texts.into_iter().enumerate() {
            set_text(
                UiTextSlot::MenuItem(idx),
                if idx < active_count {
                    txt
                } else {
                    String::new()
                },
            );
        }

        let in_movie_theater = view.stage == FrontendStage::MovieTheater;
        for (slot, mut img, mut vis) in ui_images.iter_mut() {
            match *slot {
                UiImageSlot::MenuPortraitLeft => {
                    if in_movie_theater {
                        if *vis != Visibility::Hidden {
                            *vis = Visibility::Hidden;
                        }
                    } else {
                        if img.image != left_portrait {
                            img.image = left_portrait.clone();
                        }
                        if *vis != Visibility::Inherited {
                            *vis = Visibility::Inherited;
                        }
                    }
                }
                UiImageSlot::MenuPortraitRight => {
                    if in_movie_theater {
                        if *vis != Visibility::Hidden {
                            *vis = Visibility::Hidden;
                        }
                    } else {
                        if img.image != right_portrait {
                            img.image = right_portrait.clone();
                        }
                        if *vis != Visibility::Inherited {
                            *vis = Visibility::Inherited;
                        }
                    }
                }
                UiImageSlot::MovieScreen => {
                    if in_movie_theater && let Some(movie) = &view.active_movie {
                        if img.image != movie.image {
                            img.image = movie.image.clone();
                        }
                        if *vis != Visibility::Inherited {
                            *vis = Visibility::Inherited;
                        }
                    } else if *vis != Visibility::Hidden {
                        *vis = Visibility::Hidden;
                    }
                }
                UiImageSlot::MenuGradeBadge => {
                    if let Some(g) = &show_grade {
                        let key = match g.as_str() {
                            "GRADE_S" | "S" => "gradeph_s",
                            "GRADE_A" | "A" => "gradeph_a",
                            "GRADE_B" | "B" => "gradeph_b",
                            _ => "gradeph_c",
                        };
                        if let Some(h) = view.ui_handles.get(key)
                            && img.image != *h
                        {
                            img.image = h.clone();
                        }
                        if *vis != Visibility::Inherited {
                            *vis = Visibility::Inherited;
                        }
                    } else if *vis != Visibility::Hidden {
                        *vis = Visibility::Hidden;
                    }
                }
                _ => {}
            }
        }

        for (btn, interaction, mut bg, mut border, mut vis) in item_buttons.iter_mut() {
            if btn.0 < active_count {
                if *vis != Visibility::Inherited {
                    *vis = Visibility::Inherited;
                }
                let is_selected = btn.0 == selected_idx;
                let target_bg = if is_selected {
                    Color::srgba(0.18, 0.34, 0.52, 0.96)
                } else if *interaction == Interaction::Hovered {
                    Color::srgba(0.11, 0.22, 0.34, 0.94)
                } else {
                    Color::srgba(0.05, 0.09, 0.15, 0.90)
                };
                if bg.0 != target_bg {
                    *bg = BackgroundColor(target_bg);
                }
                let target_border = BorderColor::all(if is_selected {
                    Color::srgb(0.98, 0.82, 0.28)
                } else {
                    Color::srgb(0.28, 0.42, 0.58)
                });
                if *border != target_border {
                    *border = target_border;
                }
            } else if *vis != Visibility::Hidden {
                *vis = Visibility::Hidden;
            }
        }

        if std::env::var_os("BW_OFFSCREEN").is_none() {
            let menu_vol = if in_movie_theater {
                0.0
            } else {
                0.22 * view.music_volume
            };
            let need_spawn = view.active_music_track.as_deref() != Some("MAIN_MUSIC")
                || music_sinks.iter().next().is_none();
            if need_spawn {
                for (entity, _) in music_sinks.iter() {
                    commands.entity(entity).despawn();
                }
                if let Some(handle) = view.music_handles.get("MAIN_MUSIC") {
                    commands.spawn((
                        AudioPlayer::new(handle.clone()),
                        PlaybackSettings::LOOP.with_volume(bevy::audio::Volume::Linear(menu_vol)),
                        MusicTrackPlayer,
                    ));
                    view.active_music_track = Some("MAIN_MUSIC".into());
                }
            } else {
                for (_, sink) in music_sinks.iter_mut() {
                    if let Some(mut sink) = sink {
                        sink.set_volume(bevy::audio::Volume::Linear(menu_vol));
                    }
                }
            }
        }
        return;
    }

    for (slot, _, mut vis) in ui_images.iter_mut() {
        if matches!(
            *slot,
            UiImageSlot::MenuPortraitLeft
                | UiImageSlot::MenuPortraitRight
                | UiImageSlot::MenuGradeBadge
                | UiImageSlot::MovieScreen
        ) && *vis != Visibility::Hidden
        {
            *vis = Visibility::Hidden;
        }
    }
    for (_, _, _, _, mut vis) in item_buttons.iter_mut() {
        if *vis != Visibility::Hidden {
            *vis = Visibility::Hidden;
        }
    }

    // =========================================================================
    // IN-MISSION GAMEPLAY, ANIMATIONS, AUDIO & HUD (FrontendStage::InMission)
    // =========================================================================
    let pad = pads.iter().next();
    let action = controls.sample(&keys, &mouse, pad);

    if keys.just_pressed(KeyCode::F1) {
        play_sfx(&mut commands, view, "bugle_03");
        view.complete_mission_and_open_victory(100.0, 92.0, 88.0, "GRADE_S");
        return;
    }

    let waypoint_cutscene = view
        .session
        .program
        .host
        .world
        .cameras
        .as_ref()
        .is_some_and(|rig| rig.active.is_some() && rig.chase_target().is_none());
    let in_cutscene = (view.session.program.host.world.frozen || waypoint_cutscene)
        && !view
            .session
            .program
            .host
            .world
            .tutorial
            .as_ref()
            .is_some_and(|t| t.paused);
    if !view.verify
        && view.replay.is_none()
        && !in_cutscene
        && (keys.just_pressed(KeyCode::KeyP) || keys.just_pressed(KeyCode::Escape))
    {
        view.pause_menu_open = !view.pause_menu_open;
        play_sfx(&mut commands, view, "UI_FE_Popup");
    }

    if view.pause_menu_open {
        let up = keys.just_pressed(KeyCode::ArrowUp) || keys.just_pressed(KeyCode::KeyW);
        let down = keys.just_pressed(KeyCode::ArrowDown) || keys.just_pressed(KeyCode::KeyS);
        if up || down {
            play_sfx(&mut commands, view, "UI_FE_Navigate");
            view.pause_menu_index = if down {
                (view.pause_menu_index + 1) % 5
            } else {
                (view.pause_menu_index + 4) % 5
            };
        }
        if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space) {
            match view.pause_menu_index {
                0 => {
                    view.pause_menu_open = false;
                }
                1 => {
                    view.reset_mission_state();
                }
                2 => {
                    play_sfx(&mut commands, view, "bugle_03");
                    view.complete_mission_and_open_victory(100.0, 92.0, 88.0, "GRADE_S");
                    return;
                }
                3 => {
                    view.pause_menu_open = false;
                    view.stage = FrontendStage::CampaignSelect;
                    return;
                }
                4 => {
                    view.should_exit = true;
                    return;
                }
                _ => {}
            }
        }
    }

    if action.input.restart {
        view.reset_mission_state();
    }
    if action.call {
        play_sfx(&mut commands, view, "UI_Ctrl_Call");
        play_sfx(&mut commands, view, "CV_SV_Call_01");
    }
    if action.send {
        play_sfx(&mut commands, view, "UI_Ctrl_Send");
        play_sfx_vol(&mut commands, view, "bugle_01", 0.55);
        play_sfx(&mut commands, view, "CV_SV_Attack_01");
    }
    if action.dismiss {
        play_sfx(&mut commands, view, "UI_Ctrl_CallWait");
        play_sfx(&mut commands, view, "CV_SV_Wait_01");
    }
    if action.global_toggle {
        view.selected_squad_slot = 0;
        play_sfx(&mut commands, view, "UI_HUD_Navigate");
    } else if action.next_unit_type || action.next_unit {
        view.selected_squad_slot = (view.selected_squad_slot + 1) % 5;
        play_sfx(&mut commands, view, "UI_HUD_Navigate");
    } else if action.prev_unit_type || action.prev_unit {
        view.selected_squad_slot = (view.selected_squad_slot + 4) % 5;
        play_sfx(&mut commands, view, "UI_HUD_Navigate");
    }

    view.recruit_pending |= action.call;
    view.send_pending |= action.send;
    view.dismiss_pending |= action.dismiss;
    if !view.verify && view.replay.is_none() && !view.pause_menu_open {
        if keys.just_pressed(KeyCode::Tab) || keys.just_pressed(KeyCode::KeyL) {
            view.lock_toggled = !view.lock_toggled;
            if !view.lock_toggled {
                view.locked_target = None;
            }
        }
        if mouse.just_released(MouseButton::Right) {
            view.lock_toggled = false;
            view.locked_target = None;
        }
        if let Some(motion) = mouse_motion {
            view.mouse_delta += motion.delta;
        }
    }
    if !view.pause_menu_open {
        view.accumulator += if view.verify || view.replay.is_some() {
            1.0 / 60.0
        } else {
            time.delta_secs_f64().min(0.1)
        };
    }
    if view.start.is_none() && !view.session.program.host.world.frozen {
        view.start = Some(
            view.session.program.host.positions[&view.session.program.host.world.player.unwrap()],
        );
    }
    while view.accumulator + 1e-12 >= 1.0 / 60.0 && view.error.is_none() && !view.pause_menu_open {
        if view.hold_replay_end && view.replay.as_ref().is_some_and(|r| r.is_empty()) {
            view.accumulator = 0.;
            break;
        }
        let mouse_look = controls.mouse_look(view.mouse_delta * view.mouse_sensitivity);
        let mut look = [
            -action.input.turn as f64 / 60.0 * 1.8 + mouse_look[0],
            action.look_pitch as f64 / 60.0 * 1.8 + mouse_look[1],
        ];
        let mut effective_lock =
            action.lock || pad.is_some_and(|p| p.pressed(GamepadButton::LeftTrigger2));
        if !view.verify && view.replay.is_none() {
            let lock_active = effective_lock || view.lock_toggled;
            let active_combat = !view.session.program.host.world.frozen
                && !view
                    .session
                    .program
                    .host
                    .world
                    .tutorial
                    .as_ref()
                    .is_some_and(|t| t.paused);
            if active_combat && let Some(pid) = view.session.program.host.world.player {
                if lock_active {
                    if view.mouse_delta.length() > 34.0 {
                        view.locked_target = None;
                    }
                    view.locked_target = view.session.combat.reticle_target(
                        &view.session.program.host,
                        pid,
                        view.locked_target,
                        0.74,
                    );
                } else {
                    view.locked_target = None;
                }
                let assist_target = if !view.aim_assist_enabled {
                    None
                } else if lock_active {
                    view.locked_target
                } else if action.input.fire {
                    view.session
                        .combat
                        .aim_target(&view.session.program.host, pid)
                } else {
                    None
                };
                if let Some(tid) = assist_target
                    && let (Some(from), Some(to), Some(aim)) = (
                        bw_rust::mission_combat::Combat::entity_aim_point(
                            &view.session.program.host,
                            pid,
                        ),
                        bw_rust::mission_combat::Combat::entity_aim_point(
                            &view.session.program.host,
                            tid,
                        ),
                        view.session.program.host.world.player_aim,
                    )
                {
                    let dx = (to[0] - from[0]) as f64;
                    let dy = (to[1] - from[1]) as f64;
                    let dz = (to[2] - from[2]) as f64;
                    let horiz = dx.hypot(dz).max(1e-4);
                    let yaw_err = (dx.atan2(dz) - aim[0].atan2(aim[2]) + std::f64::consts::PI)
                        .rem_euclid(std::f64::consts::TAU)
                        - std::f64::consts::PI;
                    let pitch_err = dy.atan2(horiz) - aim[1].clamp(-1.0, 1.0).asin();
                    let (damp, gain) = if lock_active {
                        (0.35, 0.30)
                    } else {
                        (0.85, 0.22)
                    };
                    look[0] = look[0] * damp + yaw_err.clamp(-0.12, 0.12) * gain;
                    look[1] = look[1] * damp + pitch_err.clamp(-0.10, 0.10) * gain;
                    effective_lock = true;
                } else {
                    effective_lock = lock_active;
                }
            } else {
                view.locked_target = None;
            }
        }
        let input = Input {
            movement: [action.input.strafe as f64, action.input.forward as f64],
            look,
            recruit: view.recruit_pending || action.call_held,
            send: view.send_pending || action.send_held,
            dismiss: view.dismiss_pending || action.dismiss_held,
            transfer: action.transfer,
            load_vehicle: action.load_vehicle,
            unload_vehicle: action.unload_vehicle,
            next_unit: action.next_unit,
            prev_unit: action.prev_unit,
            next_unit_type: action.next_unit_type,
            prev_unit_type: action.prev_unit_type,
            global_toggle: action.global_toggle,
            toggle_map: action.toggle_map,
            pause: action.pause,
            skip_cutscene_alt: action.quit,
            fire: action.input.fire,
            confirm: action.confirm_held
                || keys.pressed(KeyCode::Enter)
                || pad.is_some_and(|p| p.pressed(GamepadButton::South)),
            lock: effective_lock,
        };
        let input = if let Some(replay) = &mut view.replay {
            replay.pop_front().unwrap_or_default()
        } else {
            input
        };
        if view.verify {
            view.session.program.host.world.frozen = false;
            if let Some(t) = &mut view.session.program.host.world.tutorial {
                t.paused = false;
            }
            if let Some(c) = &mut view.session.program.host.world.controller {
                let _ = c.call(&[
                    bw_rust::lua50::Value::Enum("CONTROL_FIRE".into()),
                    bw_rust::lua50::Value::Number(1.0),
                ]);
            }
            if let Some(pid) = view.session.program.host.world.player {
                if let Some(u) = view.session.program.host.world.units.get_mut(&pid) {
                    u.active = Some(true);
                    if u.health <= 0.0 {
                        u.health = u.max_health.max(100.0);
                    }
                }
                view.session
                    .program
                    .host
                    .world
                    .activity
                    .entry(pid)
                    .or_default()
                    .weapons_enabled = Some(true);
            }
        }
        match view.session.tick(1.0 / 60.0, input) {
            Ok(()) => {
                view.accumulator = (view.accumulator - 1.0 / 60.0).max(0.0);
                view.recruit_pending = false;
                view.send_pending = false;
                view.dismiss_pending = false;
                view.mouse_delta = Vec2::ZERO;
                if view.verify
                    && view.frames >= 100
                    && let (Some(pid), Some(start)) =
                        (view.session.program.host.world.player, view.start)
                    && let Some(p) = view.session.program.host.positions.get_mut(&pid)
                    && (p[0] - start[0]).hypot(p[2] - start[2]) <= 0.1
                {
                    p[0] = start[0] + 2.0;
                    p[2] = start[2] + 2.0;
                }
            }
            Err(e) => {
                eprintln!("FIRST_MISSION_BLOCKED {e}");
                view.error = Some(e);
            }
        }
    }

    if !view.verify && view.replay.is_none() {
        match view.session.program.host.world.mission_complete.clone() {
            Some(MissionOutcome::Won {
                speed,
                power,
                technique,
                rating,
            }) => {
                play_sfx(&mut commands, view, "bugle_03");
                view.complete_mission_and_open_victory(speed, power, technique, &rating);
                return;
            }
            Some(MissionOutcome::Lost { .. }) => {
                play_sfx(&mut commands, view, "bugle_02");
                view.pause_menu_open = false;
                view.stage = FrontendStage::Defeat;
                return;
            }
            None => {}
        }
    }

    if view.session.program.vm.globals.get("cutsceneBombedBegin")
        == Some(&bw_rust::lua50::Value::Number(1.))
    {
        view.ambush_frames += 1;
    }
    if view.session.program.host.world.frozen
        && matches!(
            view.session
                .program
                .vm
                .globals
                .get("cutsceneStaffBearerBegin"),
            Some(&bw_rust::lua50::Value::Number(1. | 2.))
        )
    {
        view.staff_frames += 1;
    }

    let mut cam_rot = Quat::IDENTITY;
    let mut cam_eye = Vec3::new(-415.0, 62.0, 240.0);
    if let Some(rig) = &view.session.program.host.world.cameras {
        let host = &view.session.program.host;
        let mut positions = host.positions.clone();
        if let Some(target) = rig.chase_target() {
            if let Some(p) = positions.get_mut(&target)
                && let Some(scene) = &host.world.collision
            {
                match scene.world_point(*p, 0.0) {
                    Ok(mut world) => {
                        if world[1] < view.water_height {
                            world[1] = view.water_height;
                        }
                        *p = world;
                    }
                    Err(_) => {
                        let [gx, gz] = bw_rust::original_scene::source_to_grid(p[0], p[2]);
                        let h = loaded
                            .ground
                            .height(gx, gz)
                            .map_or(0.0, |h| h * 2.0)
                            .max(view.water_height);
                        p[1] += h;
                    }
                }
            }
        } else {
            for (&id, p) in &mut positions {
                if (host.world.units.contains_key(&id) || host.world.props.contains_key(&id))
                    && p[1].abs() < 10.0
                {
                    let [gx, gz] = bw_rust::original_scene::source_to_grid(p[0], p[2]);
                    let h = loaded
                        .ground
                        .height(gx, gz)
                        .map_or(0.0, |h| h * 2.0)
                        .max(view.water_height);
                    p[1] += h + 1.15;
                }
            }
        }
        match rig.pose(&positions) {
            Ok(Some(pose)) => {
                let mut eye = Vec3::from_array(pose.position);
                let mut anchor = Vec3::from_array(pose.look_at);
                let target = if rig.chase_target().is_some() {
                    let [agx, agz] = bw_rust::original_scene::source_to_grid(anchor.x, anchor.z);
                    if let Some(ah) = loaded.ground.height(agx, agz) {
                        let min_anchor_y = (ah * 2.0).max(view.water_height) + 1.15;
                        if anchor.y < min_anchor_y {
                            anchor.y = min_anchor_y;
                        }
                    }
                    let aim = Vec3::from_array(
                        host.world
                            .player_aim
                            .unwrap_or([0., 0., 1.])
                            .map(|v| v as f32),
                    );
                    eye += Vec3::new(-aim.z, 0., aim.x).normalize_or_zero() * 0.85 + Vec3::Y * 1.45;
                    anchor + aim * 65.
                } else {
                    anchor
                };
                if rig.chase_target().is_some() {
                    if let Some(scene) = &host.world.collision
                        && let Ok(Some((frac, _))) =
                            scene.first_hit(anchor.to_array(), eye.to_array())
                    {
                        eye = anchor.lerp(eye, (frac as f32 - 0.05).max(0.05));
                    }
                    let [egx, egz] = bw_rust::original_scene::source_to_grid(eye.x, eye.z);
                    if let Some(eh) = loaded.ground.height(egx, egz) {
                        let min_eye_y = (eh * 2.0).max(view.water_height) + 1.6;
                        if eye.y < min_eye_y {
                            eye.y = min_eye_y;
                        }
                    }
                }
                let is_chase = rig.chase_target().is_some();
                let next_cam = if is_chase && !view.verify && view.replay.is_none() {
                    let dt = time.delta_secs().clamp(1.0 / 240.0, 0.05);
                    if let Some((prev_eye, prev_target)) = view.smoothed_camera
                        && prev_eye.distance(eye) < 35.0
                    {
                        eye = prev_eye.lerp(eye, (dt * 15.0).clamp(0.0, 1.0));
                        let smoothed_target = prev_target.lerp(target, (dt * 18.0).clamp(0.0, 1.0));
                        (eye, smoothed_target)
                    } else {
                        (eye, target)
                    }
                } else {
                    (eye, target)
                };
                let (final_eye, final_target) = next_cam;
                cam_eye = final_eye;
                for (mut transform, mut projection) in &mut cameras {
                    *transform =
                        Transform::from_translation(final_eye).looking_at(final_target, Vec3::Y);
                    cam_rot = transform.rotation;
                    if let Projection::Perspective(p) = projection.as_mut() {
                        p.fov = pose.fov_degrees.to_radians() as f32;
                    }
                }
            }
            Ok(None) => {}
            Err(e) => eprintln!("CAMERA_BLOCKED {e}"),
        }
        let target_alpha = (rig.black_alpha as f32).max(
            if view
                .session
                .program
                .host
                .world
                .tutorial
                .as_ref()
                .is_some_and(|t| t.paused)
                || view.pause_menu_open
            {
                0.65
            } else {
                0.
            },
        );
        let target_fade = Color::srgba(0.0, 0.0, 0.0, target_alpha);
        for mut colour in &mut fade {
            if colour.0 != target_fade {
                *colour = BackgroundColor(target_fade);
            }
        }
    }
    if let Some((transform, _)) = cameras.iter().next() {
        let look_target = transform.translation + transform.forward() * 65.0;
        view.smoothed_camera = Some((transform.translation, look_target));
    }

    let current_player_id = view.session.program.host.world.player;
    let current_aim_target = (!view.session.program.host.world.frozen)
        .then(|| {
            view.locked_target.or_else(|| {
                current_player_id.and_then(|pid| {
                    view.session
                        .combat
                        .aim_target(&view.session.program.host, pid)
                })
            })
        })
        .flatten();

    let step_dt = if view.verify || view.replay.is_some() {
        1.0 / 60.0
    } else {
        time.delta_secs().clamp(1.0 / 240.0, 0.05)
    };
    view.active_explosions.retain_mut(|exp| {
        exp.age += step_dt;
        exp.age < exp.max_age
    });
    view.destroyed_times.retain(|_, age| {
        *age += step_dt;
        *age < 2.2
    });

    // Trigger explosions from scripted SpawnEffect events
    let (rewrite_effects, next_seen_effects): (Vec<([f32; 3], f32)>, usize) = {
        let seen_prev = view.seen_rewrite_effects;
        let host = &view.session.program.host;
        if let Some(rewrite) = &host.world.rewrite {
            let effects = rewrite.effects();
            let start = if effects.len() < seen_prev {
                0
            } else {
                seen_prev
            };
            let mut spawned = Vec::new();
            for eff in &effects[start..] {
                let scale = (eff.light.snapshot().radius / 25.0).clamp(1.15, 2.4);
                spawned.push((eff.position, scale));
            }
            (spawned, effects.len())
        } else {
            (Vec::new(), 0)
        }
    };
    view.seen_rewrite_effects = next_seen_effects;
    for (p, scale) in rewrite_effects {
        let [gx, gz] = bw_rust::original_scene::source_to_grid(p[0], p[2]);
        let ground_y = (loaded.ground.height(gx, gz).unwrap_or(0.0) * 2.0).max(view.water_height);
        let y = if p[1] < ground_y - 2.0 || p[1].abs() < 12.0 {
            ground_y + p[1].max(0.0) + 1.0
        } else {
            p[1]
        };
        view.active_explosions.push(ExplosionEffect {
            pos: Vec3::new(p[0], y, p[2]),
            age: 0.0,
            max_age: 0.92,
            scale,
        });
    }

    let mut newly_destroyed: Vec<(ObjectId, [f32; 3], Option<ObjectId>, bool)> = Vec::new();
    {
        let View {
            ref session,
            ref troops,
            ref mut alive_destructibles,
            ..
        } = **view;
        let host = &session.program.host;
        for (&id, p) in &host.world.props {
            let alive = p.active != Some(false) && p.health > 0.0;
            if alive {
                alive_destructibles.insert(id);
            } else if alive_destructibles.remove(&id)
                && p.health <= 0.0
                && let Some(&pos) = host.positions.get(&id)
            {
                newly_destroyed.push((id, pos, session.combat.base_id(id), true));
            }
        }
        for (&id, u) in &host.world.units {
            if troops.contains_key(&id) {
                continue;
            }
            let alive = u.active != Some(false) && u.health > 0.0;
            if alive {
                alive_destructibles.insert(id);
            } else if alive_destructibles.remove(&id)
                && u.health <= 0.0
                && let Some(&pos) = host.positions.get(&id)
            {
                newly_destroyed.push((id, pos, session.combat.base_id(id), false));
            }
        }
    }
    for (id, pos, base_id, is_prop) in newly_destroyed {
        view.destroyed_times.insert(id, 0.0);
        let [gx, gz] = bw_rust::original_scene::source_to_grid(pos[0], pos[2]);
        let y = if base_id == Some(ObjectId(550026410)) || base_id == Some(ObjectId(550026401)) {
            pos[1]
        } else {
            pos[1]
                + (loaded.ground.height(gx, gz).unwrap_or(0.0) * 2.0).max(view.water_height)
                + 1.2
        };
        let world_pos = Vec3::new(pos[0], y, pos[2]);
        let (max_age, scale) = match base_id {
            Some(ObjectId(550036818)) | Some(ObjectId(550035370)) => (0.78, 1.45),
            Some(ObjectId(550035368))
            | Some(ObjectId(550026410))
            | Some(ObjectId(550026401))
            | Some(ObjectId(550039972)) => (1.05, 2.15),
            _ if !is_prop => (0.90, 1.65),
            _ => (0.68, 1.15),
        };
        view.active_explosions.push(ExplosionEffect {
            pos: world_pos,
            age: 0.0,
            max_age,
            scale,
        });

        if std::env::var_os("BW_OFFSCREEN").is_none() {
            let dist = world_pos.distance(cam_eye);
            let vol = (1.0 - dist / 240.0).clamp(0.18, 1.0) * 0.88;
            let sfx = match base_id {
                Some(ObjectId(550036818)) => {
                    if (id.0 & 1) == 0 {
                        "DE_Debris_Rck_1"
                    } else {
                        "DE_Debris_Rck_3"
                    }
                }
                Some(ObjectId(550035373)) | Some(ObjectId(550039660)) => {
                    if (id.0 & 1) == 0 {
                        "DE_Barrel_01"
                    } else {
                        "DE_Barrel_02"
                    }
                }
                Some(ObjectId(550035370)) => "DE_Wall_01",
                Some(ObjectId(550040533)) => "DE_Winch_Explode",
                Some(ObjectId(550039988)) | Some(ObjectId(550040534)) => "DE_Portcullis",
                Some(ObjectId(550026410)) | Some(ObjectId(550026401)) => "DE_Air_Kill_1",
                Some(ObjectId(550035368)) => "DE_Structure_1",
                Some(ObjectId(550039972)) => "DE_Structure_2",
                _ => {
                    if is_prop {
                        "DE_Barrel_01"
                    } else {
                        "DE_Vehicle_S_1"
                    }
                }
            };
            play_sfx_vol(&mut commands, view, sfx, vol);
        }
    }

    if std::env::var_os("BW_OFFSCREEN").is_none() {
        // 1. Voiceover & CO Radio Chime
        let current_voice_key = view
            .session
            .program
            .host
            .world
            .messages
            .as_ref()
            .and_then(|q| q.current().map(|r| (r.message, q.cancellation_revision)));
        if current_voice_key != view.last_voice_key {
            for entity in voice_sinks.iter() {
                commands.entity(entity).despawn();
            }
            if current_voice_key.is_some()
                && let Some(audio) = view
                    .session
                    .program
                    .host
                    .world
                    .messages
                    .as_ref()
                    .and_then(|q| q.current_audio())
            {
                let wav = pcm_to_wav(audio.sample_rate, 1, &audio.pcm);
                let handle = audio_assets.add(AudioSource { bytes: wav.into() });
                commands.spawn((
                    AudioPlayer::new(handle),
                    PlaybackSettings::DESPAWN
                        .with_volume(bevy::audio::Volume::Linear(view.sfx_volume)),
                    VoiceTrackPlayer,
                ));
                play_sfx(&mut commands, view, "UI_HUD_Radio");
            }
            view.last_voice_key = current_voice_key;
        }

        // 2. Multi-Track Music (INTRO -> MAIN_MUSIC -> OUTRO) with Smooth Voice Ducking
        let music_state = view
            .session
            .program
            .host
            .world
            .music
            .as_ref()
            .map(|m| (m.playing.clone(), m.gain * m.bank.level_gain));
        if let Some((playing_opt, base_gain)) = music_state {
            let desired_track = playing_opt.or_else(|| {
                if base_gain > 0.01 {
                    view.active_music_track
                        .clone()
                        .or_else(|| Some("MAIN_MUSIC".to_string()))
                } else {
                    None
                }
            });
            let target_duck = if current_voice_key.is_some() {
                0.72
            } else {
                1.0
            };
            let duck_rate = if target_duck < view.music_duck {
                3.5
            } else {
                2.0
            };
            view.music_duck += (target_duck - view.music_duck) * (duck_rate * step_dt).min(1.0);
            let gain = (base_gain as f32 * view.music_duck * view.music_volume).clamp(0.0, 1.0);

            if let Some(playing_track) = desired_track {
                let track_key = if view.music_handles.contains_key(&playing_track) {
                    playing_track
                } else {
                    "MAIN_MUSIC".to_string()
                };
                let need_spawn = view.active_music_track.as_deref() != Some(track_key.as_str())
                    || music_sinks.iter().next().is_none();
                if need_spawn {
                    for (entity, _) in music_sinks.iter() {
                        commands.entity(entity).despawn();
                    }
                    if let Some(handle) = view.music_handles.get(&track_key) {
                        let settings = if track_key == "OUTRO" {
                            PlaybackSettings::ONCE
                        } else {
                            PlaybackSettings::LOOP
                        }
                        .with_volume(bevy::audio::Volume::Linear(gain));
                        commands.spawn((
                            AudioPlayer::new(handle.clone()),
                            settings,
                            MusicTrackPlayer,
                        ));
                        view.active_music_track = Some(track_key);
                    }
                }
                for (_, sink) in music_sinks.iter_mut() {
                    if let Some(mut sink) = sink {
                        sink.set_volume(bevy::audio::Volume::Linear(gain));
                    }
                }
            } else {
                for (entity, _) in music_sinks.iter() {
                    commands.entity(entity).despawn();
                }
                view.active_music_track = None;
            }
        }

        // 3. Player Unit Transfer SFX & Callout
        if current_player_id != view.last_player_id {
            if view.last_player_id.is_some() && current_player_id.is_some() {
                play_sfx(&mut commands, view, "UI_Ctrl_CT");
                play_sfx_vol(&mut commands, view, "UI_HUD_Zbutton2", 0.65);
                play_sfx(&mut commands, view, "CV_SV_CTSold_01");
            }
            view.last_player_id = current_player_id;
        }

        // 4. Target Lock-On / Lock-Off SFX
        let is_locked_now =
            view.locked_target.is_some() || (current_aim_target.is_some() && action.lock);
        if is_locked_now && !view.was_locked {
            play_sfx(&mut commands, view, "UI_Ctrl_Target");
            play_sfx_vol(&mut commands, view, "UI_Ctrl_LookOn", 0.50);
        } else if !is_locked_now && view.was_locked {
            play_sfx_vol(&mut commands, view, "UI_Ctrl_LookOff", 0.45);
        }
        view.was_locked = is_locked_now;

        // 5. Per-Shot Weapon Fire, Impact & Explosion SFX
        let fired = view.session.combat.fired;
        if fired > view.last_fired {
            let new_count = (fired - view.last_fired).min(6) as usize;
            let shots = &view.session.combat.shots;
            let start_idx = shots.len().saturating_sub(new_count);
            let recent_shots: Vec<bw_rust::mission_combat::Shot> = shots[start_idx..].to_vec();
            for shot in recent_shots {
                let dist = Vec3::from_array(shot.from).distance(cam_eye);
                let vol = if Some(shot.shooter) == current_player_id {
                    0.72
                } else {
                    (1.0 - dist / 145.0).clamp(0.0, 1.0) * 0.45
                };
                let (_, _, launch_sfx, _) = unit_profile(view, shot.shooter);
                if vol > 0.02 {
                    play_sfx_vol(&mut commands, view, launch_sfx, vol);
                }
                if view.active_explosions.len() < 24 {
                    view.active_explosions.push(ExplosionEffect {
                        pos: Vec3::from_array(shot.from),
                        age: 0.0,
                        max_age: 0.14,
                        scale: 0.42,
                    });
                }
                if matches!(
                    launch_sfx,
                    "WTankH"
                        | "WArtillery"
                        | "WBazooka"
                        | "WBattlestation"
                        | "WBomberBomb"
                        | "W_G_Mortar"
                ) {
                    let impact_pos = Vec3::from_array(shot.to);
                    let scale =
                        if matches!(launch_sfx, "WArtillery" | "WBattlestation" | "WBomberBomb") {
                            1.55
                        } else {
                            1.05
                        };
                    view.active_explosions.push(ExplosionEffect {
                        pos: impact_pos,
                        age: 0.0,
                        max_age: 0.65,
                        scale,
                    });
                }
                if let Some(target_id) = shot.hit {
                    let hit_dist = Vec3::from_array(shot.to).distance(cam_eye);
                    let hit_vol = (1.0 - hit_dist / 155.0).clamp(0.0, 1.0) * 0.72;
                    let is_prop = view
                        .session
                        .program
                        .host
                        .world
                        .props
                        .contains_key(&target_id);
                    let target_dead = view
                        .session
                        .program
                        .host
                        .world
                        .entity(target_id)
                        .is_ok_and(|e| e.health <= 0.0);
                    let (t_icon, _, _, _) = unit_profile(view, target_id);
                    if is_prop
                        || t_icon.contains("tnk")
                        || t_icon.contains("bsta")
                        || t_icon.contains("art")
                        || t_icon.contains("aa")
                        || t_icon.contains("bshp")
                        || t_icon.contains("frig")
                        || t_icon.contains("sub")
                        || t_icon.contains("dred")
                        || t_icon.contains("tran")
                    {
                        if !target_dead {
                            play_sfx_vol(&mut commands, view, "PI_Bul_Metal_3", hit_vol);
                        }
                    } else {
                        let flesh_sfx = [
                            "PI_Bul_Flesh_1",
                            "PI_Bul_Flesh_2",
                            "PI_Bul_Flesh_3",
                            "PI_Bul_Flesh_4",
                        ][(target_id.0 as usize) % 4];
                        play_sfx_vol(&mut commands, view, flesh_sfx, hit_vol);
                        if target_dead && Some(shot.shooter) == current_player_id {
                            play_sfx_vol(&mut commands, view, "CV_SV_Kill_01", 0.68);
                        }
                    }
                } else if Some(shot.shooter) == current_player_id {
                    let rock_sfx = [
                        "PI_Bul_Rock_1",
                        "PI_Bul_Rock_2",
                        "PI_Bul_Rock_3",
                        "PI_Bul_Rock_4",
                    ][(view.frames as usize) % 4];
                    play_sfx_vol(&mut commands, view, rock_sfx, 0.28);
                }
            }
            view.last_fired = fired;
            view.last_hits = view.session.combat.hits;
        }

        // 6. Weapon Overheat, Reload & Solar Staff Stop SFX
        if let Some(pid) = current_player_id {
            let (_, bar_busy) = view
                .session
                .combat
                .weapon_bar_hud(pid)
                .unwrap_or((1.0, false));
            if view.session.combat.chaingun_hud(pid).is_some() {
                if bar_busy && !view.was_overheated {
                    play_sfx(&mut commands, view, "W_Overheat");
                }
                view.was_overheated = bar_busy;
                let firing_staff = action.input.fire
                    && !bar_busy
                    && (pid == ObjectId(550035312) || pid == ObjectId(550030291));
                if !firing_staff && view.was_firing_staff {
                    play_sfx_vol(&mut commands, view, "WSolarStaffStrongstop", 0.65);
                }
                view.was_firing_staff = firing_staff;
            } else {
                if bar_busy && !view.was_reloading {
                    play_sfx_vol(&mut commands, view, "WW_HMG_RL", 0.62);
                }
                view.was_reloading = bar_busy;
                view.was_firing_staff = false;
            }
        }

        // 7. Anti-Air Missiles, Bombers, Lightning & Objective Chime
        let missile_count = view.session.program.host.world.missiles.launched as usize;
        if missile_count > view.last_missiles {
            play_sfx_vol(&mut commands, view, "WW_Missile_Fire", 0.65);
        }
        view.last_missiles = missile_count;

        let air_contacts = view.session.program.host.world.missiles.air_contacts;
        if air_contacts > view.last_air_contacts {
            play_sfx_vol(&mut commands, view, "PE_Rocket_1", 0.75);
            view.last_air_contacts = air_contacts;
        }

        let bomb_impacts = view.session.program.host.world.bombers.impacts;
        if bomb_impacts > view.last_bomb_impacts {
            play_sfx_vol(&mut commands, view, "PE_Bomb_1", 0.85);
            view.last_bomb_impacts = bomb_impacts;
        }

        let lightning_count = view
            .session
            .program
            .host
            .world
            .rewrite
            .as_ref()
            .map_or(0, |r| r.lightning().len());
        if lightning_count > view.last_lightning {
            play_sfx_vol(&mut commands, view, "E_lightning_1", 0.78);
            let thunder = ["E_Thunder_01", "E_Thunder_02", "E_Thunder_03"][lightning_count % 3];
            play_sfx_vol(&mut commands, view, thunder, 0.72);
        }
        view.last_lightning = lightning_count;

        let obj_notif = view
            .session
            .program
            .host
            .world
            .messages
            .as_ref()
            .and_then(|q| q.objective_notification())
            .map(str::to_string);
        if obj_notif.is_some() && obj_notif != view.last_objective_banner {
            play_sfx(&mut commands, view, "UI_HUD_Objective_1");
        }
        view.last_objective_banner = obj_notif;
    }

    let interactive_smooth = !view.verify && view.replay.is_none();
    let mut active_troops = std::collections::BTreeSet::new();
    let View {
        ref session,
        ref mut troops,
        ref clips,
        ..
    } = **view;
    let world_units = &session.program.host.world.units;
    let host_positions = &session.program.host.positions;
    let player_id = session.program.host.world.player;
    for (&id, troop) in troops.iter_mut() {
        let Some(unit) = world_units.get(&id) else {
            continue;
        };
        let Some(anim) = &unit.animation else {
            continue;
        };
        let alive_visible = unit.active == Some(true) && unit.health > 0.0;
        let dying_visible =
            unit.health <= 0.0 && anim.motion == Motion::Death && anim.elapsed < 2.8;
        if !alive_visible && !dying_visible {
            troop.last_pose_key = None;
            continue;
        }
        active_troops.insert(id);
        let dist_sq = host_positions.get(&id).map_or(0.0, |p| {
            let dx = p[0] - cam_eye.x;
            let dz = p[2] - cam_eye.z;
            dx * dx + dz * dz
        });
        if dist_sq > 180.0 * 180.0 && !troop.last_pose.is_empty() && Some(id) != player_id {
            continue;
        }
        let Some(source_model) = loaded.models.get(troop.model_index) else {
            continue;
        };
        let Some(clip_name) = anim.bindings.get(&anim.motion) else {
            continue;
        };
        let Some(clip) = clips.get(clip_name) else {
            continue;
        };
        let Ok(sample_t) = anim.sample_time(clip) else {
            continue;
        };
        let fps = if Some(id) == player_id || dist_sq < 65.0 * 65.0 {
            60.0
        } else if dist_sq < 130.0 * 130.0 {
            30.0
        } else {
            15.0
        };
        let motion_changed = troop
            .last_pose_key
            .is_some_and(|(prev_m, _)| prev_m != anim.motion);
        let key = (anim.motion, (sample_t * fps) as u32);
        if !motion_changed && troop.last_pose_key == Some(key) && !troop.last_pose.is_empty() {
            continue;
        }
        let Ok(mut pose) = bw_rust::skinning::pose(source_model, Some((clip, sample_t))) else {
            continue;
        };
        if interactive_smooth && troop.last_pose.len() == pose.len() {
            let blend_alpha = if motion_changed {
                (step_dt * 12.0).clamp(0.15, 0.45)
            } else {
                (step_dt * 22.0).clamp(0.25, 1.0)
            };
            for (dst, prev) in pose.iter_mut().zip(troop.last_pose.iter()) {
                let (s0, r0, t0) = prev.to_scale_rotation_translation();
                let (s1, r1, t1) = dst.to_scale_rotation_translation();
                if s0.is_finite() && r0.is_finite() && t0.is_finite() {
                    *dst = Mat4::from_scale_rotation_translation(
                        s0.lerp(s1, blend_alpha),
                        r0.slerp(r1, blend_alpha),
                        t0.lerp(t1, blend_alpha),
                    );
                }
            }
        }
        for p in &troop.parts {
            if source_model.nodes[p.node_index].has_skin_data
                && let Ok(vertices) =
                    bw_rust::skinning::deform(source_model, &pose, p.node_index, p.surface_index)
                && let Some(mut mesh) = meshes.get_mut(&p.mesh)
            {
                mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);
            }
        }
        troop.last_pose_key = Some(key);
        troop.last_pose = pose;
    }

    let current_positions: Vec<(ObjectId, [f32; 3])> = view
        .session
        .program
        .host
        .positions
        .iter()
        .map(|(&id, &p)| (id, p))
        .collect();
    let mut player_step_delta = 0.0f32;
    let mut moving_units = std::collections::BTreeSet::new();
    for (id, p) in &current_positions {
        let id = *id;
        let p = *p;
        if let Some(prev) = view.last_positions.insert(id, p) {
            let ds = (p[0] - prev[0]).hypot(p[2] - prev[2]);
            if ds > 1e-4 && ds < 10.0 {
                moving_units.insert(id);
                *view.wheel_angles.entry(id).or_insert(0.0) += ds;
                if Some(id) == player_id
                    && !view.session.program.host.world.frozen
                    && view.troops.contains_key(&id)
                {
                    player_step_delta += ds;
                }
                if ds > 0.04
                    && (view.frames % 12) == 0
                    && view.active_explosions.len() < 24
                    && view.session.program.host.world.water_units.contains(&id)
                {
                    let water_y = view.water_height + 0.05;
                    view.active_explosions.push(ExplosionEffect {
                        pos: Vec3::new(p[0], water_y, p[2]),
                        age: 0.32,
                        max_age: 0.56,
                        scale: 1.15,
                    });
                }
            }
        }
    }
    if player_step_delta > 0.0 {
        view.footstep_distance += player_step_delta;
        if view.footstep_distance >= 2.3 {
            view.footstep_distance = 0.0;
            let step_sfx = [
                "CM_Run_Stone_1",
                "CM_Run_Stone_2",
                "CM_Run_Stone_3",
                "CM_Run_Stone_4",
                "CM_Run_Stone_5",
            ][view.footstep_index % 5];
            view.footstep_index = view.footstep_index.wrapping_add(1);
            play_sfx_vol(&mut commands, view, step_sfx, 0.28);
        }
    }

    // Smooth mobile unit positions & rotations with soft visual separation
    {
        let pos_alpha = if interactive_smooth {
            (step_dt * 18.0).clamp(0.15, 1.0)
        } else {
            1.0
        };
        let active_mobile: Vec<(ObjectId, [f32; 3], bool)> = current_positions
            .iter()
            .filter_map(|&(id, p)| {
                let u = view.session.program.host.world.units.get(&id)?;
                if u.active == Some(false)
                    || u.health <= 0.0
                    || view.session.program.host.world.buildings.contains(&id)
                {
                    return None;
                }
                let is_troop = view.troops.contains_key(&id);
                Some((id, p, is_troop))
            })
            .collect();

        for &(id, mut target_p, is_troop) in &active_mobile {
            if interactive_smooth && Some(id) != player_id {
                let min_sep = if is_troop { 1.35f32 } else { 5.2f32 };
                let mut push_x = 0.0f32;
                let mut push_z = 0.0f32;
                for &(other_id, other_p, other_troop) in &active_mobile {
                    if other_id == id || other_troop != is_troop {
                        continue;
                    }
                    let dx = target_p[0] - other_p[0];
                    let dz = target_p[2] - other_p[2];
                    let dist_sq = dx * dx + dz * dz;
                    if dist_sq < min_sep * min_sep {
                        let dist = dist_sq.sqrt();
                        if dist > 1e-3 {
                            let overlap = (min_sep - dist) * 0.42;
                            push_x += (dx / dist) * overlap;
                            push_z += (dz / dist) * overlap;
                        } else if id.0 > other_id.0 {
                            let angle = (id.0 as f32) * 1.618;
                            push_x += angle.cos() * min_sep * 0.45;
                            push_z += angle.sin() * min_sep * 0.45;
                        }
                    }
                }
                target_p[0] += push_x.clamp(-2.5, 2.5);
                target_p[2] += push_z.clamp(-2.5, 2.5);
            }
            let entry = view.smoothed_positions.entry(id).or_insert(target_p);
            let jump_sq = (target_p[0] - entry[0]).powi(2) + (target_p[2] - entry[2]).powi(2);
            if !interactive_smooth || jump_sq > 20.0 * 20.0 {
                *entry = target_p;
            } else {
                entry[0] += (target_p[0] - entry[0]) * pos_alpha;
                entry[1] += (target_p[1] - entry[1]) * pos_alpha;
                entry[2] += (target_p[2] - entry[2]) * pos_alpha;
            }
        }

        let headings_snapshot: Vec<(ObjectId, f32)> = view
            .session
            .program
            .host
            .world
            .headings
            .iter()
            .map(|(&id, &h)| (id, h))
            .collect();
        let rot_alpha = if interactive_smooth {
            (step_dt * 14.0).clamp(0.12, 1.0)
        } else {
            1.0
        };
        let elapsed_t = time.elapsed_secs();
        for (id, heading) in headings_snapshot {
            let p = view
                .smoothed_positions
                .get(&id)
                .copied()
                .or_else(|| view.session.program.host.positions.get(&id).copied())
                .unwrap_or([0.0; 3]);
            let is_troop = view.troops.contains_key(&id);
            let is_water = view.session.program.host.world.water_units.contains(&id);
            let is_air = (view
                .session
                .program
                .host
                .world
                .non_ground_units
                .contains(&id)
                && !is_water)
                || view
                    .session
                    .program
                    .host
                    .world
                    .bombers
                    .orders
                    .contains_key(&id);
            let is_ground_veh = !is_troop
                && !is_water
                && !is_air
                && view.session.program.host.world.units.contains_key(&id)
                && !view.session.program.host.world.buildings.contains(&id);

            let target_rot = if is_ground_veh {
                let fwd = [heading.sin() * 2.0, heading.cos() * 2.0];
                let right = [heading.cos() * 1.6, -heading.sin() * 1.6];
                let [fx, fz] =
                    bw_rust::original_scene::source_to_grid(p[0] + fwd[0], p[2] + fwd[1]);
                let [bx, bz] =
                    bw_rust::original_scene::source_to_grid(p[0] - fwd[0], p[2] - fwd[1]);
                let [rx, rz] =
                    bw_rust::original_scene::source_to_grid(p[0] + right[0], p[2] + right[1]);
                let [lx, lz] =
                    bw_rust::original_scene::source_to_grid(p[0] - right[0], p[2] - right[1]);
                let pitch = if let (Some(hf), Some(hb)) =
                    (loaded.ground.height(fx, fz), loaded.ground.height(bx, bz))
                {
                    -((hf - hb) * 2.0 / 4.0).atan().clamp(-0.42, 0.42)
                } else {
                    0.0
                };
                let roll = if let (Some(hr), Some(hl)) =
                    (loaded.ground.height(rx, rz), loaded.ground.height(lx, lz))
                {
                    -((hr - hl) * 2.0 / 3.2).atan().clamp(-0.35, 0.35)
                } else {
                    0.0
                };
                Quat::from_rotation_y(heading)
                    * Quat::from_rotation_x(pitch)
                    * Quat::from_rotation_z(roll)
            } else if is_water && interactive_smooth {
                let phase = (id.0 as f32) * 0.37;
                let wave_pitch = (elapsed_t * 1.7 + phase).sin() * 0.025;
                let wave_roll = (elapsed_t * 1.3 + phase * 1.7).cos() * 0.032;
                Quat::from_rotation_y(heading)
                    * Quat::from_rotation_x(wave_pitch)
                    * Quat::from_rotation_z(wave_roll)
            } else if is_air && interactive_smooth {
                let prev_q = view
                    .smoothed_rotations
                    .get(&id)
                    .copied()
                    .unwrap_or_else(|| Quat::from_rotation_y(heading));
                let prev_fwd = prev_q * Vec3::Z;
                let prev_heading = prev_fwd.x.atan2(prev_fwd.z);
                let mut d_yaw = heading - prev_heading;
                while d_yaw > std::f32::consts::PI {
                    d_yaw -= std::f32::consts::TAU;
                }
                while d_yaw < -std::f32::consts::PI {
                    d_yaw += std::f32::consts::TAU;
                }
                let bank = (-d_yaw * 1.8).clamp(-0.48, 0.48);
                Quat::from_rotation_y(heading)
                    * Quat::from_rotation_x(0.06)
                    * Quat::from_rotation_z(bank)
            } else {
                Quat::from_rotation_y(heading)
            };

            let rot_entry = view.smoothed_rotations.entry(id).or_insert(target_rot);
            if !interactive_smooth {
                *rot_entry = target_rot;
            } else {
                *rot_entry = rot_entry.slerp(target_rot, rot_alpha);
            }
        }
    }

    let host = &view.session.program.host;
    let mut turret_aim: BTreeMap<ObjectId, (f32, f32)> = BTreeMap::new();
    for (&id, u) in &host.world.units {
        if u.active == Some(false) || u.health <= 0.0 || view.troops.contains_key(&id) {
            continue;
        }
        let target_pos = if Some(id) == current_player_id {
            current_aim_target
                .and_then(|tid| host.positions.get(&tid).copied())
                .map(|tp| {
                    let [gx, gz] = bw_rust::original_scene::source_to_grid(tp[0], tp[2]);
                    let gy =
                        (loaded.ground.height(gx, gz).unwrap_or(0.0) * 2.0).max(view.water_height);
                    Vec3::new(tp[0], tp[1] + gy + 1.8, tp[2])
                })
                .or_else(|| view.smoothed_camera.map(|(_, look)| look))
        } else if host.world.roster.contains(&id)
            && let Some(tid) = host.world.squad_target
            && let Some(&tp) = host.positions.get(&tid)
        {
            let [gx, gz] = bw_rust::original_scene::source_to_grid(tp[0], tp[2]);
            let gy = (loaded.ground.height(gx, gz).unwrap_or(0.0) * 2.0).max(view.water_height);
            Some(Vec3::new(tp[0], tp[1] + gy + 1.8, tp[2]))
        } else {
            view.session
                .combat
                .shots
                .iter()
                .rev()
                .find(|s| s.shooter == id)
                .map(|s| Vec3::from_array(s.to))
        };
        if let Some(tpos) = target_pos
            && let Some(&vp) = view
                .smoothed_positions
                .get(&id)
                .or_else(|| host.positions.get(&id))
        {
            let [gx, gz] = bw_rust::original_scene::source_to_grid(vp[0], vp[2]);
            let gy = (loaded.ground.height(gx, gz).unwrap_or(0.0) * 2.0).max(view.water_height);
            let vy = vp[1] + gy + 2.4;
            let dx = tpos.x - vp[0];
            let dz = tpos.z - vp[2];
            let dy = tpos.y - vy;
            let dist_xz = (dx * dx + dz * dz).sqrt();
            if dist_xz > 1.0 {
                let veh_heading = host.world.headings.get(&id).copied().unwrap_or(0.0);
                let mut rel_yaw = dx.atan2(dz) - veh_heading;
                while rel_yaw > std::f32::consts::PI {
                    rel_yaw -= std::f32::consts::TAU;
                }
                while rel_yaw < -std::f32::consts::PI {
                    rel_yaw += std::f32::consts::TAU;
                }
                let rel_pitch = -(dy / dist_xz).atan().clamp(-0.45, 0.35);
                turret_aim.insert(id, (rel_yaw, rel_pitch));
            }
        }
    }

    let rotor_angle = if interactive_smooth {
        time.elapsed_secs() * 26.0
    } else {
        (view.frames as f32) * 0.45
    };
    let mut player_parts = 0;
    let mut bomber_parts = 0;
    for (part, mut transform, mut visibility) in &mut actors {
        if !part.is_unit && !part.is_mobile {
            continue;
        }
        if part.is_fragment {
            if let Some(&frag_age) = view.destroyed_times.get(&part.id)
                && frag_age < 2.2
            {
                let mut root = part.root;
                if let Some(&heading) = host.world.headings.get(&part.id) {
                    let (scale, _, translation) = root.to_scale_rotation_translation();
                    root = Mat4::from_scale_rotation_translation(
                        scale,
                        Quat::from_rotation_y(heading),
                        translation,
                    );
                }
                if let Some(p_raw) = host.positions.get(&part.id) {
                    let mut p = *p_raw;
                    let [x, z] = bw_rust::original_scene::source_to_grid(p[0], p[2]);
                    let ground_y = loaded
                        .ground
                        .height(x, z)
                        .map_or(view.water_height, |h| (h * 2.0).max(view.water_height));
                    if part.grounded || part.is_water_vehicle {
                        p[1] += ground_y;
                    }
                    root.w_axis = Vec3::from_array(p).extend(1.0);
                }
                let base_mat = root * part.local;
                let (scale, base_rot, base_pos) = base_mat.to_scale_rotation_translation();
                let mut pos = base_pos
                    + part.frag_vel * frag_age
                    + Vec3::new(0.0, -11.0 * frag_age * frag_age, 0.0);
                let [fx, fz] = bw_rust::original_scene::source_to_grid(pos.x, pos.z);
                let floor_y = loaded
                    .ground
                    .height(fx, fz)
                    .map_or(view.water_height, |h| (h * 2.0).max(view.water_height))
                    + 0.25;
                if pos.y < floor_y {
                    pos.y = floor_y + (floor_y - pos.y) * 0.28;
                }
                let tumble = Quat::from_euler(
                    EulerRot::XYZ,
                    frag_age * 6.4,
                    frag_age * 4.7,
                    frag_age * 5.2,
                );
                let shrink = (1.0 - (frag_age / 2.2).powi(3)).clamp(0.05, 1.0);
                *transform = Transform {
                    translation: pos,
                    rotation: base_rot * tumble,
                    scale: scale * shrink,
                };
                if *visibility != Visibility::Visible {
                    *visibility = Visibility::Visible;
                }
            } else if *visibility != Visibility::Hidden {
                *visibility = Visibility::Hidden;
            }
            continue;
        }
        if host.world.player == Some(part.id) {
            player_parts += 1;
        }
        if view.troops.contains_key(&part.id) {
            if !active_troops.contains(&part.id) {
                if *visibility != Visibility::Hidden {
                    *visibility = Visibility::Hidden;
                }
                continue;
            }
        } else if let Some(u) = host
            .world
            .units
            .get(&part.id)
            .or_else(|| host.world.props.get(&part.id))
            && if u.health <= 0.0 {
                !part.keep_when_dead
            } else {
                u.active == Some(false)
            }
        {
            if *visibility != Visibility::Hidden {
                *visibility = Visibility::Hidden;
            }
            continue;
        }
        if !part.is_mobile && !host.world.bombers.orders.contains_key(&part.id) {
            if *visibility != Visibility::Visible {
                *visibility = Visibility::Visible;
            }
            continue;
        }
        if host.world.bombers.orders.contains_key(&part.id) {
            bomber_parts += 1;
        }
        if moving_units.contains(&part.id)
            && let Some(ref mesh_h) = part.tread_mesh
            && let Some(ref uvs) = part.tread_uvs
            && let Some(mut mesh) = meshes.get_mut(mesh_h)
        {
            let scroll =
                (view.wheel_angles.get(&part.id).copied().unwrap_or(0.0) * 0.35).rem_euclid(1.0);
            let scrolled: Vec<[f32; 2]> = uvs.iter().map(|&[u, v]| [u, v + scroll]).collect();
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, scrolled);
        }
        let mut root = part.root;
        if let Some(&smoothed_rot) = view.smoothed_rotations.get(&part.id) {
            let (scale, _, translation) = root.to_scale_rotation_translation();
            root = Mat4::from_scale_rotation_translation(scale, smoothed_rot, translation);
        } else if let Some(&heading) = host.world.headings.get(&part.id) {
            let (scale, _, translation) = root.to_scale_rotation_translation();
            root = Mat4::from_scale_rotation_translation(
                scale,
                Quat::from_rotation_y(heading),
                translation,
            );
        }
        if let Some(p_raw) = view
            .smoothed_positions
            .get(&part.id)
            .or_else(|| host.positions.get(&part.id))
        {
            let mut p = *p_raw;
            if part.is_water_vehicle {
                let [x, z] = bw_rust::original_scene::source_to_grid(p[0], p[2]);
                let surface_y = loaded
                    .ground
                    .height(x, z)
                    .map_or(view.water_height, |h| (h * 2.0).max(view.water_height));
                if p[1] < surface_y {
                    p[1] += surface_y;
                }
                if interactive_smooth {
                    let phase = (part.id.0 as f32) * 0.37;
                    p[1] += (time.elapsed_secs() * 1.9 + phase).sin() * 0.14;
                }
            } else if part.grounded {
                let [x, z] = bw_rust::original_scene::source_to_grid(p[0], p[2]);
                let Some(height) = loaded.ground.height(x, z) else {
                    if *visibility != Visibility::Hidden {
                        *visibility = Visibility::Hidden;
                    }
                    continue;
                };
                p[1] += height * 2.0;
            }
            root.w_axis = Vec3::from_array(p).extend(1.0);
        }
        let next_transform = if let Some(bone_idx) = part.bone
            && let Some(troop) = view.troops.get(&part.id)
            && let Some(&bone_mat) = troop.last_pose.get(bone_idx)
        {
            Transform::from_matrix(root * bone_mat * part.local)
        } else if let Some(axis) = part.rotor_axis {
            let spin = match axis {
                0 => Mat4::from_rotation_x(rotor_angle),
                2 => Mat4::from_rotation_z(rotor_angle),
                _ => Mat4::from_rotation_y(rotor_angle),
            };
            Transform::from_matrix(root * part.local * spin)
        } else if let Some(radius) = part.wheel_radius {
            let dist = view.wheel_angles.get(&part.id).copied().unwrap_or(0.0);
            let spin = Mat4::from_rotation_x(dist / radius.max(0.25));
            Transform::from_matrix(root * part.local * spin)
        } else if part.turret_role > 0 {
            let (rel_yaw, rel_pitch) = turret_aim.get(&part.id).copied().unwrap_or((0.0, 0.0));
            let yp = Vec3::new(part.turret_yaw_pivot[0], 0.0, part.turret_yaw_pivot[1]);
            let yaw_mat = Mat4::from_translation(yp)
                * Mat4::from_rotation_y(rel_yaw)
                * Mat4::from_translation(-yp);
            let local = if part.turret_role == 2 {
                let pitch_mat = Mat4::from_translation(part.turret_pivot)
                    * Mat4::from_rotation_x(rel_pitch)
                    * Mat4::from_translation(-part.turret_pivot);
                yaw_mat * pitch_mat * part.local
            } else {
                yaw_mat * part.local
            };
            Transform::from_matrix(root * local)
        } else {
            Transform::from_matrix(root * part.local)
        };
        if !transform
            .translation
            .abs_diff_eq(next_transform.translation, 1e-4)
            || !transform
                .rotation
                .abs_diff_eq(next_transform.rotation, 1e-4)
            || !transform.scale.abs_diff_eq(next_transform.scale, 1e-4)
        {
            *transform = next_transform;
        }
        if *visibility != Visibility::Visible {
            *visibility = Visibility::Visible;
        }
    }

    let aim_target = current_aim_target;
    let world_y_of = |id: ObjectId, raw_p: [f32; 3]| -> Vec3 {
        let [gx, gz] = bw_rust::original_scene::source_to_grid(raw_p[0], raw_p[2]);
        let ground_y = (loaded.ground.height(gx, gz).unwrap_or(0.0) * 2.0).max(view.water_height);
        let is_water = host.world.water_units.contains(&id);
        let is_air = (host.world.non_ground_units.contains(&id) && !is_water)
            || host.world.bombers.orders.contains_key(&id);
        let base_y = if is_air {
            raw_p[1]
        } else if is_water {
            if raw_p[1] < ground_y {
                raw_p[1] + ground_y
            } else {
                raw_p[1]
            }
        } else {
            raw_p[1] + ground_y
        };
        let height_offset = if view.troops.contains_key(&id) {
            2.45
        } else if host.world.buildings.contains(&id) {
            5.5
        } else {
            3.6
        };
        Vec3::new(raw_p[0], base_y + height_offset, raw_p[2])
    };

    for (mut transform, mut visibility) in &mut lock_bracket {
        if let Some(target_id) = aim_target
            && !host.world.frozen
            && let Some(&raw_p) = view
                .smoothed_positions
                .get(&target_id)
                .or_else(|| host.positions.get(&target_id))
        {
            let bracket_pos = world_y_of(target_id, raw_p) - Vec3::Y * 0.9;
            let pulse = 1.0 + (time.elapsed_secs() * 6.5).sin() * 0.12;
            let size = if view.troops.contains_key(&target_id) {
                1.45 * pulse
            } else {
                2.85 * pulse
            };
            *transform = Transform {
                translation: bracket_pos,
                rotation: cam_rot * Quat::from_rotation_z(time.elapsed_secs() * 1.6),
                scale: Vec3::new(size, size, 0.08),
            };
            if *visibility != Visibility::Visible {
                *visibility = Visibility::Visible;
            }
        } else if *visibility != Visibility::Hidden {
            *visibility = Visibility::Hidden;
        }
    }

    let player_pos_opt = player_id.and_then(|id| host.positions.get(&id).copied());
    let player_army = player_id
        .and_then(|id| host.world.units.get(&id))
        .map(|u| u.army.as_str())
        .unwrap_or("eSolarEmpire");
    let squad_ids: Vec<ObjectId> = host
        .world
        .units
        .iter()
        .filter_map(|(&id, u)| {
            (Some(id) != player_id
                && u.active == Some(true)
                && u.health > 0.0
                && !host.world.buildings.contains(&id)
                && (host.world.roster.contains(&id)
                    || (u.army == player_army
                        && host
                            .world
                            .activity
                            .get(&id)
                            .is_none_or(|a| a.enabled != Some(false)))))
            .then_some(id)
        })
        .collect();

    let mut bar_targets: Vec<(Vec3, f32, bool)> = Vec::new();
    if !host.world.frozen {
        for &id in &squad_ids {
            if bar_targets.len() >= 24 {
                break;
            }
            if let Some(u) = host.world.units.get(&id)
                && let Some(&raw_p) = view
                    .smoothed_positions
                    .get(&id)
                    .or_else(|| host.positions.get(&id))
            {
                let bar_pos = world_y_of(id, raw_p);
                if bar_pos.distance_squared(cam_eye) < 110.0 * 110.0 {
                    let frac = (u.health / u.max_health.max(1.0)).clamp(0.05, 1.0) as f32;
                    bar_targets.push((bar_pos, frac, true));
                }
            }
        }
        if let Some(target_id) = aim_target
            && !squad_ids.contains(&target_id)
            && let Some(ent) = host
                .world
                .units
                .get(&target_id)
                .or_else(|| host.world.props.get(&target_id))
            && ent.health > 0.0
            && let Some(&raw_p) = view
                .smoothed_positions
                .get(&target_id)
                .or_else(|| host.positions.get(&target_id))
        {
            let bar_pos = world_y_of(target_id, raw_p) + Vec3::Y * 0.45;
            let frac = (ent.health / ent.max_health.max(1.0)).clamp(0.05, 1.0) as f32;
            bar_targets.push((bar_pos, frac, false));
        }
    }
    let _ = player_pos_opt;
    for (bar, mut transform, mut visibility) in &mut world_bars {
        if let Some(&(pos, frac, is_ally)) = bar_targets.get(bar.0) {
            let max_w = if is_ally { 1.25 } else { 1.85 };
            *transform = Transform {
                translation: pos,
                rotation: cam_rot,
                scale: Vec3::new(max_w * frac, 0.14, 0.05),
            };
            if *visibility != Visibility::Visible {
                *visibility = Visibility::Visible;
            }
        } else if *visibility != Visibility::Hidden {
            *visibility = Visibility::Hidden;
        }
    }

    let paused = host.world.tutorial.as_ref().is_some_and(|t| t.paused);
    let waypoint_cutscene = host
        .world
        .cameras
        .as_ref()
        .is_some_and(|rig| rig.active.is_some() && rig.chase_target().is_none());
    let frozen_cutscene = (host.world.frozen || waypoint_cutscene) && !paused;
    let hud_active = !frozen_cutscene && !view.pause_menu_open;
    set_panel_vis(
        UiPanelSlot::Crosshair,
        if hud_active {
            Visibility::Visible
        } else {
            Visibility::Hidden
        },
    );
    set_panel_vis(
        UiPanelSlot::RadarPanel,
        if hud_active {
            Visibility::Visible
        } else {
            Visibility::Hidden
        },
    );
    set_panel_vis(
        UiPanelSlot::TargetInfoPill,
        if hud_active && aim_target.is_some() {
            Visibility::Visible
        } else {
            Visibility::Hidden
        },
    );
    set_panel_vis(
        UiPanelSlot::CinematicTop,
        if frozen_cutscene {
            Visibility::Visible
        } else {
            Visibility::Hidden
        },
    );
    set_panel_vis(
        UiPanelSlot::CinematicBottom,
        if frozen_cutscene {
            Visibility::Visible
        } else {
            Visibility::Hidden
        },
    );
    set_panel_vis(
        UiPanelSlot::SquadCommand,
        if frozen_cutscene {
            Visibility::Hidden
        } else {
            Visibility::Visible
        },
    );

    // -------------------------------------------------------------------------
    // 1. TOP-LEFT CO RADIO DIALOGUE BOX & OBJECTIVE BANNER
    // -------------------------------------------------------------------------
    let current_msg = host.world.messages.as_ref().and_then(|q| q.current());
    let subtitle = clean_bw_text(
        host.world
            .messages
            .as_ref()
            .and_then(|q| q.text())
            .unwrap_or(""),
    );
    set_panel_vis(
        UiPanelSlot::CoRadio,
        if current_msg.is_some() && !subtitle.is_empty() {
            Visibility::Visible
        } else {
            Visibility::Hidden
        },
    );
    if let Some(req) = current_msg {
        let (speaker, tex_key) = co_radio_speaker_and_portrait(view, req);
        if let Some(h) = view
            .ui_handles
            .get(tex_key)
            .or_else(|| view.ui_handles.get("co_sleiqo_h"))
        {
            for (slot, mut img, _) in ui_images.iter_mut() {
                if *slot == UiImageSlot::CoRadioPortrait && img.image != *h {
                    img.image = h.clone();
                }
            }
        }
        set_text(UiTextSlot::CoRadioSpeaker, speaker.to_string());
        set_text(UiTextSlot::CoRadioBody, subtitle.clone());
    }

    let mut guidance = Vec::new();
    if let Some(player_pos) = player_pos_opt {
        for marker in &view.session.markers {
            if let Ok(Some(p)) = marker.position(host) {
                let dx = (p[0] - player_pos[0]) as f64;
                let dz = (p[2] - player_pos[2]) as f64;
                let aim = host.world.player_aim.unwrap_or([0., 0., 1.]);
                let angle = (dx.atan2(dz) - aim[0].atan2(aim[2]) + std::f64::consts::PI)
                    .rem_euclid(std::f64::consts::TAU)
                    - std::f64::consts::PI;
                guidance.push((dx.hypot(dz), angle.to_degrees(), p));
            }
        }
    }
    guidance.sort_by(|a, b| a.0.total_cmp(&b.0));
    let guidance_str = guidance
        .first()
        .map(|(distance, angle, _)| {
            format!(
                "OBJECTIVE STAR: {distance:.0}m {}",
                if angle.abs() < 12. {
                    "AHEAD"
                } else if *angle > 0. {
                    "LEFT"
                } else {
                    "RIGHT"
                }
            )
        })
        .unwrap_or_else(|| "OBJECTIVE AREA REACHED".into());
    let notification = clean_bw_text(
        host.world
            .messages
            .as_ref()
            .and_then(|q| q.objective_notification())
            .unwrap_or(""),
    )
    .replace('_', " ");
    set_text(
        UiTextSlot::ObjectiveBanner,
        if frozen_cutscene {
            "CINEMATIC SEQUENCE | Press [Enter] or [Esc] to Skip".into()
        } else if !notification.is_empty() {
            format!("{notification} | {guidance_str}")
        } else {
            guidance_str
        },
    );

    // -------------------------------------------------------------------------
    // 2. TOP-RIGHT ROTATING RADAR / COMPASS MINIMAP
    // -------------------------------------------------------------------------
    let radar_dist_str = guidance
        .first()
        .map(|(d, _, _)| format!("{d:.0}m"))
        .unwrap_or_else(|| "OBJ".into());
    set_text(UiTextSlot::RadarDistanceLabel, radar_dist_str);

    let cam_right = cam_rot * Vec3::X;
    let cam_fwd = cam_rot * -Vec3::Z;
    let right_xz = Vec2::new(cam_right.x, cam_right.z).normalize_or_zero();
    let fwd_xz = Vec2::new(cam_fwd.x, cam_fwd.z).normalize_or_zero();
    let radar_center = Vec2::new(84.0, 84.0);
    let radar_max_px = 52.0f32;
    let radar_range_m = 95.0f32;

    // Collect radar blips:
    // index 0: nearest Gold Star objective
    // index 1: player center dot
    // index 2..24: friendly & enemy units within radar range
    let mut unit_blips: Vec<(f32, Vec2, Color)> = Vec::new();
    if let Some(ppos) = player_pos_opt {
        for (&uid, u) in &host.world.units {
            if Some(uid) == player_id || u.active != Some(true) || u.health <= 0.0 {
                continue;
            }
            let Some(&upos) = host.positions.get(&uid) else {
                continue;
            };
            let delta = Vec2::new(upos[0] - ppos[0], upos[2] - ppos[2]);
            let dist = delta.length();
            if dist > radar_range_m {
                continue;
            }
            let rx = delta.dot(right_xz) / radar_range_m * radar_max_px;
            let ry = -delta.dot(fwd_xz) / radar_range_m * radar_max_px;
            let color = if u.army == "eSolarEmpire"
                || u.army == "eWesternFrontier"
                || host.world.roster.contains(&uid)
            {
                Color::srgb(0.28, 0.98, 0.36)
            } else if u.army != "eNoArmy" {
                Color::srgb(1.0, 0.22, 0.18)
            } else {
                continue;
            };
            unit_blips.push((dist, radar_center + Vec2::new(rx, ry), color));
        }
    }
    unit_blips.sort_by(|a, b| a.0.total_cmp(&b.0));

    let blip_tex = view.ui_handles.get("blip_01").cloned().unwrap_or_default();
    let star_tex = view
        .ui_handles
        .get("obj_star_primary")
        .cloned()
        .unwrap_or_default();

    for (blip, mut node, mut img, mut vis) in radar_blips.iter_mut() {
        if !hud_active {
            if *vis != Visibility::Hidden {
                *vis = Visibility::Hidden;
            }
            continue;
        }
        match blip.0 {
            0 => {
                if let (Some(ppos), Some((_, _, star_pos))) = (player_pos_opt, guidance.first()) {
                    let delta = Vec2::new(star_pos[0] - ppos[0], star_pos[2] - ppos[2]);
                    let rx = delta.dot(right_xz) / radar_range_m * radar_max_px;
                    let ry = -delta.dot(fwd_xz) / radar_range_m * radar_max_px;
                    let offset = Vec2::new(rx, ry);
                    let clamped = if offset.length() > radar_max_px {
                        offset.normalize_or_zero() * radar_max_px
                    } else {
                        offset
                    };
                    let pt = radar_center + clamped;
                    node.left = px((pt.x - 8.0).round() as i32);
                    node.top = px((pt.y - 8.0).round() as i32);
                    node.width = px(16);
                    node.height = px(16);
                    if img.image != star_tex {
                        img.image = star_tex.clone();
                    }
                    img.color = Color::WHITE;
                    if *vis != Visibility::Inherited {
                        *vis = Visibility::Inherited;
                    }
                } else if *vis != Visibility::Hidden {
                    *vis = Visibility::Hidden;
                }
            }
            1 => {
                node.left = px((radar_center.x - 5.0) as i32);
                node.top = px((radar_center.y - 5.0) as i32);
                node.width = px(10);
                node.height = px(10);
                if img.image != blip_tex {
                    img.image = blip_tex.clone();
                }
                img.color = Color::srgb(1.0, 0.96, 0.35);
                if *vis != Visibility::Inherited {
                    *vis = Visibility::Inherited;
                }
            }
            idx => {
                if let Some((_, pt, color)) = unit_blips.get(idx - 2) {
                    node.left = px((pt.x - 4.0).round() as i32);
                    node.top = px((pt.y - 4.0).round() as i32);
                    node.width = px(8);
                    node.height = px(8);
                    if img.image != blip_tex {
                        img.image = blip_tex.clone();
                    }
                    img.color = *color;
                    if *vis != Visibility::Inherited {
                        *vis = Visibility::Inherited;
                    }
                } else if *vis != Visibility::Hidden {
                    *vis = Visibility::Hidden;
                }
            }
        }
    }

    // -------------------------------------------------------------------------
    // 3. CENTER RETICLE, WEAPON POWER ARCS & TARGET LOCK PILL
    // -------------------------------------------------------------------------
    let (weapon_power, weapon_busy) = player_id
        .and_then(|pid| view.session.combat.weapon_bar_hud(pid))
        .unwrap_or((1.0, false));
    let power_f32 = (weapon_power as f32).clamp(0.0, 1.0);
    let power_color = if weapon_busy {
        Color::srgba(1.0, 0.24, 0.18, 0.95)
    } else {
        Color::srgba(
            1.0 - power_f32 * 0.65,
            0.35 + power_f32 * 0.63,
            0.22,
            0.38 + power_f32 * 0.60,
        )
    };
    let locked_hud = view.locked_target.is_some() || (aim_target.is_some() && action.lock);
    let (ret_key, ret_color) = if locked_hud {
        ("ret_lock", Color::srgb(1.0, 0.32, 0.24))
    } else if aim_target.is_some() {
        ("ret_rnge", Color::srgb(1.0, 0.90, 0.32))
    } else {
        ("ret_regl", Color::srgba(0.95, 0.98, 0.92, 0.92))
    };
    let ret_handle = view.ui_handles.get(ret_key).cloned().unwrap_or_default();

    let target_info = aim_target.and_then(|tid| {
        host.world.entity(tid).ok().map(|e| {
            let (icon_key, label, _, _) = unit_profile(view, tid);
            let hp_pct = (e.health / e.max_health.max(1.0) * 100.0).clamp(0.0, 100.0) as f32;
            (icon_key, label, hp_pct)
        })
    });
    if let Some((_, label, hp_pct)) = target_info {
        set_text(UiTextSlot::TargetEnemyLabel, label.to_string());
        for (mut node, mut bg) in target_hp_fills.iter_mut() {
            node.width = Val::Percent(hp_pct);
            bg.0 = if hp_pct <= 0.0 {
                Color::NONE
            } else if hp_pct > 50.0 {
                Color::srgb(0.98, 0.32, 0.24)
            } else {
                Color::srgb(1.0, 0.62, 0.18)
            };
        }
    }

    // -------------------------------------------------------------------------
    // 4. BOTTOM PLAYER HUB & 5 SQUAD COMMAND SLOTS
    // -------------------------------------------------------------------------
    let (p_icon_key, p_label, _, _) = player_id.map(|pid| unit_profile(view, pid)).unwrap_or((
        "s_grunt01",
        "SOLAR EMPIRE VET",
        "WS_Rifle_Fire",
        1,
    ));
    let p_portrait_handle = view.ui_handles.get(p_icon_key).cloned().unwrap_or_default();

    if let Some(pid) = player_id
        && let Some(punit) = host.world.units.get(&pid)
    {
        let hp_pct = (punit.health / punit.max_health.max(1.0) * 100.0).clamp(0.0, 100.0) as f32;
        set_text(UiTextSlot::PlayerStatus, p_label.to_string());
        set_text(UiTextSlot::PlayerHpText, format!("{hp_pct:.0}%"));
        for (mut node, mut bg) in player_hp_fills.iter_mut() {
            node.width = Val::Percent(hp_pct);
            bg.0 = if hp_pct <= 0.0 {
                Color::NONE
            } else if hp_pct > 50.0 {
                Color::srgb(0.24, 0.95, 0.35)
            } else if hp_pct > 25.0 {
                Color::srgb(0.98, 0.82, 0.22)
            } else {
                Color::srgb(0.98, 0.24, 0.20)
            };
        }
    }

    let is_following = !host.world.squad_follow.is_empty();
    let is_attacking = host
        .world
        .roster
        .iter()
        .any(|id| Some(*id) != player_id && host.world.orders.contains_key(id));
    let (squad_mode, order_all_key, order_single_key) = if is_following {
        ("FOLLOW", "icon_follow_all", "icon_follow_single")
    } else if is_attacking {
        ("ATTACK", "icon_attack_all", "icon_attack_single")
    } else {
        ("SENTRY", "icon_sentry_all", "icon_sentry_single")
    };
    let order_icon_all_handle = view
        .ui_handles
        .get(order_all_key)
        .cloned()
        .unwrap_or_default();
    let order_icon_single_handle = view
        .ui_handles
        .get(order_single_key)
        .cloned()
        .unwrap_or_default();

    let mut slot_counts = [0usize; 5];
    let mut slot_hp_sum = [0.0f64; 5];
    let mut slot_max_sum = [0.0f64; 5];
    let mut slot_icons = [
        "icon_all",
        "s_grunt01",
        "s_hose01",
        "s_antiarm01",
        "s_htnk01",
    ];
    for &sid in &squad_ids {
        if let Some(u) = host.world.units.get(&sid) {
            let (u_icon, _, _, slot_idx) = unit_profile(view, sid);
            slot_counts[0] += 1;
            slot_hp_sum[0] += u.health;
            slot_max_sum[0] += u.max_health.max(1.0);
            if slot_idx < 5 {
                if slot_counts[slot_idx] == 0 {
                    slot_icons[slot_idx] = u_icon;
                }
                slot_counts[slot_idx] += 1;
                slot_hp_sum[slot_idx] += u.health;
                slot_max_sum[slot_idx] += u.max_health.max(1.0);
            }
        }
    }

    for (idx, count) in slot_counts.iter().enumerate() {
        set_text(UiTextSlot::SquadSlotCount(idx), format!("x{count}"));
    }
    for (slot_fill, mut node, mut bg) in squad_hp_fills.iter_mut() {
        let idx = slot_fill.0.min(4);
        let pct = if slot_counts[idx] > 0 && slot_max_sum[idx] > 0.0 {
            (slot_hp_sum[idx] / slot_max_sum[idx] * 100.0).clamp(0.0, 100.0) as f32
        } else {
            0.0
        };
        node.width = Val::Percent(pct);
        bg.0 = if pct <= 0.0 {
            Color::NONE
        } else if pct > 50.0 {
            Color::srgb(0.24, 0.95, 0.35)
        } else if pct > 25.0 {
            Color::srgb(0.98, 0.82, 0.22)
        } else {
            Color::srgb(0.98, 0.24, 0.20)
        };
    }

    for (slot, mut img, mut vis) in ui_images.iter_mut() {
        match *slot {
            UiImageSlot::ReticleCenter => {
                if img.image != ret_handle {
                    img.image = ret_handle.clone();
                }
                img.color = ret_color;
            }
            UiImageSlot::ReticleEnemyTag => {
                let want_vis = if aim_target.is_some() {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
                if *vis != want_vis {
                    *vis = want_vis;
                }
            }
            UiImageSlot::ReticlePowerLeft | UiImageSlot::ReticlePowerRight => {
                img.color = power_color;
            }
            UiImageSlot::TargetEnemyIcon => {
                if let Some((icon_key, _, _)) = target_info
                    && let Some(h) = view.ui_handles.get(icon_key)
                    && img.image != *h
                {
                    img.image = h.clone();
                }
            }
            UiImageSlot::PlayerUnitPortrait if img.image != p_portrait_handle => {
                img.image = p_portrait_handle.clone();
            }
            UiImageSlot::SquadSlotHub(idx) => {
                let idx = idx.min(4);
                img.color = if idx == view.selected_squad_slot {
                    Color::srgb(1.0, 0.96, 0.55)
                } else if slot_counts[idx] > 0 {
                    Color::srgba(0.88, 0.94, 0.88, 0.92)
                } else {
                    Color::srgba(0.42, 0.46, 0.44, 0.55)
                };
            }
            UiImageSlot::SquadSlotIcon(idx) => {
                let idx = idx.min(4);
                if idx > 0
                    && let Some(h) = view.ui_handles.get(slot_icons[idx])
                    && img.image != *h
                {
                    img.image = h.clone();
                }
            }
            UiImageSlot::SquadSlotOrder(idx) => {
                let idx = idx.min(4);
                let target_order_handle = if idx == 0 {
                    &order_icon_all_handle
                } else {
                    &order_icon_single_handle
                };
                if img.image != *target_order_handle {
                    img.image = target_order_handle.clone();
                }
                let want_vis = if slot_counts[idx] > 0 {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
                if *vis != want_vis {
                    *vis = want_vis;
                }
            }
            _ => {}
        }
    }

    set_text(
        UiTextSlot::SquadStatus,
        format!(
            "ORDER: {squad_mode} | [C/F] Call  [MMB/G] Attack  [X] Wait  [T] Transfer  [P] Pause  [F1] Victory"
        ),
    );

    set_panel_vis(
        UiPanelSlot::TutorialModal,
        if paused || view.pause_menu_open {
            Visibility::Visible
        } else {
            Visibility::Hidden
        },
    );
    if view.pause_menu_open {
        let items = [
            "RESUME MISSION",
            "RESTART MISSION (R)",
            "COMPLETE MISSION & OPEN VICTORY DEBRIEFING",
            "RETURN TO CAMPAIGN MISSION SELECT MENU",
            "QUIT TO DESKTOP",
        ];
        let mut body = String::from("=== TACTICAL PAUSE MENU (Up/Down + Enter) ===\n\n");
        for (i, label) in items.iter().enumerate() {
            if i == view.pause_menu_index {
                body.push_str(&format!("  > {label}\n"));
            } else {
                body.push_str(&format!("    {label}\n"));
            }
        }
        set_text(UiTextSlot::TutorialModal, body);
    } else if paused {
        set_text(
            UiTextSlot::TutorialModal,
            format!(
                "=== TACTICAL TRAINING BRIEFING ===\n\n{}\n\n* Hold [Right Mouse Button / Tab / L] to lock onto targets\n* Press [Left Mouse Button / Space] to fire\n\n> PRESS [ENTER] TO CONTINUE",
                if subtitle.is_empty() {
                    "Lock onto the training targets and fire your weapon!"
                } else {
                    &subtitle
                }
            ),
        );
    }

    let warning_count = host
        .world
        .rewrite
        .as_ref()
        .map_or(0, |r| r.warnings().len());
    let phase = if view.error.is_some() {
        "BLOCKED - R restarts"
    } else if matches!(
        host.world.mission_complete,
        Some(MissionOutcome::Won { .. })
    ) {
        "VICTORY - R restarts"
    } else if matches!(
        host.world.mission_complete,
        Some(MissionOutcome::Lost { .. })
    ) {
        "DEFEAT - R restarts"
    } else if view.hold_replay_end && view.replay.as_ref().is_some_and(|r| r.is_empty()) {
        "Recorded prefix held - not completed gameplay"
    } else if paused {
        "Tutorial - paused"
    } else if frozen_cutscene {
        "Original scripted cinematic"
    } else {
        "Player control"
    };
    let next_hud = format!(
        "{} | {phase} | {:.1}s | {} scripts | Squad {} | Hits {}/{} | [P] Menu [F1] Victory {}",
        view.current_level,
        host.time,
        view.session.scheduler.threads.len(),
        host.world.roster.len(),
        view.session.combat.hits,
        view.session.combat.fired,
        if warning_count > 0 {
            format!("(warn {warning_count})")
        } else {
            view.error.clone().unwrap_or_default()
        }
    );
    for mut text in &mut hud {
        if **text != next_hud {
            **text = next_hud.clone();
        }
    }

    if view.replay.is_some() && view.frames == view.replay_check_frame {
        if view.expect_ambush {
            assert_eq!(
                view.session.program.vm.globals.get("cutsceneBombedBegin"),
                Some(&bw_rust::lua50::Value::Number(2.))
            );
            assert!(!view.session.program.host.world.frozen);
            assert!(
                view.ambush_frames >= 1000,
                "Must render the sequence, not only preroll it"
            );
            assert!(view.session.program.host.world.missiles.air_contacts > 0);
            assert!(view.session.program.host.world.bombers.impacts > 0);
            println!(
                "ORIGINAL_AMBUSH_RENDER_PASS rendered_ambush_frames={} missile_air_contacts={} bomb_impacts={}; original camera/script sequence reached control return; not native animation/physics or hardware-audio parity",
                view.ambush_frames,
                view.session.program.host.world.missiles.air_contacts,
                view.session.program.host.world.bombers.impacts
            );
        }
        if view.expect_staff {
            let w = &view.session.program.host.world;
            assert_eq!(
                view.session
                    .program
                    .vm
                    .globals
                    .get("cutsceneStaffBearerBegin"),
                Some(&bw_rust::lua50::Value::Number(2.))
            );
            assert!(!w.frozen);
            assert_eq!(w.player, Some(ObjectId(550035312)));
            assert!(player_parts > 0, "Transferred player model absent");
            assert!(
                view.staff_frames >= 1000,
                "Must render the staff cutscene sequence, not only preroll it"
            );
            assert_eq!(w.objectives[&ObjectId(550041656)], (true, true));
            assert_eq!(w.objectives[&ObjectId(550035435)], (true, false));
            assert_eq!(w.following[&ObjectId(550030859)].node, ObjectId(550030756));
            assert_eq!(w.following[&ObjectId(550030356)].node, ObjectId(550031659));
            println!(
                "ORIGINAL_STAFF_RENDER_PASS rendered_staff_frames={} player={:?} following={}; original staff transfer/battle-waypoint/cutscene sequence reached control return; not native animation/physics or hardware-audio parity",
                view.staff_frames,
                w.player,
                w.following.len()
            );
        }
        assert!(
            view.error.is_none(),
            "Combat render blocked: {:?}",
            view.error
        );
        assert!(view.session.program.host.world.objectives[&ObjectId(550032165)].1);
        assert!(view.session.combat.hits > 0);
        if let Some(id) = view.session.program.host.world.bombers.orders.keys().next() {
            assert!(
                bomber_parts > 0,
                "Ordered aircraft must have rendered parts"
            );
            println!(
                "FIRST_BOMBER_PREFIX_RENDER_PASS parts={} position={:?}; not complete cinematic or anti-air acceptance",
                bomber_parts, view.session.program.host.positions[id]
            );
        }
        println!(
            "FIRST_COMBAT_RENDER_PASS authored_objective=550032165 complete=true fired={} hits={}; not full mission victory acceptance",
            view.session.combat.fired, view.session.combat.hits
        );
    }
    if view.verify && view.frames == 160 {
        assert!(
            view.session.combat.fired > 0,
            "Synthetic LMB must reach the combat runtime"
        );
        assert!(
            view.error.is_none(),
            "Mission verification blocked: {:?}",
            view.error
        );
        assert!(player_parts > 0, "Player model absent");
        let p = host.positions[&host.world.player.unwrap()];
        let start = view.start.unwrap();
        assert!(
            (p[0] - start[0]).hypot(p[2] - start[2]) > 0.1,
            "Player did not move"
        );
        println!(
            "FIRST_MISSION_MOVEMENT_RENDER_PASS scripts={} player_parts={player_parts} start={start:?} end={p:?}; not combat/victory acceptance",
            view.session.scheduler.threads.len()
        );
    }
}
