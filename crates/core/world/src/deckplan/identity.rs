//! Persistent authoring identity; independent of array positions and geometry.
use serde::{Deserialize, Deserializer, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(1);
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct Id(pub u64);
impl Default for Id {
    fn default() -> Self {
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}
impl<'de> Deserialize<'de> for Id {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let n = u64::deserialize(d)?;
        if n == 0 {
            return Ok(Self::default());
        }
        if n == u64::MAX {
            return Err(serde::de::Error::custom("authoring identity exhausted"));
        }
        NEXT.fetch_max(n + 1, Ordering::Relaxed);
        Ok(Self(n))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(from = "FloorRepr")]
pub struct Floor {
    pub outline: Vec<glam::DVec2>,
    pub uid: Id,
}
#[derive(Deserialize)]
#[serde(untagged)]
enum FloorRepr {
    Named {
        outline: Vec<glam::DVec2>,
        #[serde(default)]
        uid: Id,
    },
    Legacy(Vec<glam::DVec2>),
}
impl From<FloorRepr> for Floor {
    fn from(v: FloorRepr) -> Self {
        match v {
            FloorRepr::Named { outline, uid } => Self { outline, uid },
            FloorRepr::Legacy(outline) => outline.into(),
        }
    }
}
impl From<Vec<glam::DVec2>> for Floor {
    fn from(outline: Vec<glam::DVec2>) -> Self {
        Self {
            outline,
            uid: Id::default(),
        }
    }
}
impl std::ops::Deref for Floor {
    type Target = Vec<glam::DVec2>;
    fn deref(&self) -> &Self::Target {
        &self.outline
    }
}
impl std::ops::DerefMut for Floor {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.outline
    }
}
impl<'a> IntoIterator for &'a Floor {
    type Item = &'a glam::DVec2;
    type IntoIter = std::slice::Iter<'a, glam::DVec2>;
    fn into_iter(self) -> Self::IntoIter {
        self.outline.iter()
    }
}
impl<'a> IntoIterator for &'a mut Floor {
    type Item = &'a mut glam::DVec2;
    type IntoIter = std::slice::IterMut<'a, glam::DVec2>;
    fn into_iter(self) -> Self::IntoIter {
        self.outline.iter_mut()
    }
}
