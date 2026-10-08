use memedock_core::sources::telegram::{TelegramImportOutcome, TelegramImportReport};
use memedock_domain::source::{SourceImportState, SourceItemId};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Default)]
pub struct Selection {
    pub selected: BTreeSet<SourceItemId>,
    pub states: BTreeMap<SourceItemId, SourceImportState>,
    pub busy: bool,
}
impl Selection {
    pub fn toggle(&mut self, id: SourceItemId, selected: bool) {
        if self.busy || self.states.get(&id) == Some(&SourceImportState::RestoreRequired) {
            return;
        }
        if selected {
            self.selected.insert(id);
        } else {
            self.selected.remove(&id);
        }
    }
    pub fn finish(&mut self, report: &TelegramImportReport) {
        for item in &report.items {
            match item.outcome {
                TelegramImportOutcome::Created | TelegramImportOutcome::Reused => {
                    self.states
                        .insert(item.id.clone(), SourceImportState::Imported);
                    self.selected.remove(&item.id);
                }
                TelegramImportOutcome::RestoreRequired => {
                    self.states
                        .insert(item.id.clone(), SourceImportState::RestoreRequired);
                    self.selected.remove(&item.id);
                }
                _ => (),
            }
        }
        self.busy = false;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn partial_results_keep_only_failed_and_pending_selections()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut state = Selection::default();
        let a = SourceItemId::new("a".into())?;
        let b = SourceItemId::new("b".into())?;
        state.toggle(a.clone(), true);
        state.toggle(b.clone(), true);
        state.busy = true;
        state.toggle(a.clone(), false);
        assert_eq!(state.selected.len(), 2);
        state.finish(&TelegramImportReport {
            stopped: true,
            items: vec![
                memedock_core::sources::telegram::TelegramImportItem {
                    id: a.clone(),
                    sticker: None,
                    outcome: TelegramImportOutcome::Created,
                },
                memedock_core::sources::telegram::TelegramImportItem {
                    id: b.clone(),
                    sticker: None,
                    outcome: TelegramImportOutcome::Pending,
                },
            ],
        });
        assert_eq!(state.selected, BTreeSet::from([b]));
        assert_eq!(state.states[&a], SourceImportState::Imported);
        Ok(())
    }
}
