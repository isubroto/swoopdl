//! Options → Proxy / Socks → Network adapter (#35): every connection SDL opens goes out through
//! the chosen adapter (a VPN's, usually), or not at all.
//!
//! A socket bound to the adapter before it connects can't leave through another one: the
//! operating system refuses to send its packets elsewhere. Binding therefore *is* the kill
//! switch, and it fails closed: when the adapter is gone, has lost its address, or has no route
//! to a server, the connection fails instead of taking the normal one.
//!
//! - **Windows** binds the adapter's own address (`local_address`). From Vista on, Windows uses
//!   the strong host model: a packet with an adapter's source address only leaves through that
//!   adapter. hyper (under reqwest) binds one address only, so a client bound to an IPv4 address
//!   must never be handed an IPv6 one (it would bind `[::]` and take the normal route): the
//!   resolver keeps only the bound family, and IP literals in addresses, proxies and redirects are
//!   checked ([`Binding::allows`]).
//! - **Linux** binds the device (`SO_BINDTODEVICE`, allowed without privileges since Linux 5.7).
//!   An address alone would not be enough there: Linux routes by destination (weak host model), so
//!   a packet with the VPN's address could still leave through the Ethernet adapter.
//! - **macOS** binds the interface index (`IP_BOUND_IF` / `IPV6_BOUND_IF`).
//!
//! Connections to this computer itself (127.0.0.0/8, ::1) are never bound: they don't leave it,
//! and binding them fails. A proxy on this computer therefore carries downloads unbound; where
//! its traffic goes is up to it.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

/// How SDL's connections leave this computer.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum Route {
    /// No adapter chosen: the system decides (the default).
    #[default]
    Any,
    /// Through this adapter only.
    Bound(Arc<Binding>),
    /// An adapter is chosen but it isn't there or has no address: nothing may connect.
    Down { label: String },
}

/// The chosen adapter as it is now.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Binding {
    /// Its name as the system shows it ("Ethernet", "ProtonVPN", "wg0"), for messages.
    pub label: String,
    /// The system's name for it: what Linux binds (`wg0`).
    pub name: String,
    /// Its interface index: what macOS binds.
    pub index: u32,
    pub v4: Option<Ipv4Addr>,
    pub v6: Option<Ipv6Addr>,
    /// Tests only: bind connections to this computer too (they are otherwise left unbound).
    #[doc(hidden)]
    pub loopback_too: bool,
}

impl Route {
    pub fn is_any(&self) -> bool {
        matches!(self, Route::Any)
    }

    /// The adapter's name, when one is chosen.
    pub fn label(&self) -> Option<&str> {
        match self {
            Route::Any => None,
            Route::Bound(b) => Some(&b.label),
            Route::Down { label } => Some(label),
        }
    }

    /// Whether a connection to `ip` may be opened (bound, or to this computer); else why not.
    pub fn check(&self, ip: IpAddr) -> Result<(), NetError> {
        match self {
            Route::Any => Ok(()),
            Route::Down { label } => Err(NetError::Down {
                label: label.clone(),
            }),
            Route::Bound(b) if b.allows(ip) => Ok(()),
            Route::Bound(b) => Err(NetError::Family {
                label: b.label.clone(),
                ip,
            }),
        }
    }

    /// `ips` without the addresses this route can't reach (the other IP family).
    pub fn keep(&self, ips: Vec<IpAddr>) -> Vec<IpAddr> {
        match self {
            Route::Bound(b) => ips.into_iter().filter(|ip| b.allows(*ip)).collect(),
            _ => ips,
        }
    }
}

impl Binding {
    /// Whether a connection to `dst` can go through this adapter: a connection to this computer
    /// always can (it isn't bound); otherwise the adapter needs an address of `dst`'s family.
    /// On Windows only the family that is bound counts (see the module's notes).
    pub fn allows(&self, dst: IpAddr) -> bool {
        if self.exempt(dst) {
            return true;
        }
        if cfg!(windows) {
            return self
                .windows_local()
                .is_some_and(|l| l.is_ipv4() == dst.is_ipv4());
        }
        match dst {
            IpAddr::V4(_) => self.v4.is_some(),
            IpAddr::V6(_) => self.v6.is_some(),
        }
    }

    /// A connection to this computer, left unbound.
    fn exempt(&self, dst: IpAddr) -> bool {
        dst.is_loopback() && !self.loopback_too
    }

    /// The one address a Windows client binds: IPv4 when the adapter has one.
    fn windows_local(&self) -> Option<IpAddr> {
        self.v4.map(IpAddr::V4).or(self.v6.map(IpAddr::V6))
    }

