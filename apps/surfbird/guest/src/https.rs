//! One TLS 1.3 request from the freestanding guest to an official public watch page.
extern crate alloc;
#[path = "p256_wire.rs"]
mod p256_wire;
use super::Stream;
use alloc::sync::Arc;
use core::{
    alloc::{GlobalAlloc, Layout},
    cell::UnsafeCell,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};
use pcp_game_guest::{call, load, log, sleep_us, time_us};
use rustls::{
    client::UnbufferedClientConnection, pki_types::UnixTime, time_provider::TimeProvider,
    unbuffered::ConnectionState, ClientConfig, RootCertStore,
};

pub(super) const HOST: &str = "www.youtube.com";
const REQUEST: &[u8] = b"GET /watch?v=M7lc1UVf-VE HTTP/1.1\r\nHost: www.youtube.com\r\nUser-Agent: Surfbird-native-HTTPS-probe/1\r\nAccept: text/html\r\nConnection: close\r\n\r\n";
const CAPACITY: usize = 3 * 1024 * 1024;
#[repr(C, align(4096))]
struct Heap {
    bytes: UnsafeCell<[u8; CAPACITY]>,
    next: AtomicUsize,
}
unsafe impl Sync for Heap {}
unsafe impl GlobalAlloc for Heap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.align() > 4096 {
            return core::ptr::null_mut();
        }
        let align = layout.align() - 1;
        match self
            .next
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |old| {
                ((old + align) & !align)
                    .checked_add(layout.size())
                    .filter(|end| *end <= CAPACITY)
            }) {
            Ok(old) => unsafe { self.bytes.get().cast::<u8>().add((old + align) & !align) },
            Err(_) => core::ptr::null_mut(),
        }
    }
    unsafe fn dealloc(&self, _: *mut u8, _: Layout) {}
}
// This single-request program frees its entire arena at process exit.
#[global_allocator]
static ARENA: Heap = Heap {
    bytes: UnsafeCell::new([0; CAPACITY]),
    next: AtomicUsize::new(0),
};
#[derive(Debug)]
struct BootClock;
impl TimeProvider for BootClock {
    fn current_time(&self) -> Option<UnixTime> {
        let mut t = [0i64; 2];
        if call(228, 0, t.as_mut_ptr() as usize, 0, 0, 0, 0) != 0 || t[0] <= 0 {
            return None;
        }
        Some(UnixTime::since_unix_epoch(Duration::from_secs(t[0] as u64)))
    }
}
static mut INCOMING: [u8; 32768] = [0; 32768];
static mut OUTGOING: [u8; 32768] = [0; 32768];

fn tls_error(e: rustls::Error) -> &'static str {
    use rustls::{CertificateError as C, Error};
    match e {
        Error::InvalidCertificate(C::UnknownIssuer) => "TLS unknown issuer",
        Error::InvalidCertificate(C::NotValidForNameContext { .. } | C::NotValidForName) => {
            "TLS hostname mismatch"
        }
        Error::InvalidCertificate(_) => "TLS invalid certificate",
        Error::FailedToGetRandomBytes => "TLS entropy unavailable",
        Error::FailedToGetCurrentTime => "TLS trusted clock unavailable",
        _ => "TLS negotiation failed",
    }
}

