//! Shrinking a time zone name down its constraint's rank ladder
//! (`hegel-c/docs/datetime.md` §6.1). The string passes leave such a
//! string alone, since no edit of its characters is another name; this
//! pass tries the lowest-ranked allowed name and then walks down the
//! ladder from the current name, accepting the first replacement that
//! keeps the failure and walking on from there. The node's sort key is its
//! ladder position, so every accepted step is a strict improvement.

use super::{PassExit, ShrinkResult, Shrinker, absorb_node_gone};
use crate::native::HashMap;
use crate::native::core::ChoiceValue;
use crate::native::draws::timezone::TimeZoneConstraint;
use alloc::sync::Arc;

impl<'a> Shrinker<'a> {
    pub(super) async fn shrink_timezones(&mut self) -> ShrinkResult<()> {
        let mut i = 0;
        while i < self.current_nodes.len() {
            absorb_node_gone(self.shrink_timezone_node(i).await)?;
            i += 1;
        }
        Ok(())
    }

    /// The constraint and ladder position of node `i`, or `None` when it
    /// is not (or no longer) a string under a time zone set.
    fn timezone_at(&self, i: usize) -> Option<(Arc<TimeZoneConstraint>, usize)> {
        let (sc, v) = self.current_nodes.get(i)?.data.as_string()?;
        let zones = Arc::clone(sc.constraint.as_ref()?.timezone()?);
        let position = zones.position(v)?;
        Some((zones, position))
    }

    async fn shrink_timezone_node(&mut self, i: usize) -> Result<(), PassExit> {
        let (zones, position) = self.timezone_at(i).ok_or(PassExit::NodeGone)?;
        if position == 0 {
            return Ok(());
        }
        if self.replace_timezone(i, &zones, 0).await? {
            return Ok(());
        }
        for candidate in (1..position).rev() {
            self.replace_timezone(i, &zones, candidate).await?;
        }
        Ok(())
    }

    async fn replace_timezone(
        &mut self,
        i: usize,
        zones: &TimeZoneConstraint,
        position: usize,
    ) -> Result<bool, PassExit> {
        let value = ChoiceValue::String(zones.name_at(position).to_vec());
        Ok(self.replace(&HashMap::from_iter([(i, value)])).await?)
    }
}

#[cfg(test)]
#[path = "../../../tests/embedded/native/shrinker_timezone_tests.rs"]
mod tests;