    /// Bind `socket` (not yet connected) so that it can only reach `dst` through this adapter.
    fn pin(&self, socket: &socket2::Socket, dst: SocketAddr) -> std::io::Result<()> {
        // Windows: the strong host model makes the source address enough.
        #[cfg(windows)]
        {
            let local = match dst {
                SocketAddr::V4(_) => self.v4.map(IpAddr::V4),
                SocketAddr::V6(_) => self.v6.map(IpAddr::V6),
            }
            .ok_or_else(|| std::io::Error::other("no address of that family"))?;
            socket.bind(&SocketAddr::new(local, 0).into())
        }
        // Linux: the device itself (an address alone would not hold, see the module's notes).
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            let _ = dst;
            socket.bind_device(Some(self.name.as_bytes()))
        }
        // macOS: the interface by index.
        #[cfg(target_os = "macos")]
        {
            let index = std::num::NonZeroU32::new(self.index)
                .ok_or_else(|| std::io::Error::other("no interface index"))?;
            match dst {
                SocketAddr::V4(_) => socket.bind_device_by_index_v4(Some(index)),
                SocketAddr::V6(_) => socket.bind_device_by_index_v6(Some(index)),
            }
        }
        // Elsewhere there is no way to hold a socket to an adapter that SDL knows of: refuse.
        #[cfg(not(any(
            windows,
            target_os = "linux",
            target_os = "android",
            target_os = "macos"
        )))]
        {
            let _ = (socket, dst);
            Err(std::io::Error::other(
                "binding to an adapter isn't supported on this system",
            ))
        }
    }
}

/// Why a connection was not opened.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum NetError {
    #[error("the network adapter {label} is not connected")]
    Down { label: String },
    #[error("{ip} can't be reached through {label}: it has no address of that kind (IPv4 or IPv6)")]
    Family { label: String, ip: IpAddr },
    /// A server on this computer redirected elsewhere: that connection would not be bound.
    #[error(
        "a server on this computer sent the download elsewhere, which would not go through {label}"
    )]
    Unbound { label: String },
    /// `what` can't be held to an adapter yet: refused rather than sent around it.
    #[error("{what} can't go through a chosen network adapter ({label}) yet")]
    NotSupported { label: String, what: String },
}

/// Whether `url` may be fetched through `route`: an IP address in it must be of a family the
/// adapter can reach (a host name is checked by the resolver).
pub fn check_url(route: &Route, url: &url::Url) -> Result<(), NetError> {
    match url.host() {
        Some(url::Host::Ipv4(ip)) => route.check(IpAddr::V4(ip)),
        Some(url::Host::Ipv6(ip)) => route.check(IpAddr::V6(ip)),
        _ => match route {
            Route::Down { label } => Err(NetError::Down {
                label: label.clone(),
            }),
            _ => Ok(()),
        },
    }
}

/// The system's proxy for `url` (Windows' or macOS' settings, or the environment's
/// `HTTPS_PROXY`…), as reqwest would use it; None: straight to the server.
pub fn system_proxy(url: &url::Url) -> Option<String> {
    let uri: http::Uri = url.as_str().parse().ok()?;
    hyper_util::client::proxy::matcher::Matcher::from_system()
        .intercept(&uri)
        .map(|i| i.uri().to_string())
}

/// Crawl-only, fallible counterpart to `system_proxy`. The ordinary download
/// contract stays unchanged. A missing interception is DIRECT only after checking
/// configuration that the matcher would otherwise silently ignore.
pub(crate) fn crawl_system_direct(url: &url::Url) -> Result<bool, String> {
    let uri: http::Uri = url
        .as_str()
        .parse()
        .map_err(|_| "System routing could not parse the destination")?;
    if std::env::var_os("REQUEST_METHOD").is_some() {
        return Err("System routing cannot verify proxy settings in a CGI environment".into());
    }
    let mut settings = CrawlProxySettings {
        all: proxy_setting(&["ALL_PROXY", "all_proxy"])?,
        http: proxy_setting(&["HTTP_PROXY", "http_proxy"])?,
        https: proxy_setting(&["HTTPS_PROXY", "https_proxy"])?,
        no: proxy_setting(&["NO_PROXY", "no_proxy"])?,
    };
    settings.read_platform(url)?;
    settings.direct(&uri)
}

fn proxy_setting(names: &[&str]) -> Result<String, String> {
    for name in names {
        match std::env::var(name) {
            Ok(value) => return Ok(value),
            Err(std::env::VarError::NotPresent) => {}
            Err(_) => return Err("System routing has an unreadable proxy setting".into()),
        }
    }
    Ok(String::new())
}

