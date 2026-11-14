//! The install transaction, as a state that changes only by whole steps.
//!
//! What is installed is a sequence of *generations*. A generation is a complete list of what is active:
//! for each package, the digest of the one entry installed. Installing, removing and undoing never edit a
//! generation; each makes a new one and moves one pointer to it. So there is always a whole previous state
//! to go back to, and a reader never sees half of a change.
//!
//! A change is made in two steps, by a person's command or a machine's tool call alike. A *plan* is drawn
//! against the generation that is active when it is drawn, and says exactly what would change. *Commit*
//! applies a plan only if that generation is still the active one: a plan shown to someone and then
//! overtaken by another change is refused, not applied to a state its reader never saw.
//!
//! Each request carries an identifier chosen by the caller. The same identifier with the same plan is the
//! same request arriving again (a reply was lost, a call was repeated) and gets the first answer; the same
//! identifier with another plan is refused. Nothing here touches a file: the caller stages and verifies the
//! bytes first ([`crate::verify`]) and records each step in a register afterwards.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::Entry;

/// One complete list of what is active: `owner/name` to the digest of its entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Generation {
    /// Its number. The first is 1; numbers are never reused. Number 0 is the empty state before any
    /// installation: it is not stored, and it can be restored like any other.
    pub number: u64,
    /// The generation that was active when this one was made (0 for the first).
    pub made_from: u64,
    /// What is installed in it.
    pub packages: BTreeMap<String, String>,
}

/// What a plan would do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    /// Install a package that is not installed.
    Add {
        /// `owner/name`.
        package: String,
        /// The digest of the entry to install.
        digest: String,
    },
    /// Put another entry in place of the one installed.
    Replace {
        /// `owner/name`.
        package: String,
        /// The digest installed now.
        from: String,
        /// The digest to install.
        to: String,
    },
    /// Take a package out.
    Remove {
        /// `owner/name`.
        package: String,
        /// The digest installed now.
        digest: String,
    },
    /// Make active again exactly what an earlier generation held.
    Restore {
        /// The number of that generation; 0 is the empty state before any installation.
        generation: u64,
    },
}

/// A change drawn against one generation, not yet applied.
///
/// A plan is made only by the `plan_` methods of [`Installed`], from an entry that was read and checked, and
/// it cannot be changed afterwards: it is read through the methods below. So what [`Installed::commit`]
/// applies is the plan that was issued, for the package and the digest it was issued for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    request: String,
    against: u64,
    change: Change,
}

impl Plan {
    /// The caller's identifier for this request.
    pub fn request(&self) -> &str {
        &self.request
    }
    /// The generation that was active when the plan was drawn (0 is the empty state before any installation).
    pub fn against(&self) -> u64 {
        self.against
    }
    /// What it would do.
    pub fn change(&self) -> &Change {
        &self.change
    }
}

/// What is installed, and every state it has been in.
#[derive(Clone, Debug, Default)]
pub struct Installed {
    generations: Vec<Generation>,
    active: u64,
    answered: BTreeMap<String, (Plan, u64)>,
}

fn key(owner: &str, name: &str) -> String {
    format!("{owner}/{name}")
}

impl Installed {
    /// Nothing installed.
    pub fn new() -> Installed {
        Installed::default()
    }

    /// The number of the active generation; 0 is the empty state before any installation.
    pub fn active(&self) -> u64 {
        self.active
    }

    /// The active generation, if there is one.
    pub fn current(&self) -> Option<&Generation> {
        self.generations.iter().find(|g| g.number == self.active)
    }

    /// Every generation there has been, oldest first. None is ever removed or changed.
    pub fn history(&self) -> &[Generation] {
        &self.generations
    }

    /// The digest installed for a package in the active generation.
    pub fn installed(&self, owner: &str, name: &str) -> Option<&str> {
        self.current().and_then(|g| g.packages.get(&key(owner, name))).map(String::as_str)
    }

    fn request_ok(request: &str) -> Result<(), String> {
        let ok = !request.is_empty() && request.len() <= 64 && request.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-');
        if ok { Ok(()) } else { Err("a request identifier is 1 to 64 ASCII letters, digits and hyphens".to_string()) }
    }