pub(super) fn watch(stream: &mut impl Stream) -> Result<(), &'static str> {
    let mut mode = [0];
    if !load(b"tls.cfg\0", &mut mode) || mode[0] > 2 {
        return Err("invalid HTTPS probe configuration");
    }
    let mut roots = RootCertStore::empty();
    if mode[0] != 2 {
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    }
    let mut provider = rustls_rustcrypto::provider();
    // The prerelease provider's X25519 completion does not reject all-zero shared
    // secrets as RFC 8446 section 7.4.2 requires. Its P-256 path validates SEC1 points.
    p256_wire::restrict(&mut provider)?;
    let provider = Arc::new(provider);
    let roots = Arc::new(roots);
    let wrong_name_check = if mode[0] == 1 {
        Some(
            rustls::client::WebPkiServerVerifier::builder_with_provider(
                roots.clone(),
                provider.clone(),
            )
            .build()
            .map_err(|_| "TLS verifier setup failed")?,
        )
    } else {
        None
    };
    let config = ClientConfig::builder_with_details(provider, Arc::new(BootClock))
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(tls_error)?
        .with_root_certificates(roots)
        .with_no_client_auth();
    let mut tls = UnbufferedClientConnection::new(
        Arc::new(config),
        HOST.try_into().map_err(|_| "invalid TLS name")?,
    )
    .map_err(tls_error)?;
    let input = unsafe { &mut *(&raw mut INCOMING) };
    let output = unsafe { &mut *(&raw mut OUTGOING) };
    let (mut have, mut encoded) = (0, 0);
    let (mut requested, mut body) = (false, false);
    let mut http = [0u8; 32768];
    let mut http_len = 0;
    let deadline = time_us() + 25_000_000;
    while time_us() < deadline {
        let status = tls.process_tls_records(&mut input[..have]);
        let mut discard = status.discard;
        let mut need_read = false;
        match status.state.map_err(tls_error)? {
            ConnectionState::EncodeTlsData(mut rec) => {
                encoded += rec
                    .encode(&mut output[encoded..])
                    .map_err(|_| "TLS encode overflow")?;
            }
            ConnectionState::TransmitTlsData(rec) => {
                super::send(stream, &output[..encoded], deadline)?;
                rec.done();
                encoded = 0;
            }
            ConnectionState::BlockedHandshake => {
                need_read = true;
            }
            ConnectionState::WriteTraffic(mut writer) if !requested => {
                if let Some(verifier) = &wrong_name_check {
                    use rustls::client::danger::ServerCertVerifier;
                    let chain = tls.peer_certificates().ok_or("missing peer chain")?;
                    let (leaf, issuers) = chain.split_first().ok_or("empty peer chain")?;
                    let wrong_name = "wrong-host.invalid"
                        .try_into()
                        .map_err(|_| "invalid reference name")?;
                    let now = BootClock
                        .current_time()
                        .ok_or("TLS trusted clock unavailable")?;
                    // Keep SNI, the real peer chain, configured roots and time the same.
                    // Challenge the stock verifier with a different reference name before any GET.
                    verifier
                        .verify_server_cert(leaf, issuers, &wrong_name, &[], now)
                        .map_err(tls_error)?;
                    return Err("wrong hostname unexpectedly accepted");
                }
                log(b"SURFBIRD certificate chain and hostname verified\n");
                let len = writer
                    .encrypt(REQUEST, output)
                    .map_err(|_| "TLS request overflow")?;
                super::send(stream, &output[..len], deadline)?;
                requested = true;
            }
            ConnectionState::WriteTraffic(_) => {
                need_read = true;
            }
            ConnectionState::ReadTraffic(mut reader) => {
                while let Some(record) = reader.next_record() {
                    let rec = record.map_err(tls_error)?;
                    discard += rec.discard;
                    let n = rec.payload.len().min(http.len() - http_len);
                    http[http_len..http_len + n].copy_from_slice(&rec.payload[..n]);
                    http_len += n;
                    if let Some((status, markup)) =
                        surfbird_cable::https_prefix::read(&http[..http_len])?
                    {
                        log(b"SURFBIRD HTTPS ");
                        log(status);
                        log(b"\nSURFBIRD body ");
                        log(markup);
                        log(b"\nSURFBIRD authenticated HTML body prefix received\n");
                        body = true;
                        break;
                    }
                    if http_len == http.len() {
                        return Err("HTTPS response headers too large");
                    }
                }
            }
            _ => return Err("TLS ended before HTTP body"),
        }
        if body {
            return Ok(());
        }
        if discard > have {
            return Err("invalid TLS input accounting");
        }
        input.copy_within(discard..have, 0);
        have -= discard;
        if need_read {
            if have == input.len() {
                return Err("TLS input overflow");
            }
            loop {
                stream.poll()?;
                let n = stream.read(&mut input[have..])?;
                if n > 0 {
                    have += n;
                    break;
                }
                if time_us() >= deadline {
                    return Err("HTTPS receive timeout");
                }
                sleep_us(1000);
            }
        }
    }
    Err("HTTPS timeout")
}