#[derive(Default)]
struct CrawlProxySettings {
    all: String,
    http: String,
    https: String,
    no: String,
}
impl CrawlProxySettings {
    fn direct(&self, uri: &http::Uri) -> Result<bool, String> {
        use hyper_util::client::proxy::matcher::Matcher;
        for value in [&self.all, &self.http, &self.https] {
            if !value.is_empty()
                && Matcher::builder()
                    .all(value.as_str())
                    .build()
                    .intercept(uri)
                    .is_none()
            {
                return Err("System routing has an invalid or unsupported proxy setting".into());
            }
        }
        Ok(Matcher::builder()
            .all(self.all.as_str())
            .http(self.http.as_str())
            .https(self.https.as_str())
            .no(self.no.as_str())
            .build()
            .intercept(uri)
            .is_none())
    }
    fn read_platform(&mut self, _url: &url::Url) -> Result<(), String> {
        // The pinned matcher does not expose macOS PAC/discovery settings.
        #[cfg(target_os = "macos")]
        {
            Err("System routing cannot verify automatic proxy settings on macOS; select explicit direct routing".into())
        }
        #[cfg(not(target_os = "macos"))]
        {
            #[cfg(windows)]
            read_windows_crawl_proxy(self, _url)?;
            Ok(())
        }
    }
}

#[cfg(windows)]
#[allow(unsafe_code)] // WinHTTP allocates three strings; this function reads and frees them.
fn read_windows_crawl_proxy(
    settings: &mut CrawlProxySettings,
    url: &url::Url,
) -> Result<(), String> {
    use windows_sys::Win32::{
        Foundation::GlobalFree,
        Networking::WinHttp::{
            WINHTTP_CURRENT_USER_IE_PROXY_CONFIG, WinHttpGetIEProxyConfigForCurrentUser,
        },
    };
    let mut config = WINHTTP_CURRENT_USER_IE_PROXY_CONFIG::default();
    fn copy_wide(pointer: *const u16) -> Result<String, String> {
        if pointer.is_null() {
            return Ok(String::new());
        }
        let mut len = 0;
        // Successful WinHTTP calls guarantee valid NUL-terminated strings.
        while unsafe { *pointer.add(len) } != 0 {
            len += 1;
        }
        String::from_utf16(unsafe { std::slice::from_raw_parts(pointer, len) })
            .map_err(|_| "System routing has an unreadable Windows proxy setting".into())
    }
    // WinHTTP returns three strings that the caller must release with GlobalFree.
    let loaded = unsafe { WinHttpGetIEProxyConfigForCurrentUser(&mut config) } != 0;
    let result: Result<(bool, String, String, String), String> = (|| {
        if !loaded {
            return Err("System routing could not read Windows proxy settings".into());
        }
        Ok((
            config.fAutoDetect != 0,
            copy_wide(config.lpszAutoConfigUrl)?,
            copy_wide(config.lpszProxy)?,
            copy_wide(config.lpszProxyBypass)?,
        ))
    })();
    for pointer in [
        config.lpszAutoConfigUrl,
        config.lpszProxy,
        config.lpszProxyBypass,
    ] {
        if !pointer.is_null() {
            unsafe {
                GlobalFree(pointer.cast());
            }
        }
    }
    let (auto_detect, pac, proxy, bypass) = result?;
    if !pac.is_empty() {
        return Err(
            "System routing uses an explicit PAC script; select direct routing for Site Grabber"
                .into(),
        );
    }
    if auto_detect && !windows_auto_proxy_direct(url)? {
        return Err("System routing selected a proxy for this URL; Site Grabber requires a verified direct route".into());
    }
    if proxy.is_empty() {
        return Ok(());
    }
    // Match the pinned matcher's precedence: platform values fill empty specific
    // environment settings. Build from this verified snapshot, never a second
    // registry lookup that could silently fail and look like DIRECT.
    if settings.http.is_empty() {
        settings.http = proxy.clone();
    }
    if settings.https.is_empty() {
        settings.https = proxy;
    }
    if settings.no.is_empty() {
        settings.no = windows_proxy_bypass(&bypass);
    }
    Ok(())
}

