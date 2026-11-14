//! Capabilities: references to objects that carry rights and cannot be forged or widened.
//!
//! Three rules, each enforced here and nowhere else:
//! a capability is only ever derived from one already held; a derived capability has the same rights or
//! fewer; revoking a capability revokes everything derived from it. Authority is therefore what was handed
//! over, never what a name happens to resolve to.
use alloc::vec::Vec;

/// A set of rights.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rights(pub u32);

impl Rights {
    /// No right.
    pub const NONE: Rights = Rights(0);
    /// Read the object.
    pub const READ: Rights = Rights(1);
    /// Append to the object. There is no right to overwrite: no object offers that operation.
    pub const APPEND: Rights = Rights(2);
    /// Derive further capabilities from this one.
    pub const GRANT: Rights = Rights(4);
    /// Send to the object (an endpoint).
    pub const SEND: Rights = Rights(8);
    /// Direct ACR use: adopt it, commission under it. Held by the principal's session and, where policy
    /// allows, by the host.
    pub const DIRECT: Rights = Rights(16);
    /// Halt ACR use. Kept apart from DIRECT so that the means to stop can be handed out more widely than
    /// the means to start.
    pub const HALT: Rights = Rights(32);
    /// Administer principals, groups, roles and policy.
    pub const ADMIN: Rights = Rights(64);
    /// Stop a task.
    pub const STOP: Rights = Rights(128);
    /// Change the system: begin maintenance, promote an artifact into production, shut down.
    pub const SYSTEM: Rights = Rights(256);

    /// The union of two sets.
    pub const fn with(self, other: Rights) -> Rights {
        Rights(self.0 | other.0)
    }

    /// True if every right in `other` is in this set.
    pub const fn contains(self, other: Rights) -> bool {
        self.0 & other.0 == other.0
    }
}

/// A handle to a slot of a capability table. A stale handle is detected by its generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Handle {
    index: u32,
    generation: u32,
}

/// Why a capability operation is refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapError {
    /// The handle names no live capability: never issued, or revoked.
    Invalid,
    /// The capability lacks a right the operation needs.
    Denied,
    /// A derivation asked for a right its parent does not have.
    WouldWiden,
    /// The table is full.
    Full,
}

struct Slot {
    object: u64,
    rights: Rights,
    generation: u32,
    parent: Option<u32>,
    live: bool,
}

/// A table of capabilities held by one holder.
pub struct CapTable {
    slots: Vec<Slot>,
    limit: usize,
}

impl CapTable {
    /// A table that holds at most `limit` capabilities.
    pub fn new(limit: usize) -> Self {
        CapTable { slots: Vec::new(), limit }
    }

    fn place(&mut self, object: u64, rights: Rights, parent: Option<u32>) -> Result<Handle, CapError> {
        // A slot whose generation counter is exhausted is never reused: an old handle must not become valid again.
        if let Some(i) = self.slots.iter().position(|s| !s.live && s.generation < u32::MAX) {
            let s = &mut self.slots[i];
            s.object = object;
            s.rights = rights;
            s.parent = parent;
            s.live = true;
            s.generation += 1;
            return Ok(Handle { index: i as u32, generation: s.generation });
        }
        if self.slots.len() >= self.limit {
            return Err(CapError::Full);
        }
        self.slots.push(Slot { object, rights, generation: 0, parent, live: true });
        Ok(Handle { index: (self.slots.len() - 1) as u32, generation: 0 })
    }

    /// Install an original capability. Only the creator of an object does this.
    pub fn install(&mut self, object: u64, rights: Rights) -> Result<Handle, CapError> {
        self.place(object, rights, None)
    }

    fn slot(&self, h: Handle) -> Result<&Slot, CapError> {
        match self.slots.get(h.index as usize) {
            Some(s) if s.live && s.generation == h.generation => Ok(s),
            _ => Err(CapError::Invalid),
        }
    }

    /// The object behind a handle, if the capability is live and has the rights needed.
    pub fn check(&self, h: Handle, needed: Rights) -> Result<u64, CapError> {
        let s = self.slot(h)?;
        if s.rights.contains(needed) {
            Ok(s.object)
        } else {
            Err(CapError::Denied)
        }
    }

    /// Derive a capability with the same rights or fewer. The parent must carry GRANT.
    pub fn derive(&mut self, h: Handle, rights: Rights) -> Result<Handle, CapError> {
        let (object, have) = {
            let s = self.slot(h)?;
            (s.object, s.rights)
        };
        if !have.contains(Rights::GRANT) {
            return Err(CapError::Denied);
        }
        if !have.contains(rights) {
            return Err(CapError::WouldWiden);
        }
        self.place(object, rights, Some(h.index))
    }

    /// Revoke a capability and everything derived from it. Returns how many were revoked.
    pub fn revoke(&mut self, h: Handle) -> Result<usize, CapError> {
        self.slot(h)?;
        self.slots[h.index as usize].live = false;
        let mut n = 1;
        // Repeat until nothing changes: a capability whose parent is gone goes too. No memory is allocated.
        loop {
            let mut changed = false;
            for j in 0..self.slots.len() {
                if self.slots[j].live {
                    if let Some(p) = self.slots[j].parent {
                        if !self.slots[p as usize].live {
                            self.slots[j].live = false;
                            n += 1;
                            changed = true;
                        }
                    }
                }
            }
            if !changed {
                return Ok(n);
            }
        }
    }
}

