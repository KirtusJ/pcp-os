//! Bounded task trees. Roots are at depth zero and IDs are never reused.
#![no_std]
extern crate alloc;

use alloc::{string::String, vec, vec::Vec};
use pcpos_core::task::RecursionLimits;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Active,
    Finished,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub id: u32,
    pub parent: Option<u32>,
    pub label: String,
    pub state: State,
}

#[derive(Debug)]
pub struct Desk {
    nodes: Vec<Node>,
    live: u16,
    limits: RecursionLimits,
}

impl Desk {
    pub fn new(max_depth: u16, max_live: u16, max_total: u32) -> Result<Self, String> {
        let limits = RecursionLimits {
            max_depth,
            max_live,
            max_total,
        };
        limits.validate()?;
        Ok(Self {
            nodes: Vec::new(),
            live: 0,
            limits,
        })
    }

    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    fn active(&self, id: u32) -> Result<usize, String> {
        let index = id
            .checked_sub(1)
            .and_then(|n| usize::try_from(n).ok())
            .ok_or("unknown node")?;
        let node = self.nodes.get(index).ok_or("unknown node")?;
        if node.state != State::Active {
            return Err("node is already closed".into());
        }
        Ok(index)
    }

    pub fn open(&mut self, parent: Option<u32>, label: &str) -> Result<u32, String> {
        if label.trim().is_empty() {
            return Err("a task label is required".into());
        }
        let used = u32::try_from(self.nodes.len()).map_err(|_| "total node limit reached")?;
        let mut depth = 0;
        if let Some(id) = parent {
            let mut ancestor = &self.nodes[self.active(id)?];
            depth = 1;
            while let Some(parent_id) = ancestor.parent {
                depth += 1;
                ancestor = &self.nodes[(parent_id - 1) as usize];
            }
        }
        self.limits.admit(depth, u32::from(self.live), used)?;
        let id = used + 1;
        self.nodes.push(Node {
            id,
            parent,
            label: label.into(),
            state: State::Active,
        });
        self.live += 1;
        Ok(id)
    }

    pub fn finish(&mut self, id: u32) -> Result<(), String> {
        let index = self.active(id)?;
        if self.nodes[index + 1..]
            .iter()
            .any(|n| n.parent == Some(id) && n.state == State::Active)
        {
            return Err("finish or cancel the active children first".into());
        }
        self.nodes[index].state = State::Finished;
        self.live -= 1;
        Ok(())
    }

    /// Cancel every active node below this one. Earlier closures retain their disposition.
    pub fn cancel(&mut self, id: u32) -> Result<(), String> {
        let start = self.active(id)?;
        // Parents always precede their children. One forward pass handles arbitrary depth.
        let mut subtree = vec![false; self.nodes.len() - start];
        for (offset, node) in self.nodes[start..].iter_mut().enumerate() {
            let belongs = offset == 0
                || node
                    .parent
                    .and_then(|p| p.checked_sub(id))
                    .map(|parent_offset| subtree[parent_offset as usize])
                    .unwrap_or(false);
            if belongs {
                subtree[offset] = true;
                if node.state == State::Active {
                    node.state = State::Cancelled;
                    self.live -= 1;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn independent_depth_live_and_cumulative_limits_leave_failed_admissions_untouched() {
        assert!(Desk::new(1, 0, 1).is_err());
        assert!(Desk::new(1, 1, 0).is_err());
        let mut desk = Desk::new(1, 2, 3).unwrap();
        let root = desk.open(None, "root").unwrap();
        let child = desk.open(Some(root), "child").unwrap();
        assert_eq!(
            desk.open(None, "another").unwrap_err(),
            "live node limit reached"
        );
        desk.finish(child).unwrap();
        assert!(desk.open(Some(child), "closed parent").is_err());
        assert!(desk.open(Some(0), "zero").is_err());
        assert_eq!(desk.open(None, "another").unwrap(), 3);
        desk.finish(3).unwrap();
        assert_eq!(
            desk.open(None, "after close").unwrap_err(),
            "total node limit reached"
        );
        let mut root_only = Desk::new(0, 2, 4).unwrap();
        root_only.open(None, "root").unwrap();
        assert_eq!(
            root_only.open(Some(1), "too deep").unwrap_err(),
            "depth limit reached"
        );
        assert_eq!(root_only.nodes().len(), 1);
    }

    #[test]
    fn cancelling_one_subtree_preserves_siblings_and_completed_descendants() {
        let mut desk = Desk::new(4, 12, 12).unwrap();
        for (parent, label) in [
            (None, "root"),
            (Some(1), "left"),
            (Some(1), "right"),
            (Some(2), "closed"),
            (Some(2), "open"),
            (Some(5), "leaf"),
        ] {
            desk.open(parent, label).unwrap();
        }
        desk.finish(4).unwrap();
        assert!(desk.finish(1).is_err());
        desk.cancel(2).unwrap();
        assert_eq!(
            desk.nodes().iter().map(|n| n.state).collect::<Vec<_>>(),
            [
                State::Active,
                State::Cancelled,
                State::Active,
                State::Finished,
                State::Cancelled,
                State::Cancelled
            ]
        );
        assert!(desk.cancel(2).is_err());
        desk.finish(3).unwrap();
        desk.finish(1).unwrap();
    }
}