#[cfg(windows)]
fn windows_auto_direct_result(result: Result<u32, u32>) -> Result<bool, String> {
    use windows_sys::Win32::Networking::WinHttp::{
        ERROR_WINHTTP_AUTODETECTION_FAILED, WINHTTP_ACCESS_TYPE_NAMED_PROXY,
        WINHTTP_ACCESS_TYPE_NO_PROXY,
    };
    match result {
        Ok(WINHTTP_ACCESS_TYPE_NO_PROXY) => Ok(true),
        Ok(WINHTTP_ACCESS_TYPE_NAMED_PROXY) => Ok(false),
        // Microsoft documents this specific outcome as no WPAD configuration
        // found. Apply the existing static/environment fallback; never clear it.
        Err(ERROR_WINHTTP_AUTODETECTION_FAILED) => Ok(true),
        Err(code) => Err(format!(
            "System routing automatic discovery failed (Windows error {code})"
        )),
        Ok(_) => Err("System routing returned an unsupported automatic proxy configuration".into()),
    }
}

#[cfg(windows)]
#[allow(unsafe_code)] // WinHTTP session and proxy strings are owned by the guards below.
fn windows_auto_proxy_direct(url: &url::Url) -> Result<bool, String> {
    use windows_sys::Win32::{
        Foundation::{GetLastError, GlobalFree},
        Networking::WinHttp::{
            WINHTTP_ACCESS_TYPE_NO_PROXY, WINHTTP_AUTO_DETECT_TYPE_DHCP,
            WINHTTP_AUTO_DETECT_TYPE_DNS_A, WINHTTP_AUTOPROXY_AUTO_DETECT,
            WINHTTP_AUTOPROXY_OPTIONS, WINHTTP_PROXY_INFO, WinHttpCloseHandle,
            WinHttpGetProxyForUrl, WinHttpOpen, WinHttpSetTimeouts,
        },
    };
    struct Session(*mut std::ffi::c_void);
    impl Drop for Session {
        fn drop(&mut self) {
            unsafe {
                WinHttpCloseHandle(self.0);
            }
        }
    }
    struct ProxyInfo(WINHTTP_PROXY_INFO);
    impl Drop for ProxyInfo {
        fn drop(&mut self) {
            for pointer in [self.0.lpszProxy, self.0.lpszProxyBypass] {
                if !pointer.is_null() {
                    unsafe {
                        GlobalFree(pointer.cast());
                    }
                }
            }
        }
    }
    let agent: Vec<u16> = "SwoopdlGrabber/0.1".encode_utf16().chain(Some(0)).collect();
    let handle = unsafe {
        WinHttpOpen(
            agent.as_ptr(),
            WINHTTP_ACCESS_TYPE_NO_PROXY,
            std::ptr::null(),
            std::ptr::null(),
            0,
        )
    };
    if handle.is_null() {
        return Err("System routing could not start automatic discovery".into());
    }
    let session = Session(handle);
    if unsafe { WinHttpSetTimeouts(session.0, 5000, 5000, 5000, 5000) } == 0 {
        return Err("System routing could not apply automatic discovery timeouts".into());
    }
    let destination: Vec<u16> = url.as_str().encode_utf16().chain(Some(0)).collect();
    let mut options = WINHTTP_AUTOPROXY_OPTIONS {
        dwFlags: WINHTTP_AUTOPROXY_AUTO_DETECT,
        dwAutoDetectFlags: WINHTTP_AUTO_DETECT_TYPE_DHCP | WINHTTP_AUTO_DETECT_TYPE_DNS_A,
        // Never send the user's integrated Windows credentials to a discovered server.
        fAutoLogonIfChallenged: 0,
        ..Default::default()
    };
    let mut info = ProxyInfo(WINHTTP_PROXY_INFO::default());
    let resolved = unsafe {
        WinHttpGetProxyForUrl(session.0, destination.as_ptr(), &mut options, &mut info.0)
    } != 0;
    let result = if resolved {
        Ok(info.0.dwAccessType)
    } else {
        Err(unsafe { GetLastError() })
    };
    windows_auto_direct_result(result)
}

#[cfg(windows)]
fn windows_proxy_bypass(value: &str) -> String {
    // Keep hyper-util 0.1.21's Windows override semantics, including IPv4 wildcards.
    value
        .split(';')
        .map(|entry| {
            let entry = entry.trim();
            let parts: Vec<_> = entry.split('.').collect();
            if let Some(prefix) = parts.iter().position(|part| *part == "*")
                && (1..=3).contains(&prefix)
                && parts[prefix..].iter().all(|part| *part == "*")
            {
                let mut octets = [0u8; 4];
                let valid = parts[..prefix]
                    .iter()
                    .zip(octets.iter_mut())
                    .all(|(part, octet)| part.parse::<u8>().map(|v| *octet = v).is_ok());
                if valid {
                    return format!(
                        "{}.{}.{}.{}/{}",
                        octets[0],
                        octets[1],
                        octets[2],
                        octets[3],
                        prefix * 8
                    );
                }
            }
            entry.to_owned()
        })
        .collect::<Vec<_>>()
        .join(",")
        .replace("*.", "")
}

