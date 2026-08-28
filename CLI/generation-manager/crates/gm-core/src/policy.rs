use crate::{error::{Error,
                    Result},
            generation::GenerationId};

pub fn select_worktree(explicit: Option<&str>, current: Option<&str>) -> Option<String> { explicit.or(current).map(str::to_string) }

pub fn next_generation_id(existing: &[GenerationId]) -> GenerationId { existing.iter().max().copied().map(|id| id.next()).unwrap_or(GenerationId(1)) }

pub fn rollback_target(current: Option<GenerationId>, existing: &[GenerationId]) -> Result<GenerationId> {
    let current = current.ok_or(Error::NoCurrentGeneration)?;
    existing.iter().copied().filter(|id| *id < current).max().ok_or(Error::NoPreviousGeneration)
}

pub fn gc_candidates(existing: &[GenerationId], current: Option<GenerationId>, keep: usize) -> Vec<GenerationId> {
    let keep = keep.max(1);
    let mut ordered = existing.to_vec();
    ordered.sort();
    if ordered.len() <= keep {
        return Vec::new();
    }
    let rollback = rollback_target(current, &ordered).ok();
    ordered[.. ordered.len() - keep]
        .iter()
        .copied()
        .filter(|id| Some(*id) != current && Some(*id) != rollback)
        .collect()
}

pub fn activation_restore_target(previous: Option<GenerationId>, failed: GenerationId) -> Option<GenerationId> {
    previous.filter(|previous| *previous != failed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(values: &[u64]) -> Vec<GenerationId> { values.iter().copied().map(GenerationId).collect() }

    #[test]
    fn policies_choose_generation_numbers_and_rollback_targets() {
        let existing = ids(&[1, 3, 2]);
        assert_eq!(next_generation_id(&existing), GenerationId(4));
        assert_eq!(rollback_target(Some(GenerationId(3)), &existing).unwrap(), GenerationId(2));
    }

    #[test]
    fn gc_preserves_the_active_generation_and_rollback_target() {
        assert_eq!(gc_candidates(&ids(&[1, 2, 3, 4]), Some(GenerationId(4)), 2), ids(&[1, 2]));
        assert_eq!(gc_candidates(&ids(&[1, 2, 3, 4]), Some(GenerationId(2)), 1), ids(&[3]));
    }

    #[test]
    fn failed_activation_only_restores_a_different_previous_generation() {
        assert_eq!(activation_restore_target(Some(GenerationId(1)), GenerationId(2)), Some(GenerationId(1)));
        assert_eq!(activation_restore_target(Some(GenerationId(2)), GenerationId(2)), None);
    }
}
