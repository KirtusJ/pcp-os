//! Who may do what: identities, roles, groups, editions and standards.
//!
//! The kernel knows nothing of users, groups or roles. It knows capabilities. This module is the policy
//! side: a pure function from *who someone is and what they have been assigned* to *the rights their
//! session is handed when it starts*. After that hand-over nothing is looked up by name again; every
//! later check is a capability check.
//!
//! * An **identity** is a person, a program or a model-driven process. It is established outside this
//!   module (login, attestation, the host). Identity is not authority.
//! * A **role** is a named bundle of rights. A **group** is a set of roles that identities can be put in
//!   together; it is an administrative convenience and confers nothing that its roles do not.
//! * An **edition** fixes which separations of duty are enforced by the system and which are left to the
//!   one person using it.
//! * A **standard** is the installation's policy on matters ACR leaves to whoever directs the work. A
//!   standard only ever narrows what the method allows; it cannot widen it. See [`Standard::at_least`].
//!
//! Two rules hold in every edition under every standard and are checked exhaustively in the tests:
//! a model-driven process is never handed the right to adopt ACR use, to administer, or to change the
//! system; and no combination of roles yields a right to overwrite, because no object offers one.
//!
//! This module uses `core` only.
use crate::cap::Rights;

/// What kind of thing an identity is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A natural person.
    Human,
    /// A program whose behaviour is fixed by its code and inputs, including the host.
    Program,
    /// A process driven by a language model.
    Model,
}

/// A set of roles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Roles(pub u16);

impl Roles {
    /// No role.
    pub const NONE: Roles = Roles(0);
    /// Directs the work: decides that ACR is used, commissions reviews, halts.
    pub const PRINCIPAL: Roles = Roles(1);
    /// Manages identities, groups, roles and the standard. Assesses nothing.
    pub const ADMINISTRATOR: Roles = Roles(2);
    /// Directs a review.
    pub const DIRECTOR: Roles = Roles(4);
    /// Examines, challenges, records findings and objections.
    pub const REVIEWER: Roles = Roles(8);
    /// Supplies the subject of a review. The record states the relation; it confers no right.
    pub const AUTHOR: Roles = Roles(16);
    /// Keeps the system running: sees tasks, stops them, can halt. Cannot start ACR use.
    pub const OPERATOR: Roles = Roles(32);
    /// Reads everything, writes nothing.
    pub const AUDITOR: Roles = Roles(64);
    /// Every role.
    pub const ALL: Roles = Roles(127);

    /// The union of two sets.
    pub const fn with(self, other: Roles) -> Roles {
        Roles(self.0 | other.0)
    }

    /// True if every role in `other` is in this set.
    pub const fn has(self, other: Roles) -> bool {
        self.0 & other.0 == other.0 && other.0 != 0
    }

    /// The union of an identity's own roles and those of the groups it is in.
    pub fn effective(own: Roles, groups: &[Roles]) -> Roles {
        let mut r = own.0;
        for g in groups {
            r |= g.0;
        }
        Roles(r & Roles::ALL.0)
    }
}

/// The three editions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edition {
    /// Licensed to one natural person.
    Personal,
    /// Licensed to the personnel of one organization.
    Commercial,
    /// Licensed to an organization, its platforms and affiliates, by agreement.
    Enterprise,
}

/// How ACR is present in an installation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Method {
    /// The method built into this system release, or a packet release pinned by digest: the person
    /// directing the work chooses, and the record says which.
    NativeOrPinned,
    /// Only the method built into this system release. Its identity is the identity of the release.
    NativeOnly,
}

/// What a model-driven reviewer's finding may be.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ModelFindings {
    /// As the method allows.
    AsMethod,
    /// As the method allows, but a review holding a model's finding is closed as complete only by a person.
    HumanCloses,
    /// A model may record SUPPORTED_IN_SCOPE or REFUTED_IN_SCOPE only on a claim that is a mechanical
    /// check, and a review holding a model's finding is closed as complete only by a person.
    MechanicalOnly,
}

