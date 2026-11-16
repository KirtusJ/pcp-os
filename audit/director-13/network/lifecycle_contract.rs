//! Test-only contract for a future network controller, NOT a PCP OS implementation.
//! Separates operator intent, link/association observation, network configuration,
//! reachability and independent firewall policy. The modeled checks cannot enforce
//! a physical device, Wi-Fi authentication, routing or filter in the live kernel.
//!
//! External anchors: RFC 2863 ifAdminStatus/ifOperStatus, RFC 2131 reacquisition
//! after link loss, and NetworkManager's distinction between disconnect and
//! auto-activation. See README.md. Both positive and adverse cases are modeled.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Medium { Ethernet, Wifi }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Carrier { Unknown, Absent, Down, Up }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Disabled, WaitingManual, WaitingLink, WaitingAuthentication, Configuring,
    LanReady, InternetConfirmed, Limited, Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    OperatorConnect { allow_auto_reconnect: bool },
    OperatorDisconnect,
    ObservedCarrier(Carrier),
    WifiAuthenticated { generation: u64 },
    WifiAuthenticationFailed { generation: u64 },
    ConfigurationSucceeded { generation: u64 },
    ConfigurationFailed { generation: u64 },
    ReachabilitySucceeded { generation: u64 },
    ReachabilityFailed { generation: u64 },
    SuspendOrResume,
}

#[derive(Debug, Clone)]
pub struct Network {
    pub medium: Medium,
    pub phase: Phase,
    pub carrier: Carrier,
    pub operator_enabled: bool,
    pub auto_reconnect: bool,
    // Operator activation is separate from the interface being enabled.
    // After a link loss, a nonautomatic profile requires a fresh Connect.
    pub activation_armed: bool,
    pub attempt_seen: bool,
    pub generation: u64,
    pub address_valid: bool,
    pub route_valid: bool,
    pub dns_valid: bool,
    pub wifi_authenticated: bool,
    // The policy is owned and enforced by a DIFFERENT trusted component.
    pub firewall_policy_revision: u64,
}

impl Network {
    pub fn new(medium: Medium, firewall_policy_revision: u64) -> Self {
        Self {
            medium, phase: Phase::Disabled, carrier: Carrier::Unknown,
            operator_enabled: false, auto_reconnect: false,
            activation_armed: false, attempt_seen: false,
            generation: 1, address_valid: false, route_valid: false,
            dns_valid: false, wifi_authenticated: false,
            firewall_policy_revision,
        }
    }

    fn invalidate(&mut self) {
        self.address_valid = false;
        self.route_valid = false;
        self.dns_valid = false;
        self.wifi_authenticated = false;
    }

