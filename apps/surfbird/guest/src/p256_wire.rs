//! Enforce the TLS 1.3 uncompressed P-256 key-share format before delegating ECDH.
extern crate alloc;
use alloc::boxed::Box;
use rustls::{
    crypto::{ActiveKeyExchange, CryptoProvider, SharedSecret, SupportedKxGroup},
    Error, NamedGroup, PeerMisbehaved,
};

#[derive(Debug)]
struct Group(&'static dyn SupportedKxGroup);
struct Active(Box<dyn ActiveKeyExchange>);

fn validate(point: &[u8]) -> Result<(), Error> {
    if point.len() != 65 || point[0] != 4 {
        return Err(PeerMisbehaved::InvalidKeyShare.into());
    }
    Ok(())
}

impl SupportedKxGroup for Group {
    fn name(&self) -> NamedGroup {
        NamedGroup::secp256r1
    }
    fn start(&self) -> Result<Box<dyn ActiveKeyExchange>, Error> {
        let active = self.0.start()?;
        validate(active.pub_key())?;
        Ok(Box::new(Active(active)))
    }
}
impl ActiveKeyExchange for Active {
    fn group(&self) -> NamedGroup {
        NamedGroup::secp256r1
    }
    fn pub_key(&self) -> &[u8] {
        self.0.pub_key()
    }
    fn complete(self: Box<Self>, peer: &[u8]) -> Result<SharedSecret, Error> {
        validate(peer)?;
        self.0.complete(peer)
    }
}

pub(super) fn restrict(provider: &mut CryptoProvider) -> Result<(), &'static str> {
    let group = provider
        .kx_groups
        .iter()
        .copied()
        .find(|g| g.name() == NamedGroup::secp256r1)
        .ok_or("TLS provider missing P-256")?;
    // CryptoProvider expects a process-lifetime group. This guest exits after one request.
    provider.kx_groups.clear();
    provider.kx_groups.push(Box::leak(Box::new(Group(group))));
    Ok(())
}
