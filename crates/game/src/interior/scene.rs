//! Retained presentation compiled from document identities, not array positions.
use super::*;
use std::collections::{BTreeMap, BTreeSet};
use universe_engine::scene::{ObjectId, Scene};

#[derive(Default)]
pub(super) struct AuthoringScene {
    pub scene: Scene,
    modules: BTreeMap<Id, (Block, String, crate::equipment_visual::Placed)>,
}
impl AuthoringScene {
    pub fn sync(&mut self, interior: &Interior) {
        let live: BTreeSet<_> = interior.plan.blocks.iter().map(|b| b.uid).collect();
        self.modules.retain(|id, _| {
            let keep = live.contains(id);
            if !keep {
                self.scene.remove(ObjectId(id.0));
            }
            keep
        });
        for block in &interior.plan.blocks {
            let fit = interior.fit.iter().find(|f| f.id == kind(&block.id));
            let key = fit.map_or("", |f| f.key.as_str());
            let changed = self
                .modules
                .get(&block.uid)
                .is_none_or(|(old, old_key, _)| old != block || old_key != key);
            if changed {
                self.modules.insert(
                    block.uid,
                    (block.clone(), key.into(), placed_module(block, fit)),
                );
            }
            let visual = &self.modules[&block.uid].2;
            let id = ObjectId(block.uid.0);
            visual
                .set_node(&mut self.scene, &id, None)
                .expect("valid document placement");
            if let Some(node) = self.scene.get(id) {
                let visible = interior.shown(layer::MODULES);
                if node.visible != visible {
                    let mut node = node.clone();
                    node.visible = visible;
                    self.scene.set(id, node).expect("valid visibility change");
                }
            }
        }
    }
}

/// A catalogue camera shares renderer/cache resources but keeps its graph and depth.
#[derive(Default)]
pub(super) struct CatalogueScene {
    pub scene: Scene,
    item: Option<(Block, String, crate::equipment_visual::Placed)>,
}
impl CatalogueScene {
    pub fn sync(&mut self, block: &Block, fit: &Fitted) {
        if self
            .item
            .as_ref()
            .is_none_or(|(old, key, _)| old != block || key != &fit.key)
        {
            self.item = Some((
                block.clone(),
                fit.key.clone(),
                placed_module(block, Some(fit)),
            ));
        }
        self.item
            .as_ref()
            .unwrap()
            .2
            .set_node(&mut self.scene, &ObjectId(1), None)
            .expect("catalogue placement");
    }
}