// ---- capabilities across holders --------------------------------------------------------------------

struct Node {
    holder: u32,
    object: u64,
    rights: Rights,
    generation: u32,
    parent: Option<(u32, u32)>,
    live: bool,
}

/// Capabilities of many holders in one derivation tree, so that revoking a capability reaches what was
/// derived from it wherever it was handed. This is what lets a halt, or the end of a process, take back
/// everything that flowed from one grant.
pub struct CapSpace {
    nodes: Vec<Node>,
    limit: usize,
}

impl CapSpace {
    /// A space that holds at most `limit` capabilities in all.
    pub fn new(limit: usize) -> Self {
        CapSpace { nodes: Vec::new(), limit }
    }

    fn place(&mut self, holder: u32, object: u64, rights: Rights, parent: Option<(u32, u32)>) -> Result<Handle, CapError> {
        if let Some(i) = self.nodes.iter().position(|s| !s.live && s.generation < u32::MAX) {
            let s = &mut self.nodes[i];
            *s = Node { holder, object, rights, generation: s.generation + 1, parent, live: true };
            return Ok(Handle { index: i as u32, generation: s.generation });
        }
        if self.nodes.len() >= self.limit {
            return Err(CapError::Full);
        }
        self.nodes.push(Node { holder, object, rights, generation: 0, parent, live: true });
        Ok(Handle { index: (self.nodes.len() - 1) as u32, generation: 0 })
    }

    fn node(&self, holder: u32, h: Handle) -> Result<&Node, CapError> {
        match self.nodes.get(h.index as usize) {
            Some(s) if s.live && s.generation == h.generation && s.holder == holder => Ok(s),
            _ => Err(CapError::Invalid),
        }
    }

    /// Install an original capability for a holder. Only the creator of an object does this.
    pub fn install(&mut self, holder: u32, object: u64, rights: Rights) -> Result<Handle, CapError> {
        self.place(holder, object, rights, None)
    }

    /// The object behind a handle, if `holder` holds it live with the rights needed. A handle presented by
    /// anyone but its holder is invalid: knowing a handle's number confers nothing.
    pub fn check(&self, holder: u32, h: Handle, needed: Rights) -> Result<u64, CapError> {
        let s = self.node(holder, h)?;
        if s.rights.contains(needed) {
            Ok(s.object)
        } else {
            Err(CapError::Denied)
        }
    }

    /// Hand a capability with the same rights or fewer to `to` (which may be the holder itself). The
    /// capability handed from must carry GRANT.
    pub fn grant(&mut self, holder: u32, h: Handle, to: u32, rights: Rights) -> Result<Handle, CapError> {
        let (object, have) = {
            let s = self.node(holder, h)?;
            (s.object, s.rights)
        };
        if !have.contains(Rights::GRANT) {
            return Err(CapError::Denied);
        }
        if !have.contains(rights) {
            return Err(CapError::WouldWiden);
        }
        self.place(to, object, rights, Some((h.index, h.generation)))
    }

    fn sweep(&mut self, mut n: usize) -> usize {
        loop {
            let mut changed = false;
            for j in 0..self.nodes.len() {
                if self.nodes[j].live {
                    if let Some((p, g)) = self.nodes[j].parent {
                        let parent = &self.nodes[p as usize];
                        if !parent.live || parent.generation != g {
                            self.nodes[j].live = false;
                            n += 1;
                            changed = true;
                        }
                    }
                }
            }
            if !changed {
                return n;
            }
        }
    }

    /// Revoke a capability and everything derived from it, in every holder. Returns how many were revoked.
    /// It may be revoked by whoever holds it, or by whoever holds a capability it was derived from: what
    /// was handed out can be taken back by the one who handed it out, and by no one else.
    pub fn revoke(&mut self, holder: u32, h: Handle) -> Result<usize, CapError> {
        let mut at = match self.nodes.get(h.index as usize) {
            Some(s) if s.live && s.generation == h.generation => h.index,
            _ => return Err(CapError::Invalid),
        };
        let mut steps = 0;
        loop {
            let n = &self.nodes[at as usize];
            if n.holder == holder {
                break;
            }
            match n.parent {
                Some((p, g)) if steps < self.nodes.len() && self.nodes[p as usize].live && self.nodes[p as usize].generation == g => at = p,
                _ => return Err(CapError::Invalid),
            }
            steps += 1;
        }
        self.nodes[h.index as usize].live = false;
        Ok(self.sweep(1))
    }

    /// Revoke everything a holder holds and everything derived from it. Used when a process ends or is stopped.
    pub fn revoke_holder(&mut self, holder: u32) -> usize {
        let mut n = 0;
        for s in self.nodes.iter_mut() {
            if s.live && s.holder == holder {
                s.live = false;
                n += 1;
            }
        }
        self.sweep(n)
    }

    /// How many live capabilities a holder has.
    pub fn held_by(&self, holder: u32) -> usize {
        self.nodes.iter().filter(|s| s.live && s.holder == holder).count()
    }
}
