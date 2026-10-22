//! Initial BW2 model decoder. Binary layouts cross-checked against local assets.
//! Retains hierarchy and matrix indices; does NOT claim skinning/animation parity.
use crate::{
    Result,
    archive::{chunks, named_payload},
};
use serde::Serialize;

struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}
impl<'a> Cursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(n).ok_or("Offset overflow")?;
        let value = self.data.get(self.pos..end).ok_or("Truncated model data")?;
        self.pos = end;
        Ok(value)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn be16(&mut self) -> Result<u16> {
        Ok(u16::from_be_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn be32(&mut self) -> Result<u32> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn string(&mut self) -> Result<String> {
        let n = self.be32()? as usize;
        let raw = self.take(n)?;
        Ok(std::str::from_utf8(raw)
            .map_err(|e| e.to_string())?
            .trim_end_matches('\0')
            .into())
    }
}
#[derive(Debug, Clone, Serialize)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: Option<[f32; 3]>,
    pub uv: [f32; 2],
    pub matrix: Option<u8>,
}
#[derive(Debug, Serialize)]
pub struct Surface {
    pub material: usize,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub lod: usize,
    pub bone_palette: Vec<usize>,
}
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Bounds {
    pub min: [f32; 3],
    pub max: [f32; 3],
}
#[derive(Debug, Serialize)]
pub struct Node {
    pub name: String,
    /// Little-endian header kind. Native name-tag resolution may override this value;
    /// an unmatched ordinary name keeps it (0x803293e0..0x803293f8).
    pub authored_kind: u16,
    pub transform: [f32; 11],
    pub parent: Option<usize>,
    pub scale: f32,
    pub bounds: Option<Bounds>,
    pub materials: Vec<[String; 4]>,
    pub material_blend_modes: Vec<u16>,
    pub surfaces: Vec<Surface>,
    pub tags: Vec<String>,
    pub has_skin_data: bool,
    pub inverse_bind: Vec<[f32; 16]>,
}
#[derive(Debug, Serialize)]
pub struct Model {
    pub name: String,
    pub version: [u32; 2],
    pub source: String,
    /// Authored big-endian header center XYZ/radius; not a generated mesh bound.
    /// Retained separately from node XOBB and physics-body shapes.
    pub header_sphere: [f32; 4],
    pub nodes: Vec<Node>,
    /// Original TCNC order; parent fields alone cannot recover sibling insertion order.
    pub hierarchy_links: Vec<(usize, usize)>,
}
#[derive(Debug, Clone)]
struct Ref {
    position: usize,
    normal: Option<usize>,
    uv: Option<usize>,
    matrix: Option<u8>,
}

