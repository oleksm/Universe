//! A revision-bound preparation worker. Only the finished owned draft is published;
//! the editor thread keeps processing close/cancel while geometry is compiled.
use super::*;
#[derive(Clone, Copy)]
pub(super) enum Mode {
    Balance(bool),
    Drive(Option<f64>),
}
pub(super) struct Pending {
    plan: Plan,
    decks: Option<universe_sim::world::deckplan::DeckPlan>,
    mode: Mode,
    receiver: mpsc::Receiver<crate::test_drive::Craft>,
}
impl Interior {
    pub(super) fn prepare(&mut self, mode: Mode) {
        if self.preparation.is_some() {
            return;
        }
        let plan = self.plan.clone();
        let decks = self.decks_now.clone();
        let fit = self.fit.clone();
        let (sender, receiver) = mpsc::channel();
        self.preparation = Some(Pending {
            plan: plan.clone(),
            decks: decks.clone(),
            mode,
            receiver,
        });
        job("studio-prepare", move || {
            let _scope = universe_prof::scope("studio/prepare");
            let draft = Interior {
                plan,
                fit,
                decks_now: decks,
                ..Interior::default()
            };
            let _ = sender.send(craft(&draft));
        });
    }
    pub(super) fn poll_preparation(&mut self) {
        let Some(pending) = &self.preparation else {
            return;
        };
        if pending.plan != self.plan || pending.decks != self.decks_now {
            self.preparation = None;
            return;
        }
        match pending.receiver.try_recv() {
            Ok(craft) => {
                let mode = pending.mode;
                self.preparation = None;
                match mode {
                    Mode::Balance(trimmed) => {
                        let mut balance = crate::test_drive::Balance::new(craft);
                        balance.set_trim(trimmed);
                        self.balance = Some(balance);
                    }
                    Mode::Drive(collective) => {
                        let mut drive = crate::test_drive::Drive::new(craft);
                        if let Some(c) = collective {
                            drive.set_assist(!crate::devenv::flag("UNIVERSE_DRIVE_MANUAL"));
                            drive.set_throttle(c);
                        }
                        self.drive = Some(drive);
                    }
                }
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.preparation = None;
                self.message = Some(("DESIGN PREPARATION FAILED - SEE LOG".into(), 10.));
            }
        }
    }
}
