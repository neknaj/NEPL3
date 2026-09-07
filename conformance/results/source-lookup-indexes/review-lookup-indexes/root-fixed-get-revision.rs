    pub fn get_revision_with_budget(
        &self,
        source: &SourceId,
        revision: u64,
        budget: &mut Budget,
    ) -> Result<Option<&SourceSnapshot>, StopReason> {
        budget.poll()?;
        let (mut low, mut high) = (0, self.index.len());
        while low < high {
            let mid = low + (high - low) / 2;
            let snapshot = &self.snapshots[self.index[mid]];
            budget.charge(
                Resource::Work,
                (snapshot.id.source.0.len() as u64)
                    .saturating_add(source.0.len() as u64)
                    .saturating_add(1),
            )?;
            match snapshot
                .id
                .source
                .cmp(source)
                .then_with(|| snapshot.id.revision.cmp(&revision))
            {
                core::cmp::Ordering::Equal => return Ok(Some(snapshot)),
                core::cmp::Ordering::Less => low = mid + 1,
                core::cmp::Ordering::Greater => high = mid,
            }
        }
        Ok(None)
    }