fn read_index(c: &mut Cursor<'_>, mode: u32) -> Result<Option<usize>> {
    match mode {
        0 => Ok(None),
        2 => Ok(Some(c.u8()? as usize)),
        3 => Ok(Some(c.be16()? as usize)),
        _ => Err("Direct GX attribute requires format decoding".into()),
    }
}
fn display_list(data: &[u8], descriptor: &mut [u32; 2]) -> Result<Vec<(u8, Vec<Ref>)>> {
    let mut c = Cursor::new(data);
    let mut draws = Vec::new();
    while c.pos < data.len() {
        let command = c.u8()?;
        match command {
            0 => {}
            8 => {
                let register = c.u8()?;
                let value = c.be32()?;
                match register {
                    0x50 => descriptor[0] = value,
                    0x60 => descriptor[1] = value,
                    _ => return Err(format!("Unsupported CP register {register:#x}")),
                }
            }
            0x10 => {
                let header = c.be32()?;
                let words = (header >> 16) as usize + 1;
                c.take(words * 4)?;
            }
            op if matches!(op & 0xf8, 0x80 | 0x90 | 0x98 | 0xa0) => {
                let count = c.be16()? as usize;
                let mut refs = Vec::with_capacity(count);
                for _ in 0..count {
                    let mut matrix = None;
                    for bit in 0..9 {
                        if descriptor[0] & (1 << bit) != 0 {
                            let value = c.u8()?;
                            if bit == 0 {
                                matrix = Some(value);
                            }
                        }
                    }
                    let position = read_index(&mut c, (descriptor[0] >> 9) & 3)?
                        .ok_or("Missing GX position index")?;
                    let normal = read_index(&mut c, (descriptor[0] >> 11) & 3)?;
                    for shift in [13, 15] {
                        read_index(&mut c, (descriptor[0] >> shift) & 3)?;
                    }
                    let mut uv = None;
                    for channel in 0..8 {
                        let index = read_index(&mut c, (descriptor[1] >> (channel * 2)) & 3)?;
                        if channel == 0 {
                            uv = index;
                        }
                    }
                    refs.push(Ref {
                        position,
                        normal,
                        uv,
                        matrix,
                    });
                }
                draws.push((op & 0xf8, refs));
            }
            _ => {
                return Err(format!(
                    "Unsupported GX command {command:#x} at {}",
                    c.pos - 1
                ));
            }
        }
    }
    Ok(draws)
}
fn triangles(op: u8, count: usize) -> Result<Vec<u32>> {
    let mut indices = Vec::new();
    match op {
        0x98 => {
            for i in 2..count {
                let i = i as u32;
                if i.is_multiple_of(2) {
                    indices.extend_from_slice(&[i - 2, i - 1, i]);
                } else {
                    indices.extend_from_slice(&[i - 1, i - 2, i]);
                }
            }
        }
        0x90 => {
            if !count.is_multiple_of(3) {
                return Err("Incomplete triangle primitive".into());
            }
            indices.extend(0..count as u32);
        }
        0x80 => {
            if !count.is_multiple_of(4) {
                return Err("Incomplete quad primitive".into());
            }
            for i in (0..count as u32).step_by(4) {
                indices.extend_from_slice(&[i, i + 1, i + 2, i, i + 2, i + 3]);
            }
        }
        0xa0 => {
            for i in 2..count as u32 {
                indices.extend_from_slice(&[0, i - 1, i]);
            }
        }
        _ => return Err("Unsupported primitive".into()),
    }
    Ok(indices)
}
fn node(data: &[u8], extra: usize) -> Result<Node> {
    let mut c = Cursor::new(data);
    let name = c.string()?;
    let header = c.take(56)?;
    let material_count = u32::from_le_bytes(header[8..12].try_into().unwrap()) as usize;
    let mut transform = [0.0; 11];
    for (i, f) in transform.iter_mut().enumerate() {
        *f = f32::from_le_bytes(header[12 + i * 4..16 + i * 4].try_into().unwrap());
        if !f.is_finite() {
            return Err("Nonfinite model transform".into());
        }
    }
    c.take(extra * 4)?;
    let sections = chunks(&data[c.pos..])?;
    let mut out = Node {
        name,
        authored_kind: u16::from_le_bytes(header[0..2].try_into().unwrap()),
        transform,
        parent: None,
        scale: 1.0,
        bounds: None,
        materials: Vec::new(),
        material_blend_modes: Vec::new(),
        surfaces: Vec::new(),
        tags: Vec::new(),
        has_skin_data: false,
        inverse_bind: Vec::new(),
    };
    // Resolve display-list references after loading their corresponding attribute arrays.
    let mut position_arrays = Vec::new();
    let mut normal_arrays = Vec::new();
    let mut uv_arrays = Vec::new();
    let mut draws = Vec::new();
    let mut descriptor = [0; 2];
    let mut lod = 0;
    for section in &sections {
        out.tags.push(String::from_utf8_lossy(&section.tag).into());
        let p = section.payload;
        match &section.tag {
            b"XOBB" => {
                if p.len() != 24 || out.bounds.is_some() {
                    return Err("Invalid/duplicate model bounds".into());
                }
                let values: [f32; 6] = std::array::from_fn(|i| {
                    f32::from_le_bytes(p[i * 4..i * 4 + 4].try_into().unwrap())
                });
                if values.iter().any(|v| !v.is_finite()) {
                    return Err("Nonfinite model bounds".into());
                }
                out.bounds = Some(Bounds {
                    min: values[..3].try_into().unwrap(),
                    max: values[3..].try_into().unwrap(),
                });
            }
            b"LCSV" => {
                if p.len() != 4 {
                    return Err("Invalid vertex scale".into());
                }
                out.scale = f32::from_le_bytes(p.try_into().unwrap());
                if !out.scale.is_finite() || out.scale <= 0.0 {
                    return Err("Nonfinite vertex scale".into());
                }
            }
            b"DONR" => {
                out.has_skin_data = true;
                let count = p.get(..4).ok_or("Truncated RNOD")?;
                let count = u32::from_le_bytes(count.try_into().unwrap()) as usize;
                if count > 10000 || p.len() != 4 + count * 64 {
                    return Err("Invalid inverse-bind table".into());
                }
                for row in p[4..].chunks_exact(64) {
                    let matrix = std::array::from_fn(|i| {
                        f32::from_le_bytes(row[i * 4..i * 4 + 4].try_into().unwrap())
                    });
                    if matrix.iter().any(|v| !v.is_finite()) {
                        return Err("Nonfinite inverse-bind matrix".into());
                    }
                    out.inverse_bind.push(matrix);
                }
            }
            b"LTAM" => {
                if p.len() != material_count * 164 {
                    return Err("Model material size mismatch".into());
                }
                for material in p.chunks_exact(164) {
                    let mut names = std::array::from_fn(|_| String::new());
                    for (i, n) in names.iter_mut().enumerate() {
                        let bytes = &material[i * 32..i * 32 + 32];
                        let end = bytes.iter().position(|b| *b == 0).unwrap_or(32);
                        *n = std::str::from_utf8(&bytes[..end])
                            .map_err(|e| e.to_string())?
                            .into();
                    }
                    let mut blend_mode = 0u16;
                    for attr in material[128..164].chunks_exact(4) {
                        let tag = u16::from_be_bytes([attr[0], attr[1]]);
                        let val = u16::from_be_bytes([attr[2], attr[3]]);
                        if tag == 0x0002 {
                            blend_mode = val;
                        }
                    }
                    out.materials.push(names);
                    out.material_blend_modes.push(blend_mode);
                }
            }
            b"SOPV" => {
                if !p.len().is_multiple_of(6) {
                    return Err("Invalid positions".into());
                }
                let points = p
                    .chunks_exact(6)
                    .map(|v| {
                        std::array::from_fn(|i| {
                            i16::from_be_bytes(v[i * 2..i * 2 + 2].try_into().unwrap()) as f32
                        })
                    })
                    .collect::<Vec<_>>();
                position_arrays.push(points);
            }
            b"MRNV" => {
                if !p.len().is_multiple_of(3) {
                    return Err("Invalid normal array".into());
                }
                normal_arrays.push(
                    p.chunks_exact(3)
                        .map(|n| {
                            let v = [n[0] as i8 as f32, n[1] as i8 as f32, n[2] as i8 as f32];
                            let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                            if len > 1e-5 {
                                [v[0] / len, v[1] / len, v[2] / len]
                            } else {
                                [0.0, 1.0, 0.0]
                            }
                        })
                        .collect::<Vec<_>>(),
                );
            }
            b"TBNV" => {
                if !p.len().is_multiple_of(36) {
                    return Err("Invalid TBNV array".into());
                }
                normal_arrays.push(
                    p.chunks_exact(36)
                        .map(|n| {
                            let v: [f32; 3] = std::array::from_fn(|i| {
                                f32::from_be_bytes(n[i * 4..i * 4 + 4].try_into().unwrap())
                            });
                            let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                            if v.iter().all(|x| x.is_finite()) && len > 1e-5 {
                                [v[0] / len, v[1] / len, v[2] / len]
                            } else {
                                [0.0, 1.0, 0.0]
                            }
                        })
                        .collect::<Vec<_>>(),
                );
            }
            b"1VUV" => {
                if !p.len().is_multiple_of(4) {
                    return Err("Invalid UV array".into());
                }
                uv_arrays.push(
                    p.chunks_exact(4)
                        .map(|v| {
                            [
                                i16::from_be_bytes(v[..2].try_into().unwrap()) as f32 / 2048.0,
                                i16::from_be_bytes(v[2..].try_into().unwrap()) as f32 / 2048.0,
                            ]
                        })
                        .collect::<Vec<_>>(),
                );
            }
            b"TNCS" => {
                if p.len() != 4 {
                    return Err("Invalid LOD marker".into());
                }
                lod = position_arrays.len().saturating_sub(1);
            }
            b"2SBX" => {
                let mut s = Cursor::new(p);
                let material = s.be32()? as usize;
                let low = s.be32()? as u64;
                let high = s.be32()? as u64;
                let mask = low | (high << 32);
                let palette = (0..64)
                    .filter(|i| mask & (1 << i) != 0)
                    .collect::<Vec<usize>>();
                let size = s.be32()? as usize;
                let bytes = s.take(size)?;
                if material >= material_count {
                    return Err("Material index out of range".into());
                }
                for (op, refs) in display_list(bytes, &mut descriptor)? {
                    draws.push((material, lod, op, refs, palette.clone()));
                }
            }
            _ => {}
        }
    }
    for (material, lod, op, refs, bone_palette) in draws {
        // Until multi-LOD sequencing is fully verified, emit only the first section group.
        if lod != 0 {
            continue;
        }
        let positions = position_arrays.first().ok_or("Draw has no positions")?;
        let normals = normal_arrays.first();
        let uvs = uv_arrays.first();
        let indices = triangles(op, refs.len())?;
        let mut vertices = Vec::new();
        for r in refs {
            let raw = positions
                .get(r.position)
                .ok_or("Position index out of range")?;
            let normal = match (r.normal, normals) {
                (Some(i), Some(n)) => Some(*n.get(i).ok_or("Normal index out of range")?),
                _ => None,
            };
            let uv = match r.uv {
                Some(i) => *uvs.and_then(|u| u.get(i)).ok_or("UV index out of range")?,
                None => [0.0, 0.0],
            };
            let position = raw.map(|v| v * out.scale);
            if position.iter().any(|v| !v.is_finite()) {
                return Err("Vertex scale overflow".into());
            }
            vertices.push(Vertex {
                position,
                normal,
                uv,
                matrix: r.matrix,
            });
        }
        out.surfaces.push(Surface {
            material,
            vertices,
            indices,
            lod,
            bone_palette,
        });
    }
    Ok(out)
}
pub fn parse(name: &str, data: &[u8]) -> Result<Model> {
    let containers = chunks(data)?;
    if containers.len() != 1 || containers[0].tag != *b"LDOM" {
        return Err("Expected one nested LDOM".into());
    }
    let body = containers[0].payload;
    let mut c = Cursor::new(body);
    let version = [c.be32()?, c.be32()?];
    if version != [101, 200] {
        return Err("Unsupported model version".into());
    }
    let node_count = c.be16()? as usize;
    let extra = c.be16()? as usize;
    let links = c.be32()? as usize;
    if node_count > 10000 || extra > 10000 || links > 10000 {
        return Err("Model count limit exceeded".into());
    }
    let mut header_sphere = [0.0; 4];
    for value in &mut header_sphere {
        *value = f32::from_bits(c.be32()?);
    }
    if header_sphere.iter().any(|v| !v.is_finite()) {
        return Err("Nonfinite model header sphere".into());
    }
    let source = c.string()?;
    c.take(extra * 4)?;
    let sections = chunks(&body[c.pos..])?;
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut have_links = false;
    for section in sections {
        match &section.tag {
            b"EDON" => nodes.push(node(section.payload, extra)?),
            b"TCNC" => {
                if have_links {
                    return Err("Duplicate hierarchy edge section".into());
                }
                have_links = true;
                if section.payload.len() != links * 4 {
                    return Err("Hierarchy edge size mismatch".into());
                }
                for e in section.payload.chunks_exact(4) {
                    edges.push((
                        u16::from_le_bytes(e[..2].try_into().unwrap()) as usize,
                        u16::from_le_bytes(e[2..].try_into().unwrap()) as usize,
                    ));
                }
            }
            _ => {}
        }
    }
    if nodes.len() != node_count {
        return Err("Node count mismatch".into());
    }
    if edges.len() != links {
        return Err("Hierarchy edge count mismatch".into());
    }
    for &(parent, child) in &edges {
        if parent >= nodes.len()
            || child >= nodes.len()
            || parent == child
            || nodes[child].parent.is_some()
        {
            return Err("Invalid hierarchy link".into());
        }
        nodes[child].parent = Some(parent);
    }
    for start in 0..nodes.len() {
        let mut current = Some(start);
        for step in 0..=nodes.len() {
            if let Some(i) = current {
                if step == nodes.len() {
                    return Err("Hierarchy cycle".into());
                }
                current = nodes[i].parent;
            } else {
                break;
            }
        }
    }
    Ok(Model {
        name: name.into(),
        version,
        source,
        header_sphere,
        nodes,
        hierarchy_links: edges,
    })
}
pub fn from_archive(data: &[u8], wanted: &str) -> Result<Model> {
    for chunk in chunks(data)? {
        if chunk.tag == *b"LDOM" {
            let (name, body) = named_payload(chunk.payload)?;
            if name.eq_ignore_ascii_case(wanted) {
                return parse(name, body);
            }
        }
    }
    Err(format!("Model missing: {wanted}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn chunk(tag: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut b = tag.to_vec();
        b.extend((payload.len() as u32).to_le_bytes());
        b.extend(payload);
        b
    }
    #[test]
    fn authored_node_kind_is_little_endian_and_not_inferred_from_name() {
        let mut bytes = 4u32.to_be_bytes().to_vec();
        bytes.extend_from_slice(b"ROOT");
        let mut header = [0u8; 56];
        header[..2].copy_from_slice(&0x1234u16.to_le_bytes());
        bytes.extend_from_slice(&header);
        assert_eq!(node(&bytes, 0).unwrap().authored_kind, 0x1234);
    }
    #[test]
    fn header_sphere_is_big_endian_separate_from_node_bounds() {
        let mut bytes = fixture();
        let expected = [1.25_f32, -2.5, 3.75, 9.0];
        // Outer chunk header (8), then version/count/link fields (16).
        for (i, value) in expected.iter().enumerate() {
            bytes[24 + i * 4..28 + i * 4].copy_from_slice(&value.to_be_bytes());
        }
        let model = parse("fixture", &bytes).unwrap();
        assert_eq!(model.header_sphere, expected);
        assert!(model.nodes[0].bounds.is_none());
        for bad in [f32::INFINITY, f32::NAN] {
            bytes[24..28].copy_from_slice(&bad.to_be_bytes());
            assert!(
                parse("fixture", &bytes)
                    .unwrap_err()
                    .contains("header sphere")
            );
        }
    }

    fn fixture() -> Vec<u8> {
        let mut node = 4u32.to_be_bytes().to_vec();
        node.extend(b"root");
        node.extend([0; 8]);
        node.extend(1u32.to_le_bytes());
        for f in [0.0_f32, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0] {
            node.extend(f.to_le_bytes());
        }
        node.extend(chunk(b"LCSV", &0.001_f32.to_le_bytes()));
        node.extend(chunk(b"LTAM", &[0; 164]));
        let positions = [0i16, 0, 0, 1000, 0, 0, 0, 1000, 0]
            .into_iter()
            .flat_map(i16::to_be_bytes)
            .collect::<Vec<_>>();
        node.extend(chunk(b"SOPV", &positions));
        let gx = [8, 0x50, 0, 0, 4, 0, 0x90, 0, 3, 0, 1, 2];
        let mut draw = vec![0; 12];
        draw.extend((gx.len() as u32).to_be_bytes());
        draw.extend(gx);
        node.extend(chunk(b"2SBX", &draw));
        let mut root = 101u32.to_be_bytes().to_vec();
        root.extend(200u32.to_be_bytes());
        root.extend(1u16.to_be_bytes());
        root.extend(0u16.to_be_bytes());
        root.extend([0; 20]);
        root.extend(7u32.to_be_bytes());
        root.extend(b"toy.bgf");
        root.extend(chunk(b"MEMX", &[]));
        root.extend(chunk(b"EDON", &node));
        root.extend(chunk(b"TCNC", &[]));
        chunk(b"LDOM", &root)
    }
    #[test]
    fn authored_bounds_are_exact_bounded_metadata() {
        let mut bytes = 1u32.to_be_bytes().to_vec();
        bytes.push(b'x');
        let mut header = [0; 56];
        header[36..40].copy_from_slice(&1.0f32.to_le_bytes());
        bytes.extend(header);
        let values = [-2.0f32, -1.0, -3.0, 2.0, 1.0, 3.0];
        let bounds = values
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect::<Vec<_>>();
        let mut valid = bytes.clone();
        valid.extend(chunk(b"XOBB", &bounds));
        let decoded = node(&valid, 0).unwrap().bounds.unwrap();
        assert_eq!(decoded.min, [-2.0, -1.0, -3.0]);
        assert_eq!(decoded.max, [2.0, 1.0, 3.0]);
        valid.extend(chunk(b"XOBB", &bounds));
        assert!(node(&valid, 0).is_err());
        for size in [0, 20, 23] {
            let mut invalid = bytes.clone();
            invalid.extend(chunk(b"XOBB", &bounds[..size]));
            assert!(node(&invalid, 0).is_err());
        }
        let mut nonfinite = bounds;
        nonfinite[..4].copy_from_slice(&f32::NAN.to_le_bytes());
        bytes.extend(chunk(b"XOBB", &nonfinite));
        assert!(node(&bytes, 0).is_err());
    }
    #[test]
    fn hierarchy_section_count_and_original_order_are_preserved() {
        let bytes = fixture();
        let mut body = chunks(&bytes).unwrap()[0].payload.to_vec();
        body.extend(chunk(b"TCNC", &[]));
        assert_eq!(
            parse("duplicate", &chunk(b"LDOM", &body)).unwrap_err(),
            "Duplicate hierarchy edge section"
        );
        let mut body = chunks(&bytes).unwrap()[0].payload.to_vec();
        body.truncate(body.len() - 8);
        body[12..16].copy_from_slice(&1u32.to_be_bytes());
        assert_eq!(
            parse("missing", &chunk(b"LDOM", &body)).unwrap_err(),
            "Hierarchy edge count mismatch"
        );
        let body = chunks(&bytes).unwrap()[0].payload;
        let sections = chunks(&body[43..]).unwrap();
        let node = sections.iter().find(|s| s.tag == *b"EDON").unwrap().payload;
        let mut linked = body[..43].to_vec();
        linked[8..10].copy_from_slice(&3u16.to_be_bytes());
        linked[12..16].copy_from_slice(&2u32.to_be_bytes());
        for _ in 0..3 {
            linked.extend(chunk(b"EDON", node));
        }
        let edges = [0u16, 2, 0, 1]
            .into_iter()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        linked.extend(chunk(b"TCNC", &edges));
        let parsed = parse("ordered", &chunk(b"LDOM", &linked)).unwrap();
        assert_eq!(parsed.hierarchy_links, vec![(0, 2), (0, 1)]);
        assert_eq!(parsed.nodes[1].parent, Some(0));
        assert_eq!(parsed.nodes[2].parent, Some(0));
    }
    #[test]
    fn independent_triangle_fixture() {
        let m = parse("toy", &fixture()).unwrap();
        assert_eq!(m.nodes[0].surfaces[0].indices, [0, 1, 2]);
        assert_eq!(m.nodes[0].surfaces[0].vertices[1].position, [1.0, 0.0, 0.0]);
    }
    #[test]
    fn every_truncated_fixture_is_rejected() {
        let b = fixture();
        for end in 0..b.len() {
            assert!(
                parse("toy", &b[..end]).is_err(),
                "accepted truncation {end}"
            );
        }
    }
    #[test]
    fn strip_winding() {
        assert_eq!(triangles(0x98, 5).unwrap(), [0, 1, 2, 2, 1, 3, 2, 3, 4]);
    }
    #[test]
    fn malformed_primitive() {
        assert!(triangles(0x90, 4).is_err());
        assert!(triangles(0x80, 5).is_err());
    }
    #[test]
    fn gx_descriptor_indices() {
        let bytes = [
            8, 0x50, 0, 0, 6, 0, 8, 0x60, 0, 0, 0, 3, 0x98, 0, 3, 0, 0, 0, 1, 0, 1, 0, 2, 0, 2, 0,
            3,
        ];
        let draws = display_list(&bytes, &mut [0, 0]).unwrap();
        assert_eq!(draws[0].1[2].position, 2);
        assert_eq!(draws[0].1[2].uv, Some(3));
    }
    #[test]
    fn rejects_truncated_command() {
        assert!(display_list(&[8, 0x50, 0], &mut [0, 0]).is_err());
    }
}