/// `builder` with its connections held to `route`: a reqwest client anywhere in SDL goes through
/// this. `Route::Down` refuses to build one. `to_this_computer`: every connection the client
/// makes goes to this computer (a local server, or a proxy on it), which isn't bound. On a
/// system without certificates, its secure connections fail saying so (crate::tls, #264).
pub fn bind_client(
    builder: reqwest::ClientBuilder,
    route: &Route,
    to_this_computer: bool,
) -> Result<reqwest::ClientBuilder, NetError> {
    let builder = crate::tls::configure(builder);
    match route {
        Route::Any => Ok(builder),
        Route::Down { label } => Err(NetError::Down {
            label: label.clone(),
        }),
        Route::Bound(_) if to_this_computer => Ok(builder),
        Route::Bound(b) => {
            #[cfg(windows)]
            {
                Ok(builder.local_address(b.windows_local()))
            }
            #[cfg(any(target_os = "linux", target_os = "android", target_os = "macos"))]
            {
                Ok(builder.interface(&b.name))
            }
            #[cfg(not(any(
                windows,
                target_os = "linux",
                target_os = "android",
                target_os = "macos"
            )))]
            {
                let _ = builder;
                Err(NetError::Down {
                    label: b.label.clone(),
                })
            }
        }
    }
}

/// A client builder for SDL's own requests to `url`'s server (release lists, the update check),
/// with the chosen DNS and adapter as downloads have them, and the system's proxy (reqwest's
/// default). With an adapter chosen, that proxy is looked up now and checked; one on this
/// computer carries the requests unbound.
pub fn client_builder(
    dns: &crate::dns::DnsChoice,
    route: &Route,
    url: &str,
) -> Result<reqwest::ClientBuilder, NetError> {
    let builder = reqwest::Client::builder().dns_resolver(crate::dns::client_resolver(dns, route));
    if route.is_any() {
        return Ok(builder);
    }
    let url = url::Url::parse(url).map_err(|_| NetError::Down {
        label: route.label().unwrap_or_default().to_owned(),
    })?;
    check_url(route, &url)?;
    let (builder, local) = match system_proxy(&url) {
        Some(p) => {
            let host = url::Url::parse(&p)
                .ok()
                .and_then(|u| u.host_str().map(str::to_owned))
                .unwrap_or_default();
            if let Ok(ip) = host.trim_matches(['[', ']']).parse() {
                route.check(ip)?;
            }
            match reqwest::Proxy::all(&p) {
                Ok(proxy) => (builder.no_proxy().proxy(proxy), is_this_computer(&host)),
                Err(_) => (builder.no_proxy(), false),
            }
        }
        None => (builder.no_proxy(), false),
    };
    bind_client(
        builder,
        route,
        local || is_this_computer(url.host_str().unwrap_or("")),
    )
}

/// A TCP connection to `addr` through `route` (FTP, DNS, the address race).
pub async fn tcp_connect(
    route: &Route,
    addr: SocketAddr,
) -> std::io::Result<tokio::net::TcpStream> {
    let refused = |e: NetError| std::io::Error::new(std::io::ErrorKind::NetworkUnreachable, e);
    route.check(addr.ip()).map_err(refused)?;
    let b = match route {
        Route::Bound(b) if !b.exempt(addr.ip()) => b,
        _ => return tokio::net::TcpStream::connect(addr).await,
    };
    let socket = socket2::Socket::new(
        socket2::Domain::for_address(addr),
        socket2::Type::STREAM,
        Some(socket2::Protocol::TCP),
    )?;
    b.pin(&socket, addr)?;
    socket.set_nonblocking(true)?;
    let socket = tokio::net::TcpSocket::from_std_stream(socket.into());
    socket.connect(addr).await
}

/// [`tcp_connect`] for code without a runtime (a reachability check), within `timeout`.
pub fn tcp_connect_blocking(
    route: &Route,
    addr: SocketAddr,
    timeout: Duration,
) -> std::io::Result<std::net::TcpStream> {
    let refused = |e: NetError| std::io::Error::new(std::io::ErrorKind::NetworkUnreachable, e);
    route.check(addr.ip()).map_err(refused)?;
    let socket = socket2::Socket::new(
        socket2::Domain::for_address(addr),
        socket2::Type::STREAM,
        Some(socket2::Protocol::TCP),
    )?;
    if let Route::Bound(b) = route
        && !b.exempt(addr.ip())
    {
        b.pin(&socket, addr)?;
    }
    socket.connect_timeout(&addr.into(), timeout)?;
    Ok(socket.into())
}