/// An installation's standard. Each field is ordered from least to most strict.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Standard {
    /// How ACR is present.
    pub method: Method,
    /// Limits on a model-driven reviewer's findings.
    pub model_findings: ModelFindings,
    /// Only a person may adopt ACR use (the host may not).
    pub human_adoption: bool,
    /// Changing the system, and replacing the standard by a less strict one, needs two different people.
    pub dual_authorization: bool,
    /// The set of tasks is fixed when the system is built; none is admitted afterwards.
    pub static_tasks: bool,
    /// The system never goes online.
    pub offline_only: bool,
}

impl Standard {
    /// The default: nothing beyond what the method and the system already enforce.
    pub const BASELINE: Standard = Standard { method: Method::NativeOrPinned, model_findings: ModelFindings::AsMethod,
                                              human_adoption: false, dual_authorization: false, static_tasks: false, offline_only: false };
    /// The strictest predefined standard, for use where a wrong result or an uncontrolled process can
    /// harm people. It is a set of restrictions; it is not a certification of anything.
    pub const CRITICAL: Standard = Standard { method: Method::NativeOnly, model_findings: ModelFindings::MechanicalOnly,
                                              human_adoption: true, dual_authorization: true, static_tasks: true, offline_only: true };

    /// Whether an edition can meet this standard. The Personal edition is licensed to one person, so it
    /// cannot meet a requirement for two.
    pub fn available_in(&self, edition: Edition) -> bool {
        !(self.dual_authorization && edition == Edition::Personal)
    }

    /// True if this standard is at least as strict as `other` in every field. Replacing a standard by one
    /// for which this is false is a relaxation.
    pub fn at_least(&self, other: &Standard) -> bool {
        self.method >= other.method
            && self.model_findings >= other.model_findings
            && (self.human_adoption || !other.human_adoption)
            && (self.dual_authorization || !other.dual_authorization)
            && (self.static_tasks || !other.static_tasks)
            && (self.offline_only || !other.offline_only)
    }
}

/// The rights a session is handed, by class of object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Authority {
    /// On the register: READ, APPEND.
    pub register: Rights,
    /// On the store of artifacts: READ, APPEND.
    pub store: Rights,
    /// On the use of ACR: DIRECT (adopt, commission), HALT.
    pub direction: Rights,
    /// On tasks: READ, STOP.
    pub tasks: Rights,
    /// On identities, groups, roles and the standard: READ, ADMIN.
    pub administration: Rights,
    /// On the system itself: SYSTEM.
    pub system: Rights,
}

impl Authority {
    /// No right at all.
    pub const NONE: Authority = Authority { register: Rights::NONE, store: Rights::NONE, direction: Rights::NONE, tasks: Rights::NONE,
                                            administration: Rights::NONE, system: Rights::NONE };

    /// The union of every right in this authority, for checks over the whole.
    pub fn union(&self) -> Rights {
        self.register.with(self.store).with(self.direction).with(self.tasks).with(self.administration).with(self.system)
    }
}

/// Why a combination of identity and roles is not given any authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Violation {
    /// No role is assigned.
    NoRole,
    /// A model-driven process or a program was assigned a role that only a person holds.
    NotAPerson,
    /// An auditor was also assigned a role that writes.
    AuditorWrites,
    /// An administrator was also assigned a role that assesses (Commercial and Enterprise).
    AdministratorAssesses,
    /// The edition is licensed to one person, and a role that exists only among several was assigned.
    NotInThisEdition,
}

