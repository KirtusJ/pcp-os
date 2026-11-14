//! The guest process owns ARP, IPv4, the TCP state machine and its socket buffers.
//! The kernel device only copies bounded Ethernet frames.
#[cfg(feature = "dns")]
#[path = "dns.rs"]
mod dns;
use super::{browse_pages, Stream};
use pcp_game_guest::{call, load, open, sleep_us, time_us};
use smoltcp::{
    iface::{Config, Interface, SocketHandle, SocketSet, SocketStorage},
    phy::{Device, DeviceCapabilities, RxToken, TxToken},
    socket::tcp,
    time::Instant,
    wire::{EthernetAddress, IpAddress, IpCidr},
};

const MTU: usize = 1514;
fn now() -> Instant {
    Instant::from_micros(time_us() as i64)
}

struct Ethernet {
    fd: usize,
    rx: [u8; MTU],
    failed: bool,
}
impl Ethernet {
    fn poll(&mut self, iface: &mut Interface, sockets: &mut SocketSet) -> Result<(), &'static str> {
        let timestamp = now();
        iface.poll_maintenance(timestamp);
        iface.poll_ingress_single(timestamp, self, sockets);
        iface.poll_egress(timestamp, self, sockets);
        if self.failed {
            return Err("Ethernet I/O failed");
        }
        Ok(())
    }
}
struct Rx<'a>(&'a [u8]);
impl RxToken for Rx<'_> {
    fn consume<R, F: FnOnce(&[u8]) -> R>(self, f: F) -> R {
        f(self.0)
    }
}
struct Tx<'a> {
    fd: usize,
    failed: &'a mut bool,
}
impl TxToken for Tx<'_> {
    fn consume<R, F: FnOnce(&mut [u8]) -> R>(self, len: usize, f: F) -> R {
        let mut packet = [0; MTU];
        let packet = &mut packet[..len];
        let result = f(packet);
        if call(1, self.fd, packet.as_ptr() as usize, len, 0, 0, 0) != len as isize {
            *self.failed = true;
        }
        result
    }
}
impl Device for Ethernet {
    type RxToken<'a> = Rx<'a>;
    type TxToken<'a> = Tx<'a>;
    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.max_transmission_unit = MTU;
        caps
    }
    fn receive(&mut self, _: Instant) -> Option<(Rx<'_>, Tx<'_>)> {
        let n = call(0, self.fd, self.rx.as_mut_ptr() as usize, MTU, 0, 0, 0);
        if n == -11 {
            return None;
        }
        if !(14..=MTU as isize).contains(&n) {
            self.failed = true;
            return None;
        }
        Some((
            Rx(&self.rx[..n as usize]),
            Tx {
                fd: self.fd,
                failed: &mut self.failed,
            },
        ))
    }
    fn transmit(&mut self, _: Instant) -> Option<Tx<'_>> {
        Some(Tx {
            fd: self.fd,
            failed: &mut self.failed,
        })
    }
}
struct Connection<'a> {
    device: Ethernet,
    iface: Interface,
    sockets: SocketSet<'a>,
    handle: SocketHandle,
}
impl Stream for Connection<'_> {
    fn poll(&mut self) -> Result<(), &'static str> {
        self.device.poll(&mut self.iface, &mut self.sockets)
    }
    fn write(&mut self, bytes: &[u8]) -> Result<usize, &'static str> {
        self.sockets
            .get_mut::<tcp::Socket>(self.handle)
            .send_slice(bytes)
            .map_err(|_| "TCP send closed")
    }
    fn read(&mut self, bytes: &mut [u8]) -> Result<usize, &'static str> {
        self.sockets
            .get_mut::<tcp::Socket>(self.handle)
            .recv_slice(bytes)
            .map_err(|_| "TCP receive closed")
    }
}

pub(super) fn browse() -> Result<(), &'static str> {
    let fd = open(b"/dev/ethernet\0", 2 | 0x800);
    if fd < 0 {
        return Err("Ethernet grant unavailable");
    }
    #[cfg(feature = "probe-ethernet-modes")]
    {
        for mode in [0, 1, 3] {
            if open(b"/dev/ethernet\0", mode | 0x800) != -22 {
                return Err("unexpected Ethernet open mode result");
            }
        }
        pcp_game_guest::log(b"SURFBIRD Ethernet open modes verified\n");
    }
    let mut boot = [0; 12];
    if !load(b"net.cfg\0", &mut boot) || boot[..2] == [0, 0] {
        return Err("invalid fixture network configuration");
    }
    let port = u16::from_be_bytes([boot[0], boot[1]]);
    let seed = u64::from_be_bytes(boot[2..10].try_into().unwrap());
    let port_random = u16::from_be_bytes([boot[10], boot[11]]);
    let mut device = Ethernet {
        fd: fd as usize,
        rx: [0; MTU],
        failed: false,
    };
    let mut config = Config::new(EthernetAddress([2, 0, 0, 0, 0, 1]).into());
    config.random_seed = seed ^ time_us();
    let mut iface = Interface::new(config, &mut device, now());
    iface.update_ip_addrs(|a| {
        a.push(IpCidr::new(IpAddress::v4(10, 0, 2, 15), 24))
            .unwrap()
    });
    #[cfg(feature = "dns")]
    let destination = dns::resolve(super::ORIGIN_HOST, &mut device, &mut iface)?;
    #[cfg(not(feature = "dns"))]
    let destination = smoltcp::wire::Ipv4Address::new(10, 0, 2, 2);
    let mut rx = [0; 4096];
    let mut tx = [0; 1024];
    let mut socket = tcp::Socket::new(
        tcp::SocketBuffer::new(&mut rx[..]),
        tcp::SocketBuffer::new(&mut tx[..]),
    );
    socket.set_nagle_enabled(false);
    socket
        .connect(
            iface.context(),
            (destination, port),
            49152 + (port_random.wrapping_add(time_us() as u16) % 16384),
        )
        .map_err(|_| "TCP connect rejected")?;
    let mut storage = [SocketStorage::EMPTY];
    let mut sockets = SocketSet::new(&mut storage[..]);
    let handle = sockets.add(socket);
    let mut conn = Connection {
        device,
        iface,
        sockets,
        handle,
    };
    let deadline = time_us() + 8_000_000;
    loop {
        conn.poll()?;
        match conn.sockets.get::<tcp::Socket>(handle).state() {
            tcp::State::Established => break,
            tcp::State::Closed => return Err("TCP connection refused"),
            _ if time_us() >= deadline => return Err("TCP connect timeout"),
            _ => sleep_us(1000),
        }
    }
    pcp_game_guest::log(b"SURFBIRD TCP established\n");
    let result = browse_pages(&mut conn);
    conn.sockets.get_mut::<tcp::Socket>(handle).close();
    let deadline = time_us() + 1_000_000;
    while time_us() < deadline {
        conn.poll()?;
        if matches!(
            conn.sockets.get::<tcp::Socket>(handle).state(),
            tcp::State::TimeWait | tcp::State::Closed
        ) {
            break;
        }
        sleep_us(1000);
    }
    result
}
