//! Bottom-up directory aggregation.
//!
//! Computing folder sizes is the one part of scanning that is inherently
//! *not* embarrassingly parallel: a folder's total is only final once every
//! descendant has been seen, and the traversal visits descendants on several
//! threads in no particular order.
//!
//! The approach here is a completion count per directory. Each node records how
//! many child directories it is waiting for; when a node's own listing has
//! arrived and all of its child directories have finalised, its totals are
//! final and roll up into its parent. Rolling up may finalise the parent, so
//! the cascade is driven by an explicit stack rather than recursion, which
//! keeps a 300-level-deep tree off the call stack.
//!
//! Memory is the reason this design was chosen over buffering directory rows
//! until they are complete: a node holds counters only, no strings. That is
//! about 56 bytes per *directory*, independent of how many files each contains,
//! so a volume with ten million files and two hundred thousand folders costs
//! roughly 11 MB to aggregate.

use crate::db::writer::DirAggregate;

/// Index into the aggregator's dense node array. Allocated by the walker as it
/// discovers directories.
pub type DirIndex = u32;

/// Sentinel for "no parent", avoiding an `Option` inside the hot node struct.
const NO_PARENT: DirIndex = DirIndex::MAX;

#[derive(Debug, Clone, Copy)]
struct DirNode {
    entry_id: i64,
    parent: DirIndex,
    /// Child directories discovered in this directory's listing.
    expected_children: u32,
    /// Child directories that have finalised.
    completed_children: u32,
    /// True once this directory's own listing has been reported.
    listed: bool,
    /// True once the totals have rolled into the parent.
    finalised: bool,
    logical: i64,
    allocated: i64,
    files: i64,
    dirs: i64,
}

impl Default for DirNode {
    fn default() -> Self {
        Self {
            entry_id: 0,
            parent: NO_PARENT,
            expected_children: 0,
            completed_children: 0,
            listed: false,
            finalised: false,
            logical: 0,
            allocated: 0,
            files: 0,
            dirs: 0,
        }
    }
}

/// Totals for one directory's own listing, as reported by a walker thread.
#[derive(Debug, Clone, Copy, Default)]
pub struct DirListing {
    pub index: DirIndex,
    pub entry_id: i64,
    pub parent: Option<DirIndex>,
    /// Bytes of the non-directory children, excluding hard-link repeats.
    pub own_logical: i64,
    pub own_allocated: i64,
    /// Non-directory children counted towards the file total.
    pub own_files: i64,
    /// Child directories this listing produced.
    pub child_dirs: u32,
}

/// Accumulates directory totals for one scan.
///
/// Driven by a single thread (the index writer), so nodes need no atomics or
/// locks. Contended atomics across eight walker threads measured worse than
/// letting the writer, which is not the bottleneck, do the arithmetic.
#[derive(Debug, Default)]
pub struct DirAggregator {
    nodes: Vec<DirNode>,
    /// Reusable scratch for the finalisation cascade.
    cascade: Vec<DirIndex>,
    finalised_count: usize,
}

