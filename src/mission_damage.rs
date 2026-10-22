//! Rust radial damage policy using authored channels and target resistance tables.
//! Linear 3D falloff; no native blast occlusion, cover or impulse parity claimed.
use crate::{Object, ObjectId, Result};
#[derive(Clone, Debug)]
pub struct Blast {
    pub owner: ObjectId,
    pub position: [f32; 3],
    pub inner: f64,
    pub outer: f64,
    pub channels: Vec<(String, f64)>,
}
fn values<'a>(o: &'a Object, key: &str) -> Result<&'a [String]> {
    let fields: Vec<_> = o
        .fields
        .iter()
        .filter(|f| f.attributes.get("name").is_some_and(|v| v == key))
        .collect();
    let [f] = fields.as_slice() else {
        return Err(format!("Missing/ambiguous damage field {key}"));
    };
    if f.items.is_empty() {
        return Err(format!("Empty damage field {key}"));
    }
    Ok(&f.items)
}
fn number(s: &str) -> Result<f64> {
    let n = s.parse::<f64>().map_err(|_| "Invalid damage number")?;
    if !n.is_finite() || n < 0.0 {
        return Err("Invalid damage range".into());
    }
    Ok(n)
}
fn scalar(o: &Object, key: &str) -> Result<f64> {
    let [s] = values(o, key)? else {
        return Err(format!("Invalid damage scalar {key}"));
    };
    number(s)
}
impl Blast {
    pub fn from_explosion(o: &Object, owner: ObjectId, position: [f32; 3]) -> Result<Self> {
        let types = values(o, "mDamageType")?;
        let amounts = values(o, "mDamageAmount")?;
        if types.len() != amounts.len()
            || types.len() != 4
            || position.iter().any(|v| !v.is_finite())
        {
            return Err("Invalid blast channels/position".into());
        }
        let mut channels = Vec::new();
        for (kind, amount) in types.iter().zip(amounts) {
            let amount = number(amount)?;
            if !kind.starts_with("DAMAGE_") {
                return Err("Invalid blast damage type".into());
            }
            if kind != "DAMAGE_NO_DAMAGE" && amount > 0.0 {
                channels.push((kind.clone(), amount));
            }
        }
        let inner = scalar(o, "mMinBlastRadius")?;
        let outer = scalar(o, "mMaxBlastRadius")?;
        if outer < inner {
            return Err("Invalid blast radii".into());
        }
        Ok(Self {
            owner,
            position,
            inner,
            outer,
            channels,
        })
    }
    pub fn attenuation(&self, distance: f64) -> f64 {
        if self.outer <= self.inner {
            return if distance <= self.outer { 1.0 } else { 0.0 };
        }
        ((self.outer - distance) / (self.outer - self.inner)).clamp(0.0, 1.0)
    }
    pub fn damage_to(&self, base: &Object, destroyable: bool) -> Result<f64> {
        let types = values(
            base,
            if destroyable {
                "DamageType"
            } else {
                "mDamageType"
            },
        )?;
        let factors = values(
            base,
            if destroyable {
                "DamageFactor"
            } else {
                "mDamageFactor"
            },
        )?;
        if types.len() != factors.len() {
            return Err("Mismatched resistance table".into());
        }
        let factors = factors
            .iter()
            .map(|v| number(v))
            .collect::<Result<Vec<_>>>()?;
        let default = scalar(base, "mDefaultDamageFactor")?;
        let damage = self
            .channels
            .iter()
            .map(|(kind, amount)| {
                amount
                    * types
                        .iter()
                        .position(|k| k == kind)
                        .map(|i| factors[i])
                        .unwrap_or(default)
            })
            .sum::<f64>();
        if !damage.is_finite() {
            return Err("Damage overflow".into());
        }
        Ok(damage)
    }
}