/// An adapter of this computer, as [`adapters`] lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Adapter {
    /// Windows: the name shown in Network Connections ("Ethernet", "ProtonVPN"); Linux and
    /// macOS: the interface's name ("wg0", "utun4").
    pub name: String,
    pub index: u32,
    /// Its addresses that can reach other computers (no link-local 169.254.x.x or fe80::).
    pub v4: Vec<Ipv4Addr>,
    pub v6: Vec<Ipv6Addr>,
    /// Link-local addresses (only a computer on the same cable answers them).
    pub local_only: Vec<IpAddr>,
}

/// The computer's network adapters (this computer's own loopback left out), in the system's
/// order. Adapters without any address are listed too: a VPN that is off.
pub fn adapters() -> Vec<Adapter> {
    use network_interface::{Addr, NetworkInterface, NetworkInterfaceConfig};
    let mut out: Vec<Adapter> = Vec::new();
    for nic in NetworkInterface::show().unwrap_or_default() {
        if nic.internal {
            continue;
        }
        // Linux lists an interface once per address: gather them.
        let at = match out.iter().position(|a| a.name == nic.name) {
            Some(i) => i,
            None => {
                out.push(Adapter {
                    name: nic.name.clone(),
                    index: nic.index,
                    v4: Vec::new(),
                    v6: Vec::new(),
                    local_only: Vec::new(),
                });
                out.len() - 1
            }
        };
        let a = &mut out[at];
        for addr in &nic.addr {
            match addr {
                Addr::V4(v) if v.ip.is_loopback() || v.ip.is_unspecified() => {}
                Addr::V4(v) if v.ip.is_link_local() => a.local_only.push(IpAddr::V4(v.ip)),
                Addr::V4(v) => a.v4.push(v.ip),
                Addr::V6(v) if v.ip.is_loopback() || v.ip.is_unspecified() => {}
                Addr::V6(v) if v.ip.is_unicast_link_local() => a.local_only.push(IpAddr::V6(v.ip)),
                Addr::V6(v) => a.v6.push(v.ip),
            }
        }
    }
    out
}

/// The route for the adapter chosen as `choice`: "" is any; else an adapter's name (any case),
/// or one of its addresses (that address is bound, even a link-local one). A choice that isn't
/// there, or has no address, is `Down`: it never becomes `Any`.
pub fn route_for(choice: &str, adapters: &[Adapter]) -> Route {
    let choice = choice.trim();
    if choice.is_empty() {
        return Route::Any;
    }
    let down = || Route::Down {
        label: choice.to_owned(),
    };
    if let Ok(ip) = choice.parse::<IpAddr>() {
        let Some(a) = adapters.iter().find(|a| {
            a.v4.iter().any(|v| IpAddr::V4(*v) == ip)
                || a.v6.iter().any(|v| IpAddr::V6(*v) == ip)
                || a.local_only.contains(&ip)
        }) else {
            return down();
        };
        let (v4, v6) = match ip {
            IpAddr::V4(v) => (Some(v), None),
            IpAddr::V6(v) => (None, Some(v)),
        };
        return Route::Bound(Arc::new(Binding {
            label: format!("{} ({ip})", a.name),
            name: a.name.clone(),
            index: a.index,
            v4,
            v6,
            loopback_too: false,
        }));
    }
    let Some(a) = adapters
        .iter()
        .find(|a| a.name.eq_ignore_ascii_case(choice))
    else {
        return down();
    };
    if a.v4.is_empty() && a.v6.is_empty() {
        return Route::Down {
            label: a.name.clone(),
        };
    }
    Route::Bound(Arc::new(Binding {
        label: a.name.clone(),
        name: a.name.clone(),
        index: a.index,
        v4: a.v4.first().copied(),
        // Prefer a global address to a unique-local one (fd00::/8, used inside VPNs too).
        v6: a
            .v6
            .iter()
            .find(|v| !v.is_unique_local())
            .or(a.v6.first())
            .copied(),
        loopback_too: false,
    }))
}