    /// Draw the plan that installs `entry`. A withdrawn entry is planned only when `pinned` is its digest:
    /// that is an installation naming exactly what it had, not a new choice.
    pub fn plan_install(&self, request: &str, entry: &Entry, pinned: Option<&str>) -> Result<Plan, String> {
        Self::request_ok(request)?;
        if entry.withdrawn() && pinned != Some(entry.digest()) {
            return Err(format!("{} is withdrawn from new installations; it is installed only by naming its digest {}", entry.id(), entry.digest()));
        }
        if let Some(p) = pinned.filter(|p| *p != entry.digest()) {
            return Err(format!("the digest named is {p} and the entry of {} has digest {}", entry.id(), entry.digest()));
        }
        let package = key(entry.owner(), entry.name());
        let change = match self.installed(entry.owner(), entry.name()) {
            None => Change::Add { package, digest: entry.digest().to_string() },
            Some(d) if d == entry.digest() => return Err(format!("{} is already installed with this digest; there is nothing to change", entry.id())),
            Some(d) => Change::Replace { package, from: d.to_string(), to: entry.digest().to_string() },
        };
        Ok(Plan { request: request.to_string(), against: self.active, change })
    }

    /// Draw the plan that removes a package.
    pub fn plan_remove(&self, request: &str, owner: &str, name: &str) -> Result<Plan, String> {
        Self::request_ok(request)?;
        let digest = self.installed(owner, name).ok_or(format!("{owner}/{name} is not installed"))?.to_string();
        Ok(Plan { request: request.to_string(), against: self.active, change: Change::Remove { package: key(owner, name), digest } })
    }

    /// Draw the plan that undoes the last change: it makes active again what the generation before the
    /// active one held. Undoing twice goes back and forth between the two; an older state is reached with
    /// [`Installed::plan_restore`].
    pub fn plan_undo(&self, request: &str) -> Result<Plan, String> {
        let before = self.current().map(|g| g.made_from).ok_or("nothing has been installed; there is no earlier state to go back to")?;
        self.plan_restore(request, before)
    }

    fn packages_of(&self, number: u64) -> Option<BTreeMap<String, String>> {
        if number == 0 {
            return Some(BTreeMap::new());
        }
        self.generations.iter().find(|g| g.number == number).map(|g| g.packages.clone())
    }

    /// Draw the plan that makes active again what generation `number` held.
    pub fn plan_restore(&self, request: &str, number: u64) -> Result<Plan, String> {
        Self::request_ok(request)?;
        if number == self.active {
            return Err(format!("generation {number} is the active one; there is nothing to change"));
        }
        if self.packages_of(number).is_none() {
            return Err(format!("there is no generation {number}"));
        }
        Ok(Plan { request: request.to_string(), against: self.active, change: Change::Restore { generation: number } })
    }

