//! Optional upper-bound reservation spanning CPU handles, GPU cache and retired submissions.
use crate::{Mesh, WireModel};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
#[derive(Debug)]
struct State {
    limit: usize,
    live: AtomicUsize,
    peak: AtomicUsize,
}
#[derive(Clone, Debug)]
pub struct MeshBudget(Arc<State>);
#[derive(Debug)]
pub(crate) struct Reservation {
    state: Arc<State>,
    bytes: usize,
}
impl Drop for Reservation {
    fn drop(&mut self) {
        self.state.live.fetch_sub(self.bytes, Ordering::AcqRel);
    }
}
impl MeshBudget {
    pub fn new(limit: usize) -> Self {
        Self(Arc::new(State {
            limit,
            live: AtomicUsize::new(0),
            peak: AtomicUsize::new(0),
        }))
    }
    pub fn live_bytes(&self) -> usize {
        self.0.live.load(Ordering::Acquire)
    }
    pub fn peak_bytes(&self) -> usize {
        self.0.peak.load(Ordering::Acquire)
    }
    /// Reserve a complete group before any member is published. Rejection returns all CPU models.
    pub fn group(&self, models: Vec<WireModel>) -> Result<Vec<Mesh>, Vec<WireModel>> {
        let Some(bytes) = models
            .iter()
            .try_fold(0usize, |sum, m| sum.checked_add(mesh_bytes(m)?))
        else {
            return Err(models);
        };
        let Ok(previous) = self.0.live.try_update(Ordering::AcqRel, Ordering::Acquire, |n| {
            n.checked_add(bytes).filter(|&n| n <= self.0.limit)
        }) else { return Err(models); };
        // Capture the reservation's linearization point, even if another group drops now.
        self.0.peak.fetch_max(previous + bytes, Ordering::Relaxed);
        let reservation = Arc::new(Reservation {
            state: self.0.clone(),
            bytes,
        });
        Ok(models
            .into_iter()
            .map(|model| {
                let mut m = Mesh::new(model);
                m.reservation = Some(reservation.clone());
                m
            })
            .collect())
    }
}
/// GpuMesh uses unindexed 56-byte vertices and a 40-byte placeholder for each empty buffer.
pub fn mesh_bytes(m: &WireModel) -> Option<usize> {
    m.faces
        .len()
        .checked_mul(3)?
        .checked_mul(56)?
        .max(40)
        .checked_add(m.edges.len().checked_mul(2)?.checked_mul(56)?.max(40))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn complete_group_reservation_survives_handles_and_refuses_overcommit() {
        let budget = MeshBudget::new(160);
        let meshes = budget
            .group(vec![WireModel::default(), WireModel::default()])
            .unwrap();
        assert_eq!(budget.live_bytes(), 160);
        assert!(budget.group(vec![WireModel::default()]).is_err());
        let retired = meshes[0].reservation.clone();
        drop(meshes);
        assert_eq!(budget.live_bytes(), 160);
        drop(retired);
        assert_eq!(budget.live_bytes(), 0);
        assert_eq!(budget.peak_bytes(), 160);
    }
}