/// The rights the session of an identity is handed, or the reason it is handed none.
pub fn authority_of(kind: Kind, roles: Roles, edition: Edition) -> Result<Authority, Violation> {
    let roles = Roles(roles.0 & Roles::ALL.0);
    if roles.0 == 0 {
        return Err(Violation::NoRole);
    }
    let person_only = Roles::PRINCIPAL.with(Roles::ADMINISTRATOR).with(Roles::AUDITOR);
    if kind != Kind::Human && roles.0 & person_only.0 != 0 {
        return Err(Violation::NotAPerson);
    }
    let writes = Roles::PRINCIPAL.with(Roles::ADMINISTRATOR).with(Roles::DIRECTOR).with(Roles::REVIEWER).with(Roles::AUTHOR).with(Roles::OPERATOR);
    if roles.has(Roles::AUDITOR) && roles.0 & writes.0 != 0 {
        return Err(Violation::AuditorWrites);
    }
    match edition {
        Edition::Personal => {
            if roles.has(Roles::ADMINISTRATOR) {
                return Err(Violation::NotInThisEdition); // one person: the principal administers
            }
        }
        Edition::Commercial | Edition::Enterprise => {
            if roles.has(Roles::ADMINISTRATOR) && roles.0 & Roles::DIRECTOR.with(Roles::REVIEWER).0 != 0 {
                return Err(Violation::AdministratorAssesses);
            }
        }
    }
    let mut a = Authority::NONE;
    let rw = Rights::READ.with(Rights::APPEND);
    if roles.has(Roles::PRINCIPAL) {
        a.register = a.register.with(rw);
        a.store = a.store.with(rw);
        a.direction = a.direction.with(Rights::DIRECT).with(Rights::HALT).with(Rights::GRANT);
        a.tasks = a.tasks.with(Rights::READ).with(Rights::STOP);
        a.system = a.system.with(Rights::SYSTEM);
        if edition == Edition::Personal {
            a.administration = a.administration.with(Rights::READ).with(Rights::ADMIN);
        }
    }
    if roles.has(Roles::ADMINISTRATOR) {
        a.administration = a.administration.with(Rights::READ).with(Rights::ADMIN);
        a.register = a.register.with(Rights::READ);
        a.system = a.system.with(Rights::SYSTEM);
    }
    if roles.has(Roles::DIRECTOR) || roles.has(Roles::REVIEWER) {
        a.register = a.register.with(rw);
        a.store = a.store.with(rw);
        a.tasks = a.tasks.with(Rights::READ);
    }
    if roles.has(Roles::DIRECTOR) {
        a.tasks = a.tasks.with(Rights::STOP);
    }
    if roles.has(Roles::AUTHOR) {
        a.register = a.register.with(Rights::READ);
        a.store = a.store.with(rw);
    }
    if roles.has(Roles::OPERATOR) {
        a.register = a.register.with(Rights::READ);
        a.tasks = a.tasks.with(Rights::READ).with(Rights::STOP);
        a.direction = a.direction.with(Rights::HALT);
    }
    if roles.has(Roles::AUDITOR) {
        a.register = a.register.with(Rights::READ);
        a.store = a.store.with(Rights::READ);
        a.tasks = a.tasks.with(Rights::READ);
        a.administration = a.administration.with(Rights::READ);
    }
    Ok(a)
}

/// The authority of the host itself: the system's own programs acting for no person. It can halt and
/// stop, and record; under a standard with `human_adoption` it cannot adopt.
pub fn authority_of_host(standard: &Standard) -> Authority {
    let mut a = Authority::NONE;
    a.register = Rights::READ.with(Rights::APPEND);
    a.store = Rights::READ.with(Rights::APPEND);
    a.tasks = Rights::READ.with(Rights::STOP);
    a.direction = if standard.human_adoption { Rights::HALT } else { Rights::HALT.with(Rights::DIRECT) };
    a
}

/// Whether an act that changes the system, or relaxes the standard, is authorized: by one person holding
/// the SYSTEM right, or under `dual_authorization` by two different people who each hold it. `first` and
/// `second` are the numbers of the two identities; `second` is `None` when only one person acted.
pub fn change_authorized(standard: &Standard, first: (u32, &Authority), second: Option<(u32, &Authority)>) -> bool {
    if !first.1.system.contains(Rights::SYSTEM) {
        return false;
    }
    if !standard.dual_authorization {
        return true;
    }
    match second {
        Some((id, a)) => id != first.0 && a.system.contains(Rights::SYSTEM),
        None => false,
    }
}