    /// Apply a plan. Returns the number of the generation it made active.
    pub fn commit(&mut self, plan: &Plan) -> Result<u64, String> {
        Self::request_ok(&plan.request)?;
        if let Some((first, made)) = self.answered.get(&plan.request) {
            return if first == plan { Ok(*made) } else { Err(format!("request {} was already used for another change; a request identifier names one change", plan.request)) };
        }
        if plan.against != self.active {
            return Err(format!("the plan was drawn against generation {} and generation {} is active now; it is not applied to a state it was not drawn for", plan.against, self.active));
        }
        let mut packages = self.packages_of(self.active).unwrap_or_default();
        match &plan.change {
            Change::Add { package, digest } => {
                if packages.insert(package.clone(), digest.clone()).is_some() {
                    return Err(format!("{package} is installed; the plan to add it does not fit the active generation"));
                }
            }
            Change::Replace { package, from, to } => {
                if packages.get(package) != Some(from) {
                    return Err(format!("{package} is not installed with digest {from}; the plan does not fit the active generation"));
                }
                packages.insert(package.clone(), to.clone());
            }
            Change::Remove { package, digest } => {
                if packages.remove(package).as_ref() != Some(digest) {
                    return Err(format!("{package} is not installed with digest {digest}; the plan does not fit the active generation"));
                }
            }
            Change::Restore { generation } => {
                if *generation == self.active {
                    return Err(format!("generation {generation} is the active one; the plan to restore it does not fit the active generation"));
                }
                packages = self.packages_of(*generation).ok_or(format!("there is no generation {generation}"))?;
            }
        }
        let number = self.generations.last().map_or(1, |g| g.number + 1);
        // Every generation remembers which one was active when it was made, so that undo, undo returns here.
        self.generations.push(Generation { number, made_from: self.active, packages });
        self.active = number;
        self.answered.insert(plan.request.clone(), (plan.clone(), number));
        Ok(number)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use pcpos_core::sha256_hex;

    fn entry(name: &str, version: &str, body: &[u8], withdrawn: bool) -> Entry {
        let json = format!(r#"{{"capabilities":[],"commit":"0123456789abcdef0123456789abcdef01234567","files":[{{"path":"Specification.md","sha256":"{}","size":{}}}],"kind":"module","name":"{name}","owner":"kirtusj","pcp_index":1,"version":"{version}","withdrawn":{withdrawn}}}"#, sha256_hex(body), body.len());
        Entry::parse(json.as_bytes()).unwrap()
    }

    #[test]
    fn a_change_is_a_new_generation_and_the_old_ones_stay() {
        let (a1, a2, b1) = (entry("optics", "1.0.0", b"one", false), entry("optics", "1.1.0", b"two", false), entry("acoustics", "0.1.0", b"three", false));
        let mut s = Installed::new();
        assert_eq!((s.active(), s.current(), s.installed("kirtusj", "optics")), (0, None, None));
        let p = s.plan_install("r-1", &a1, None).unwrap();
        assert_eq!((p.request(), p.against(), p.change()), ("r-1", 0, &Change::Add { package: "kirtusj/optics".to_string(), digest: a1.digest().to_string() }));
        assert_eq!(s.active(), 0, "a plan changes nothing");
        assert_eq!(s.commit(&p).unwrap(), 1);
        assert_eq!(s.installed("kirtusj", "optics"), Some(a1.digest()));
        assert_eq!(s.commit(&s.plan_install("r-2", &b1, None).unwrap()).unwrap(), 2);
        let p = s.plan_install("r-3", &a2, None).unwrap();
        assert!(matches!(p.change(), Change::Replace { from, to, .. } if from == a1.digest() && to == a2.digest()));
        assert_eq!(s.commit(&p).unwrap(), 3);
        assert_eq!(s.commit(&s.plan_remove("r-4", "kirtusj", "acoustics").unwrap()).unwrap(), 4);
        assert_eq!((s.installed("kirtusj", "optics"), s.installed("kirtusj", "acoustics")), (Some(a2.digest()), None));
        // Every earlier state is still there, unchanged.
        let sizes: Vec<usize> = s.history().iter().map(|g| g.packages.len()).collect();
        assert_eq!(sizes, vec![1, 2, 2, 1]);
        assert_eq!(s.history()[1].packages.get("kirtusj/optics").map(String::as_str), Some(a1.digest()));
        assert_eq!(s.history().iter().map(|g| g.made_from).collect::<Vec<_>>(), vec![0, 1, 2, 3]);
        // Nothing to do is said, not done.
        assert!(s.plan_install("r-5", &a2, None).unwrap_err().contains("already installed"));
        assert!(s.plan_remove("r-5", "kirtusj", "acoustics").unwrap_err().contains("not installed"));
    }

    #[test]
    fn undo_goes_back_to_a_whole_earlier_state() {
        let (a1, a2) = (entry("optics", "1.0.0", b"one", false), entry("optics", "1.1.0", b"two", false));
        let mut s = Installed::new();
        assert!(s.plan_undo("u-0").unwrap_err().contains("nothing has been installed"));
        s.commit(&s.plan_install("r-1", &a1, None).unwrap()).unwrap();
        s.commit(&s.plan_install("r-2", &a2, None).unwrap()).unwrap();
        assert_eq!(s.commit(&s.plan_undo("u-1").unwrap()).unwrap(), 3);
        assert_eq!(s.installed("kirtusj", "optics"), Some(a1.digest()), "back to the first entry");
        assert_eq!(s.history().len(), 3, "going back is a new generation; none is deleted");
        // Undo again returns to what was undone.
        assert_eq!(s.commit(&s.plan_undo("u-2").unwrap()).unwrap(), 4);
        assert_eq!(s.installed("kirtusj", "optics"), Some(a2.digest()));
        // Any earlier state can be named.
        assert_eq!(s.commit(&s.plan_restore("u-3", 1).unwrap()).unwrap(), 5);
        assert_eq!(s.installed("kirtusj", "optics"), Some(a1.digest()));
        assert!(s.plan_restore("u-4", 5).unwrap_err().contains("the active one"));
        assert!(s.plan_restore("u-4", 99).unwrap_err().contains("no generation 99"));
    }

    #[test]
    fn the_first_installation_can_be_undone() {
        // Director G's finding: the empty state before any installation is a state like any other.
        let a1 = entry("optics", "1.0.0", b"one", false);
        let mut s = Installed::new();
        assert!(s.plan_restore("u-0", 0).unwrap_err().contains("the active one"), "nothing installed: the empty state is active");
        assert_eq!(s.commit(&s.plan_install("r-1", &a1, None).unwrap()).unwrap(), 1);
        // Undo the first installation: a new generation, numbered on, and empty.
        let undo = s.plan_undo("u-1").unwrap();
        assert_eq!(undo.change(), &Change::Restore { generation: 0 });
        assert_eq!(s.commit(&undo).unwrap(), 2);
        assert_eq!((s.active(), s.installed("kirtusj", "optics"), s.current().map(|g| g.packages.len())), (2, None, Some(0)));
        // Undo that: the first package is back, in generation 3.
        assert_eq!(s.commit(&s.plan_undo("u-2").unwrap()).unwrap(), 3);
        assert_eq!(s.installed("kirtusj", "optics"), Some(a1.digest()));
        assert_eq!(s.history().iter().map(|g| (g.number, g.made_from, g.packages.len())).collect::<Vec<_>>(), vec![(1, 0, 1), (2, 1, 0), (3, 2, 1)]);
        // The empty state can also be named directly.
        assert_eq!(s.commit(&s.plan_restore("u-3", 0).unwrap()).unwrap(), 4);
        assert_eq!(s.current().map(|g| g.packages.len()), Some(0));
    }

    #[test]
    fn a_plan_is_what_was_issued_or_it_is_refused() {
        // Director G's finding. From outside this crate a plan cannot be built or edited at all: its fields
        // are private. Inside it, a plan put together by hand is still checked against the active generation.
        let a1 = entry("optics", "1.0.0", b"one", false);
        let mut s = Installed::new();
        s.commit(&s.plan_install("r-1", &a1, None).unwrap()).unwrap();
        let by_hand = |request: &str, against: u64, change: Change| Plan { request: request.to_string(), against, change };
        // Restoring the generation that is active: the planner refuses it, and so does commit.
        assert!(s.commit(&by_hand("h-1", 1, Change::Restore { generation: 1 })).unwrap_err().contains("does not fit"));
        assert!(s.commit(&by_hand("h-2", 1, Change::Restore { generation: 7 })).unwrap_err().contains("no generation 7"));
        // A package that is installed cannot be added again, whatever digest is named.
        assert!(s.commit(&by_hand("h-3", 1, Change::Add { package: "kirtusj/optics".to_string(), digest: "x".to_string() })).unwrap_err().contains("does not fit"));
        assert_eq!((s.active(), s.history().len()), (1, 1), "nothing was applied");
    }

    #[test]
    fn a_plan_overtaken_by_another_change_is_not_applied() {
        let (a1, a2, b1) = (entry("optics", "1.0.0", b"one", false), entry("optics", "1.1.0", b"two", false), entry("acoustics", "0.1.0", b"three", false));
        let mut s = Installed::new();
        s.commit(&s.plan_install("r-1", &a1, None).unwrap()).unwrap();
        // A person is shown this plan; before they confirm, a machine installs something else.
        let shown = s.plan_install("person-1", &a2, None).unwrap();
        s.commit(&s.plan_install("machine-1", &b1, None).unwrap()).unwrap();
        let e = s.commit(&shown).unwrap_err();
        assert!(e.contains("drawn against generation 1") && e.contains("generation 2 is active"), "{e}");
        assert_eq!(s.installed("kirtusj", "optics"), Some(a1.digest()), "nothing changed");
        // Drawn again against what is active now, it applies.
        assert_eq!(s.commit(&s.plan_install("person-2", &a2, None).unwrap()).unwrap(), 3);
        // A plan altered after it was drawn does not fit and is refused.
        let mut forged = s.plan_remove("x-1", "kirtusj", "acoustics").unwrap();
        forged.change = Change::Remove { package: "kirtusj/optics".to_string(), digest: a1.digest().to_string() };
        assert!(s.commit(&forged).unwrap_err().contains("does not fit"));
        let mut forged = s.plan_remove("x-2", "kirtusj", "acoustics").unwrap();
        forged.change = Change::Add { package: "kirtusj/optics".to_string(), digest: a1.digest().to_string() };
        assert!(s.commit(&forged).unwrap_err().contains("does not fit"));
        // A replacement that states the wrong entry as the one installed: refused, whatever it would put in.
        let mut forged = s.plan_remove("x-3", "kirtusj", "acoustics").unwrap();
        forged.change = Change::Replace { package: "kirtusj/optics".to_string(), from: a1.digest().to_string(), to: b1.digest().to_string() };
        assert!(s.commit(&forged).unwrap_err().contains("does not fit"));
        assert_eq!((s.active(), s.installed("kirtusj", "optics")), (3, Some(a2.digest())));
    }

    #[test]
    fn a_request_repeated_is_answered_once_and_a_request_reused_is_refused() {
        let (a1, b1) = (entry("optics", "1.0.0", b"one", false), entry("acoustics", "0.1.0", b"three", false));
        let mut s = Installed::new();
        let p = s.plan_install("r-1", &a1, None).unwrap();
        assert_eq!(s.commit(&p).unwrap(), 1);
        // The reply was lost and the same request arrives again, even after other changes: the first answer, no new generation.
        s.commit(&s.plan_install("r-2", &b1, None).unwrap()).unwrap();
        assert_eq!(s.commit(&p).unwrap(), 1);
        assert_eq!((s.active(), s.history().len()), (2, 2));
        // The same identifier for another change is refused.
        let other = Plan { request: "r-1".to_string(), against: s.active(), change: Change::Remove { package: "kirtusj/optics".to_string(), digest: a1.digest().to_string() } };
        assert!(s.commit(&other).unwrap_err().contains("already used for another change"));
        for bad in ["", "has space", "semi;colon", &"x".repeat(65)] {
            assert!(s.plan_remove(bad, "kirtusj", "optics").unwrap_err().contains("request identifier"), "{bad:?}");
        }
    }

    #[test]
    fn a_withdrawn_entry_is_installed_only_by_its_digest() {
        let w = entry("optics", "1.0.0", b"one", true);
        let live = entry("optics", "1.1.0", b"two", false);
        let mut s = Installed::new();
        assert!(s.plan_install("r-1", &w, None).unwrap_err().contains("withdrawn"));
        assert!(s.plan_install("r-1", &w, Some(live.digest())).unwrap_err().contains("withdrawn"));
        assert_eq!(s.commit(&s.plan_install("r-1", &w, Some(w.digest())).unwrap()).unwrap(), 1);
        // A digest named for an entry that is not withdrawn must still be that entry's.
        assert!(s.plan_install("r-2", &live, Some(w.digest())).unwrap_err().contains("the digest named"));
        assert_eq!(s.commit(&s.plan_install("r-2", &live, Some(live.digest())).unwrap()).unwrap(), 2);
    }
}