/// Whether a host (from a URL or a proxy address) is this computer.
pub fn is_this_computer(host: &str) -> bool {
    let host = host.trim_matches(['[', ']']);
    host.eq_ignore_ascii_case("localhost")
        || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

/// The system's answer to "which of my addresses would a connection to `dst` leave from",
/// installed by the app (Windows: GetBestRoute2, in its win32.rs).
static SOURCE: OnceLock<fn(IpAddr) -> Option<IpAddr>> = OnceLock::new();

/// Install the system's source-address lookup (see [`source_address`]).
pub fn set_source_lookup(f: fn(IpAddr) -> Option<IpAddr>) {
    let _ = SOURCE.set(f);
}

/// The address a connection to `dst` would leave from (PAC's `myIpAddress`). Nothing is sent, and
/// on Windows no socket is opened: a UDP socket there is a listening socket to the firewall, which
/// would ask the user about SDL.
pub fn source_address(dst: IpAddr) -> Option<IpAddr> {
    if let Some(f) = SOURCE.get() {
        return f(dst);
    }
    // Linux and macOS: a UDP socket "connected" to `dst` (no packet goes out; their firewalls
    // don't ask about it) tells the system's choice.
    #[cfg(not(windows))]
    {
        let unspecified: SocketAddr = match dst {
            IpAddr::V4(_) => (Ipv4Addr::UNSPECIFIED, 0).into(),
            IpAddr::V6(_) => (Ipv6Addr::UNSPECIFIED, 0).into(),
        };
        if let Ok(ip) = std::net::UdpSocket::bind(unspecified)
            .and_then(|s| s.connect((dst, 9)).map(|_| s))
            .and_then(|s| s.local_addr())
            .map(|a| a.ip())
        {
            return Some(ip);
        }
    }
    // Windows without the lookup installed (the engine alone): the first adapter's address.
    adapters().into_iter().find_map(|a| match dst {
        IpAddr::V4(_) => a.v4.first().copied().map(IpAddr::V4),
        IpAddr::V6(_) => a.v6.first().copied().map(IpAddr::V6),
    })
}

/// How long the race and DNS connections may take to connect.
pub(crate) const CONNECT: Duration = Duration::from_secs(10);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crawl_system_snapshot_checks_scheme_and_per_host_bypass() {
        let settings = CrawlProxySettings {
            http: "http://user:secret@proxy.test:8080".into(),
            no: "direct.test,192.168.0.0/16".into(),
            ..Default::default()
        };
        for (url, direct) in [
            ("http://direct.test/a", true),
            ("http://sub.direct.test/a", true),
            ("http://notdirect.test/a", false),
            ("http://192.168.2.3/a", true),
            ("https://proxied.test/a", true),
            ("http://proxied.test/a", false),
        ] {
            assert_eq!(
                settings.direct(&url.parse().unwrap()).unwrap(),
                direct,
                "{url}"
            );
        }
        let invalid = CrawlProxySettings {
            all: "http://user:secret@[".into(),
            ..Default::default()
        };
        let reason = invalid
            .direct(&"http://direct.test/".parse().unwrap())
            .unwrap_err();
        assert!(reason.contains("routing"));
        assert!(!reason.contains("secret") && !reason.contains("user"));
        assert!(
            CrawlProxySettings::default()
                .direct(&"http://direct.test/".parse().unwrap())
                .unwrap()
        );
    }

    #[cfg(windows)]
    #[test]
    fn crawl_windows_auto_discovery_distinguishes_no_wpad_from_failure_and_proxy() {
        use windows_sys::Win32::Networking::WinHttp::{
            ERROR_WINHTTP_AUTODETECTION_FAILED, ERROR_WINHTTP_TIMEOUT,
            ERROR_WINHTTP_UNABLE_TO_DOWNLOAD_SCRIPT, WINHTTP_ACCESS_TYPE_NAMED_PROXY,
            WINHTTP_ACCESS_TYPE_NO_PROXY,
        };
        assert!(windows_auto_direct_result(Ok(WINHTTP_ACCESS_TYPE_NO_PROXY)).unwrap());
        assert!(!windows_auto_direct_result(Ok(WINHTTP_ACCESS_TYPE_NAMED_PROXY)).unwrap());
        assert!(windows_auto_direct_result(Err(ERROR_WINHTTP_AUTODETECTION_FAILED)).unwrap());
        assert!(windows_auto_direct_result(Err(ERROR_WINHTTP_TIMEOUT)).is_err());
        assert!(windows_auto_direct_result(Err(ERROR_WINHTTP_UNABLE_TO_DOWNLOAD_SCRIPT)).is_err());
        assert!(windows_auto_direct_result(Ok(0)).is_err());
        let fallback = CrawlProxySettings {
            http: "http://user:secret@proxy.test:8080".into(),
            ..Default::default()
        };
        // No WPAD does not authorize bypassing a separately configured fallback proxy.
        let direct = windows_auto_direct_result(Err(ERROR_WINHTTP_AUTODETECTION_FAILED)).unwrap()
            && fallback
                .direct(&"http://public.test/".parse().unwrap())
                .unwrap();
        assert!(!direct);
    }

    #[cfg(windows)]
    #[test]
    fn crawl_windows_bypass_keeps_pinned_matcher_semantics() {
        let settings = CrawlProxySettings {
            all: "http://proxy.test:8080".into(),
            no: windows_proxy_bypass(" *.example.test ;10.*;192.168.*.*"),
            ..Default::default()
        };
        for (url, direct) in [
            ("http://example.test/", true),
            ("http://10.3.4.5/", true),
            ("http://192.168.2.3/", true),
            ("http://192.169.2.3/", false),
        ] {
            assert_eq!(settings.direct(&url.parse().unwrap()).unwrap(), direct);
        }
    }

    fn nic(name: &str, v4: &[&str], v6: &[&str], local: &[&str]) -> Adapter {
        Adapter {
            name: name.into(),
            index: 7,
            v4: v4.iter().map(|s| s.parse().unwrap()).collect(),
            v6: v6.iter().map(|s| s.parse().unwrap()).collect(),
            local_only: local.iter().map(|s| s.parse().unwrap()).collect(),
        }
    }

    #[test]
    fn a_missing_or_addressless_adapter_is_down_never_any() {
        let list = [
            nic("Ethernet", &["192.168.1.11"], &[], &["fe80::1"]),
            nic("ProtonVPN", &[], &[], &[]),
        ];
        assert_eq!(route_for("", &list), Route::Any);
        assert!(matches!(route_for("WireGuard", &list), Route::Down { .. }));
        assert!(matches!(route_for("ProtonVPN", &list), Route::Down { .. }));
        assert!(matches!(route_for("10.9.9.9", &list), Route::Down { .. }));
        let Route::Bound(b) = route_for("ethernet", &list) else {
            panic!("Ethernet is up");
        };
        assert_eq!(b.v4, Some("192.168.1.11".parse().unwrap()));
        assert_eq!(b.v6, None);
        // An address picks its adapter, even a link-local one.
        let Route::Bound(b) = route_for("fe80::1", &list) else {
            panic!("fe80::1 is Ethernet's");
        };
        assert_eq!((b.v4, b.v6), (None, Some("fe80::1".parse().unwrap())));
    }

    #[test]
    fn only_the_bound_family_is_reached_and_this_computer_always() {
        let b = Binding {
            label: "VPN".into(),
            v4: Some("10.8.0.2".parse().unwrap()),
            ..Default::default()
        };
        let route = Route::Bound(Arc::new(b.clone()));
        assert!(b.allows("93.184.216.34".parse().unwrap()));
        assert!(!b.allows("2606:2800::1".parse().unwrap()));
        assert!(b.allows("::1".parse().unwrap()));
        assert!(b.allows("127.0.0.1".parse().unwrap()));
        let kept = route.keep(vec![
            "2606:2800::1".parse().unwrap(),
            "1.2.3.4".parse().unwrap(),
        ]);
        assert_eq!(kept, vec!["1.2.3.4".parse::<IpAddr>().unwrap()]);
        assert!(matches!(
            route.check("2606:2800::1".parse().unwrap()),
            Err(NetError::Family { .. })
        ));
        let down = Route::Down {
            label: "VPN".into(),
        };
        assert!(matches!(
            down.check("127.0.0.1".parse().unwrap()),
            Err(NetError::Down { .. })
        ));
        assert!(is_this_computer("[::1]") && is_this_computer("LOCALHOST"));
        assert!(!is_this_computer("192.168.1.1"));
    }

    #[tokio::test]
    async fn a_bound_connection_leaves_from_the_adapter() {
        // 127.0.0.2 answers on Windows and Linux; macOS only has 127.0.0.1 unless aliased.
        if cfg!(target_os = "macos") {
            return;
        }
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let route = Route::Bound(Arc::new(Binding {
            label: "test".into(),
            name: "lo".into(),
            index: 1,
            v4: Some(Ipv4Addr::new(127, 0, 0, 2)),
            v6: None,
            loopback_too: true,
        }));
        let (conn, accepted) = tokio::join!(tcp_connect(&route, addr), listener.accept());
        let conn = conn.unwrap();
        let (_, peer) = accepted.unwrap();
        if cfg!(windows) {
            assert_eq!(peer.ip(), IpAddr::from([127, 0, 0, 2]));
        }
        assert_eq!(conn.peer_addr().unwrap(), addr);
        // The adapter is gone: nothing connects.
        let down = Route::Down {
            label: "test".into(),
        };
        let err = tcp_connect(&down, addr).await.unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::NetworkUnreachable);
    }
}