    fn bump(&mut self) -> Result<(), &'static str> {
        self.generation = self.generation.checked_add(1).ok_or("generation exhausted")?;
        Ok(())
    }

    fn can_configure(&self) -> bool {
        self.operator_enabled && self.activation_armed && self.carrier == Carrier::Up &&
            (self.medium == Medium::Ethernet || self.wifi_authenticated)
    }

    fn choose_phase(&mut self) {
        self.phase = if !self.operator_enabled {
            Phase::Disabled
        } else if !self.activation_armed {
            Phase::WaitingManual
        } else if self.carrier != Carrier::Up {
            Phase::WaitingLink
        } else if self.medium == Medium::Wifi && !self.wifi_authenticated {
            self.attempt_seen = true;
            Phase::WaitingAuthentication
        } else {
            self.attempt_seen = true;
            Phase::Configuring
        };
    }

    // This transition updates model state only. An actual implementation MUST
    // separately read back driver, DHCP, route, DNS and firewall destinations.
    pub fn step(&mut self, event: Event) -> Result<bool, &'static str> {
        let previous = self.clone();
        match event {
            Event::OperatorConnect { allow_auto_reconnect } => {
                self.bump()?;
                self.operator_enabled = true;
                self.auto_reconnect = allow_auto_reconnect;
                self.activation_armed = true;
                self.attempt_seen = false;
                self.invalidate();
                self.choose_phase();
            }
            Event::OperatorDisconnect => {
                self.bump()?;
                self.operator_enabled = false;
                self.auto_reconnect = false;
                self.activation_armed = false;
                self.attempt_seen = false;
                self.invalidate();
                self.phase = Phase::Disabled;
            }
            Event::ObservedCarrier(carrier) => {
                if self.carrier == carrier { return Ok(false); }
                self.bump()?;
                self.carrier = carrier;
                self.invalidate();
                // Do not let a rising carrier silently renew an old explicit
                // one-shot activation. An initially absent carrier can still
                // complete the *first* operator-requested connection.
                if carrier != Carrier::Up && self.attempt_seen && !self.auto_reconnect {
                    self.activation_armed = false;
                }
                self.choose_phase();
            }
            Event::WifiAuthenticated { generation } => {
                if self.medium != Medium::Wifi || generation != self.generation ||
                   !self.operator_enabled || self.carrier != Carrier::Up ||
                   self.phase != Phase::WaitingAuthentication {
                    return Ok(false)
                }
                self.wifi_authenticated = true;
                self.phase = Phase::Configuring;
            }
            Event::WifiAuthenticationFailed { generation } => {
                if self.medium != Medium::Wifi || generation != self.generation ||
                   self.phase != Phase::WaitingAuthentication {
                    return Ok(false)
                }
                if !self.auto_reconnect { self.activation_armed = false; }
                self.phase = Phase::Error;
            }
            Event::ConfigurationSucceeded { generation } => {
                if generation != self.generation || self.phase != Phase::Configuring ||
                   !self.can_configure() {
                    return Ok(false)
                }
                self.address_valid = true;
                self.route_valid = true;
                self.dns_valid = true;
                self.phase = Phase::LanReady;
            }
            Event::ConfigurationFailed { generation } => {
                if generation != self.generation || self.phase != Phase::Configuring {
                    return Ok(false)
                }
                self.invalidate();
                if !self.auto_reconnect { self.activation_armed = false; }
                self.phase = Phase::Error;
            }
            Event::ReachabilitySucceeded { generation } => {
                if generation != self.generation || !self.can_configure() ||
                   !matches!(self.phase, Phase::LanReady | Phase::Limited) ||
                   !self.address_valid || !self.route_valid {
                    return Ok(false)
                }
                self.phase = Phase::InternetConfirmed;
            }
            Event::ReachabilityFailed { generation } => {
                if generation != self.generation || !self.can_configure() ||
                   !matches!(self.phase, Phase::LanReady | Phase::InternetConfirmed) {
                    return Ok(false)
                }
                self.phase = Phase::Limited;
            }
            Event::SuspendOrResume => {
                self.bump()?;
                self.invalidate();
                self.carrier = Carrier::Unknown;
                if self.attempt_seen && !self.auto_reconnect {
                    self.activation_armed = false;
                }
                self.choose_phase();
            }
        }
        assert_eq!(self.firewall_policy_revision, previous.firewall_policy_revision);
        if !self.operator_enabled {
            assert!(!self.address_valid && !self.route_valid && !self.dns_valid);
            assert_eq!(self.phase, Phase::Disabled);
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ethernet_manual_disconnect_must_not_auto_reconnect_when_carrier_returns() {
        let mut n = Network::new(Medium::Ethernet, 77);
        assert_eq!(n.phase, Phase::Disabled);
        n.step(Event::OperatorConnect { allow_auto_reconnect: true }).unwrap();
        assert_eq!(n.phase, Phase::WaitingLink);
        n.step(Event::ObservedCarrier(Carrier::Up)).unwrap();
        assert_eq!(n.phase, Phase::Configuring);
        let g = n.generation;
        n.step(Event::ConfigurationSucceeded { generation: g }).unwrap();
        assert_eq!(n.phase, Phase::LanReady); // LAN is not proof of Internet
        n.step(Event::ReachabilitySucceeded { generation: g }).unwrap();
        assert_eq!(n.phase, Phase::InternetConfirmed);
        n.step(Event::OperatorDisconnect).unwrap();
        assert_eq!(n.phase, Phase::Disabled);
        assert!(!n.auto_reconnect && !n.operator_enabled && !n.address_valid);
        n.step(Event::ObservedCarrier(Carrier::Down)).unwrap();
        n.step(Event::ObservedCarrier(Carrier::Up)).unwrap();
        assert_eq!(n.phase, Phase::Disabled);
        assert_eq!(n.firewall_policy_revision, 77);
    }

    #[test]
    fn interrupted_config_response_cannot_resurrect_obsolete_lease_or_old_socket() {
        let mut n = Network::new(Medium::Ethernet, 77);
        n.step(Event::OperatorConnect { allow_auto_reconnect: true }).unwrap();
        n.step(Event::ObservedCarrier(Carrier::Up)).unwrap();
        let old = n.generation;
        n.step(Event::ObservedCarrier(Carrier::Down)).unwrap();
        assert_eq!(n.phase, Phase::WaitingLink);
        assert!(!n.step(Event::ConfigurationSucceeded { generation: old }).unwrap());
        n.step(Event::ObservedCarrier(Carrier::Up)).unwrap();
        assert_eq!(n.phase, Phase::Configuring);
        let current = n.generation;
        assert_ne!(current, old);
        n.step(Event::ConfigurationSucceeded { generation: current }).unwrap();
        assert!(n.address_valid);
        assert!(!n.step(Event::ReachabilitySucceeded { generation: old }).unwrap());
        assert_eq!(n.phase, Phase::LanReady);
        assert_eq!(n.firewall_policy_revision, 77);
    }

    #[test]
    fn wifi_carrier_is_not_authentication_and_failed_auth_is_not_connected() {
        let mut n = Network::new(Medium::Wifi, 10);
        n.step(Event::OperatorConnect { allow_auto_reconnect: true }).unwrap();
        n.step(Event::ObservedCarrier(Carrier::Up)).unwrap();
        assert_eq!(n.phase, Phase::WaitingAuthentication);
        let g = n.generation;
        assert!(!n.step(Event::ConfigurationSucceeded { generation: g }).unwrap());
        n.step(Event::WifiAuthenticationFailed { generation: g }).unwrap();
        assert_eq!(n.phase, Phase::Error);
        n.step(Event::OperatorConnect { allow_auto_reconnect: true }).unwrap();
        let current = n.generation;
        assert_eq!(n.phase, Phase::WaitingAuthentication);
        n.step(Event::WifiAuthenticated { generation: current }).unwrap();
        n.step(Event::ConfigurationSucceeded { generation: current }).unwrap();
        assert_eq!(n.phase, Phase::LanReady);
        assert_eq!(n.firewall_policy_revision, 10);
    }

    #[test]
    fn resume_never_resurrects_old_address_dns_or_internet_claim() {
        let mut n = Network::new(Medium::Ethernet, 99);
        n.step(Event::OperatorConnect { allow_auto_reconnect: true }).unwrap();
        n.step(Event::ObservedCarrier(Carrier::Up)).unwrap();
        let first = n.generation;
        n.step(Event::ConfigurationSucceeded { generation: first }).unwrap();
        n.step(Event::ReachabilitySucceeded { generation: first }).unwrap();
        n.step(Event::SuspendOrResume).unwrap();
        assert_eq!(n.phase, Phase::WaitingLink);
        assert!(!n.address_valid && !n.route_valid && !n.dns_valid);
        assert!(!n.step(Event::ReachabilitySucceeded { generation: first }).unwrap());
    }

    #[test]
    fn manual_only_link_loss_requires_new_user_connect_even_if_carrier_comes_back() {
        let mut n = Network::new(Medium::Ethernet, 44);
        n.step(Event::OperatorConnect { allow_auto_reconnect: false }).unwrap();
        n.step(Event::ObservedCarrier(Carrier::Up)).unwrap();
        let first = n.generation;
        n.step(Event::ConfigurationSucceeded { generation: first }).unwrap();
        n.step(Event::ReachabilitySucceeded { generation: first }).unwrap();
        assert_eq!(n.phase, Phase::InternetConfirmed);

        n.step(Event::ObservedCarrier(Carrier::Down)).unwrap();
        assert_eq!(n.phase, Phase::WaitingManual);
        assert!(n.operator_enabled && !n.activation_armed);
        n.step(Event::ObservedCarrier(Carrier::Up)).unwrap();
        assert_eq!(n.phase, Phase::WaitingManual);
        let stale = n.generation;
        assert!(!n.step(Event::ConfigurationSucceeded { generation: stale }).unwrap());
        assert!(!n.step(Event::ReachabilitySucceeded { generation: stale }).unwrap());
        assert!(!n.address_valid);

        n.step(Event::OperatorConnect { allow_auto_reconnect: false }).unwrap();
        assert_eq!(n.phase, Phase::Configuring);
        let fresh = n.generation;
        n.step(Event::ConfigurationSucceeded { generation: fresh }).unwrap();
        n.step(Event::ReachabilitySucceeded { generation: fresh }).unwrap();
        assert_eq!(n.phase, Phase::InternetConfirmed);
        assert_eq!(n.firewall_policy_revision, 44);
    }

    #[test]
    fn manual_only_resume_requires_user_action_but_initial_down_can_connect_once() {
        let mut n = Network::new(Medium::Wifi, 11);
        n.step(Event::OperatorConnect { allow_auto_reconnect: false }).unwrap();
        n.step(Event::ObservedCarrier(Carrier::Down)).unwrap();
        assert_eq!(n.phase, Phase::WaitingLink); // first attempt has not started
        n.step(Event::ObservedCarrier(Carrier::Up)).unwrap();
        assert_eq!(n.phase, Phase::WaitingAuthentication);
        let first = n.generation;
        n.step(Event::WifiAuthenticated { generation: first }).unwrap();
        n.step(Event::ConfigurationSucceeded { generation: first }).unwrap();
        assert_eq!(n.phase, Phase::LanReady);

        n.step(Event::SuspendOrResume).unwrap();
        assert_eq!(n.phase, Phase::WaitingManual);
        assert!(!n.activation_armed);
        n.step(Event::ObservedCarrier(Carrier::Up)).unwrap();
        assert_eq!(n.phase, Phase::WaitingManual);
        let after_resume = n.generation;
        assert!(!n.step(Event::WifiAuthenticated { generation: after_resume }).unwrap());
        n.step(Event::OperatorConnect { allow_auto_reconnect: false }).unwrap();
        assert_eq!(n.phase, Phase::WaitingAuthentication);
        let renewed = n.generation;
        assert!(n.step(Event::WifiAuthenticated { generation: renewed }).unwrap());
        assert!(n.step(Event::ConfigurationSucceeded { generation: renewed }).unwrap());
        assert_eq!(n.firewall_policy_revision, 11);
    }

    #[test]
    fn offline_lan_can_be_ready_even_when_internet_reachability_fails() {
        let mut n = Network::new(Medium::Ethernet, 3);
        n.step(Event::OperatorConnect { allow_auto_reconnect: false }).unwrap();
        n.step(Event::ObservedCarrier(Carrier::Up)).unwrap();
        let g = n.generation;
        n.step(Event::ConfigurationSucceeded { generation: g }).unwrap();
        n.step(Event::ReachabilityFailed { generation: g }).unwrap();
        assert_eq!(n.phase, Phase::Limited);
        assert!(n.address_valid && n.route_valid && n.dns_valid);
        assert_eq!(n.firewall_policy_revision, 3);
    }
}
