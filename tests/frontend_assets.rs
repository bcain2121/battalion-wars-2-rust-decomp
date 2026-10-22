//! Local original assets only; successful decoding is not frontend/playthrough acceptance.
use bw_rust::{
    archive::{chunks, named_payload},
    texture::decode_named,
};
#[test]
#[ignore = "requires local frontend disc extraction"]
fn frontend_textures_decode_or_explicitly_reject_short_tiles() {
    let data = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/local/extracted/files/Data/CompoundFiles/Frontend2_Level.res"
    ))
    .unwrap();
    let mut count = 0;
    let mut failures = Vec::new();
    let mut formats = std::collections::BTreeMap::new();
    for c in chunks(&data).unwrap() {
        if c.tag != *b"RXET" {
            continue;
        }
        let (_, body) = named_payload(c.payload).unwrap();
        for g in chunks(body).unwrap() {
            if g.tag != *b"FTBG" {
                continue;
            }
            for t in chunks(&g.payload[4..]).unwrap() {
                if t.tag != *b"DXTG" {
                    continue;
                }
                let p = t.payload;
                let end = p[..32].iter().position(|v| *v == 0).unwrap_or(32);
                let name = std::str::from_utf8(&p[..end]).unwrap();
                match decode_named(&data, name) {
                    Ok(decoded) => {
                        assert_eq!(decoded.rgba.len(), decoded.width * decoded.height * 4)
                    }
                    Err(e) => failures.push(format!("{name}: {e}")),
                };
                *formats.entry(p[52..56].to_vec()).or_insert(0) += 1;
                count += 1;
            }
        }
    }
    println!("failures={failures:?}");
    assert_eq!(
        failures,
        vec![
            "OP2_screen: Tiled texture byte count mismatch",
            "OP1_desk: Tiled texture byte count mismatch",
            "OP4_nunch: Tiled texture byte count mismatch",
            "OP5_screen2: Tiled texture byte count mismatch",
            "PHTO_06_SolBeach: RGBA8 byte count mismatch",
            "FM_panel_left: RGBA8 byte count mismatch",
            "FM_panel_mid: RGBA8 byte count mismatch"
        ]
    );
    assert_eq!(count, 432);
    assert_eq!(formats.len(), 8);
    println!("frontend_texture_count={count} formats={formats:?}");
}
