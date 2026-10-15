//! Retained presentation compiled from document identities, not array positions.
use super::*;
use std::collections::{BTreeMap, BTreeSet};
use universe_engine::scene::{ObjectId, Scene};

#[derive(Default)]
pub(super) struct AuthoringScene {
    pub scene: Scene,
    modules: BTreeMap<Id, (Block, String, crate::equipment_visual::Placed)>,
    /// The frame (members and nodes) and the decks as drawn, and what they were
    /// drawn from.
    frame: Option<(Vec<Beam>, Nodes, Drawn)>,
    decks: Option<(Vec<Plate>, Drawn)>,
}

type Drawn = Option<crate::equipment_visual::Placed>;
/// The frame's nodes: where, and of which forging.
type Nodes = Vec<(Vec3, String)>;

/// The scene's frame and decks (no module's uid is so high).
const FRAME_ID: ObjectId = ObjectId(u64::MAX - 1);
const DECKS_ID: ObjectId = ObjectId(u64::MAX - 2);

/// Puts `placed` in the scene as `id` (or takes it out), shown or not.
fn show(scene: &mut Scene, id: ObjectId, placed: Option<&crate::equipment_visual::Placed>, visible: bool) {
    let Some(p) = placed else {
        scene.remove(id);
        return;
    };
    p.set_node(scene, &id, None).expect("valid frame placement");
    if let Some(node) = scene.get(id).filter(|n| n.visible != visible) {
        let mut node = node.clone();
        node.visible = visible;
        scene.set(id, node).expect("valid visibility change");
    }
}

impl AuthoringScene {
    /// The frame and decks, from their stock (rebuilt only when they change).
    fn sync_frame(&mut self, interior: &Interior) {
        use crate::frame_visual as fv;
        let plan = &interior.plan;
        let nodes: Nodes = interior.node_cache.as_ref().filter(|(p, _)| p.beams == plan.beams).map_or(Vec::new(), |(_, ns)| {
            ns.iter().filter_map(|(at, _, n)| n.map(|n| (*at, n.key.clone()))).collect()
        });
        if self.frame.as_ref().is_none_or(|(b, n, _)| *b != plan.beams || *n != nodes) {
            let members: Vec<fv::Member> = plan.beams.iter().map(|b| fv::Member { a: b.a, b: b.b, stock: &b.stock, roll: 0.0 }).collect();
            let ns: Vec<fv::Node> = nodes.iter().map(|(at, k)| fv::Node { at: *at, stock: k }).collect();
            let placed = fv::placed(&members, &ns, &[]);
            self.frame = Some((plan.beams.clone(), nodes, placed));
        }
        if self.decks.as_ref().is_none_or(|(p, _)| *p != plan.plates) {
            let decks: Vec<fv::Deck> = plan.plates.iter().map(|p| fv::Deck { lo: p.lo, hi: p.hi, top: p.y, stock: &p.stock }).collect();
            self.decks = Some((plan.plates.clone(), fv::placed(&[], &[], &decks)));
        }
        show(&mut self.scene, FRAME_ID, self.frame.as_ref().and_then(|f| f.2.as_ref()), interior.shown(layer::FRAME));
        show(&mut self.scene, DECKS_ID, self.decks.as_ref().and_then(|d| d.1.as_ref()), interior.shown(layer::DECKS));
    }

    pub fn sync(&mut self, interior: &Interior) {
        self.sync_frame(interior);
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