impl DirAggregator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn finalised_count(&self) -> usize {
        self.finalised_count
    }

    /// Approximate heap use, reported by the benchmark harness.
    pub fn memory_bytes(&self) -> usize {
        self.nodes.capacity() * std::mem::size_of::<DirNode>()
    }

    fn ensure(&mut self, index: DirIndex) {
        let needed = index as usize + 1;
        if self.nodes.len() < needed {
            self.nodes.resize(needed, DirNode::default());
        }
    }

    /// Record a directory's own listing. Listings may arrive in any order, and
    /// a child's listing may arrive before its parent's.
    pub fn record_listing(&mut self, listing: DirListing) {
        self.ensure(listing.index);
        if let Some(parent) = listing.parent {
            self.ensure(parent);
        }
        {
            let node = &mut self.nodes[listing.index as usize];
            node.entry_id = listing.entry_id;
            node.parent = listing.parent.unwrap_or(NO_PARENT);
            node.expected_children = listing.child_dirs;
            node.listed = true;
            node.logical += listing.own_logical;
            node.allocated += listing.own_allocated;
            node.files += listing.own_files;
        }
        self.try_finalise(listing.index);
    }

    /// A directory that could not be read still occupies a slot: it must not be
    /// left pending or its ancestors would never finalise. Its subtree totals
    /// stay zero, and `fs_entries.accessible` marks it as unknown rather than
    /// empty.
    pub fn record_unreadable(&mut self, index: DirIndex, entry_id: i64, parent: Option<DirIndex>) {
        self.record_listing(DirListing {
            index,
            entry_id,
            parent,
            own_logical: 0,
            own_allocated: 0,
            own_files: 0,
            child_dirs: 0,
        });
    }

    fn try_finalise(&mut self, start: DirIndex) {
        self.cascade.clear();
        self.cascade.push(start);
        while let Some(index) = self.cascade.pop() {
            let node = self.nodes[index as usize];
            if node.finalised || !node.listed || node.completed_children < node.expected_children {
                continue;
            }
            self.nodes[index as usize].finalised = true;
            self.finalised_count += 1;

            if node.parent == NO_PARENT {
                continue;
            }
            let parent = &mut self.nodes[node.parent as usize];
            parent.logical += node.logical;
            parent.allocated += node.allocated;
            parent.files += node.files;
            parent.dirs += node.dirs + 1;
            parent.completed_children += 1;
            self.cascade.push(node.parent);
        }
    }

    /// Totals for the root directory of the scan, if it has finalised.
    pub fn root_totals(&self, root: DirIndex) -> Option<DirAggregate> {
        let node = self.nodes.get(root as usize)?;
        node.finalised.then_some(DirAggregate {
            id: node.entry_id,
            logical: node.logical,
            allocated: node.allocated,
            files: node.files,
            dirs: node.dirs,
        })
    }

    /// Every directory's totals, for the writer's single-pass update.
    ///
    /// Directories that never finalised (a cancelled scan leaves some pending)
    /// are still emitted with the partial totals gathered so far. Reporting a
    /// partial number for an explicitly incomplete scan is honest; dropping the
    /// row would make the folder look empty.
    pub fn drain_aggregates(&self) -> Vec<DirAggregate> {
        self.nodes
            .iter()
            .filter(|node| node.entry_id != 0)
            .map(|node| DirAggregate {
                id: node.entry_id,
                logical: node.logical,
                allocated: node.allocated,
                files: node.files,
                dirs: node.dirs,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listing(
        index: DirIndex,
        parent: Option<DirIndex>,
        bytes: i64,
        files: i64,
        child_dirs: u32,
    ) -> DirListing {
        DirListing {
            index,
            entry_id: index as i64 + 1,
            parent,
            own_logical: bytes,
            own_allocated: bytes,
            own_files: files,
            child_dirs,
        }
    }

    #[test]
    fn a_leaf_directory_finalises_immediately() {
        let mut agg = DirAggregator::new();
        agg.record_listing(listing(0, None, 100, 2, 0));
        let root = agg.root_totals(0).expect("root should be final");
        assert_eq!(root.logical, 100);
        assert_eq!(root.files, 2);
        assert_eq!(root.dirs, 0);
    }

    #[test]
    fn totals_roll_up_through_nested_directories() {
        // root -> a -> b, with bytes at each level.
        let mut agg = DirAggregator::new();
        agg.record_listing(listing(0, None, 10, 1, 1));
        agg.record_listing(listing(1, Some(0), 20, 1, 1));
        agg.record_listing(listing(2, Some(1), 30, 1, 0));

        let root = agg.root_totals(0).expect("root should be final");
        assert_eq!(root.logical, 60);
        assert_eq!(root.files, 3);
        assert_eq!(root.dirs, 2);
    }

    #[test]
    fn listings_may_arrive_before_their_parent() {
        // The deepest directory reports first, as happens when a different
        // worker thread finishes it before the parent's batch is flushed.
        let mut agg = DirAggregator::new();
        agg.record_listing(listing(2, Some(1), 30, 1, 0));
        agg.record_listing(listing(1, Some(0), 20, 1, 1));
        assert!(agg.root_totals(0).is_none(), "root is not listed yet");
        agg.record_listing(listing(0, None, 10, 1, 1));

        let root = agg.root_totals(0).expect("root should be final");
        assert_eq!(root.logical, 60);
        assert_eq!(root.dirs, 2);
    }

    #[test]
    fn a_wide_directory_finalises_once_every_child_reports() {
        let mut agg = DirAggregator::new();
        agg.record_listing(listing(0, None, 0, 0, 50));
        for index in 1..=50u32 {
            assert!(agg.root_totals(0).is_none());
            agg.record_listing(listing(index, Some(0), 2, 1, 0));
        }
        let root = agg.root_totals(0).unwrap();
        assert_eq!(root.logical, 100);
        assert_eq!(root.files, 50);
        assert_eq!(root.dirs, 50);
    }

    #[test]
    fn a_deep_chain_does_not_recurse_on_the_call_stack() {
        const DEPTH: u32 = 10_000;
        let mut agg = DirAggregator::new();
        // Report leaf-first so the whole chain finalises in one cascade.
        agg.record_listing(listing(DEPTH - 1, Some(DEPTH - 2), 1, 1, 0));
        for index in (1..DEPTH - 1).rev() {
            agg.record_listing(listing(index, Some(index - 1), 1, 1, 1));
        }
        agg.record_listing(listing(0, None, 1, 1, 1));

        let root = agg.root_totals(0).unwrap();
        assert_eq!(root.logical, DEPTH as i64);
        assert_eq!(root.dirs, DEPTH as i64 - 1);
    }

    #[test]
    fn an_unreadable_directory_does_not_stall_its_ancestors() {
        let mut agg = DirAggregator::new();
        agg.record_listing(listing(0, None, 10, 1, 1));
        agg.record_unreadable(1, 2, Some(0));
        let root = agg.root_totals(0).expect("root should still finalise");
        assert_eq!(root.logical, 10);
        assert_eq!(root.dirs, 1);
    }

    #[test]
    fn cancelled_scans_still_yield_partial_aggregates() {
        let mut agg = DirAggregator::new();
        agg.record_listing(listing(0, None, 10, 1, 2));
        agg.record_listing(listing(1, Some(0), 5, 1, 0));
        // Directory 2 never reports: the scan was cancelled.
        assert!(agg.root_totals(0).is_none());
        let aggregates = agg.drain_aggregates();
        assert_eq!(aggregates.len(), 2);
        assert_eq!(aggregates[0].logical, 15, "root keeps what was counted");
    }
}
