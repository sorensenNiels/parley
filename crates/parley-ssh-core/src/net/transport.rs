//! How the first hop is reached (R-SSH-26, R-SSH-29, D51): TCP with the IP version asked for, TCP keepalives and
//! Nagle off, either straight, through a SOCKS 5 proxy (RFC 1928, user name and password RFC 1929, the host name
//! resolved by the proxy), an HTTP proxy's CONNECT (RFC 9110 §9.3.6, Basic authentication, RFC 7617), or a
//! local proxy command whose standard input and output carry the connection (OpenSSH's `ProxyCommand`). A proxy
//! password is asked when the proxy wants one and never kept.

use std::fmt::Write as _;
use std::net::SocketAddr;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;

use super::sign_on::ask_or_skip;
use super::{Commands, Link, Prompt, PromptField, Target};
use crate::{Error, Secret};

/// Which IP version to connect with (R-SSH-26).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AddressFamily {
    #[default]
    Any,
    V4,
    V6,
}

/// A proxy before the first hop (R-SSH-29).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Proxy {
    #[default]
    None,
    /// SOCKS 5 at `host:port`; `user` empty for none.
    Socks5 {
        host: String,
        port: u16,
        user: String,
    },
    /// An HTTP proxy's CONNECT at `host:port`; `user` empty for none.
    Http {
        host: String,
        port: u16,
        user: String,
    },
    /// A local command, with `%h`, `%p`, `%r` and `%%` replaced.
    Command(String),
}

/// A stream the SSH connection runs over.
pub trait Io: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Io for T {}

/// The longest reply header a proxy may send.
const MAX_HEADER: usize = 8 * 1024;
/// How often the operating system checks an idle TCP connection.
const TCP_KEEPALIVE: Duration = Duration::from_secs(60);

/// Opens the stream to `target` as `proxy` says.
pub(super) async fn open(
    target: &Target,
    family: AddressFamily,
    proxy: &Proxy,
    commands: &Commands,
    link: &Link,
) -> Result<Box<dyn Io>, Error> {
    match proxy {
        Proxy::None => Ok(Box::new(tcp(&target.host, target.port, family).await?)),
        Proxy::Socks5 { host, port, user } => {
            link.log(format!("Through the SOCKS 5 proxy {host} port {port}"));
            let mut stream = tcp(host, *port, family).await?;
            let proxy = ProxyAsk {
                host,
                port: *port,
                user,
                commands,
                link,
            };
            socks5(&mut stream, target, &proxy).await?;
            Ok(Box::new(stream))
        }
        Proxy::Http { host, port, user } => {
            link.log(format!("Through the HTTP proxy {host} port {port}"));
            let proxy = ProxyAsk {
                host,
                port: *port,
                user,
                commands,
                link,
            };
            Ok(Box::new(http(target, family, &proxy).await?))
        }
        Proxy::Command(command) => {
            let run = proxy_command_line(command, target);
            link.log(format!("Through the proxy command: {run}"));
            Ok(Box::new(spawn(&run)?))
        }
    }
}

/// A proxy that may ask for its password.
struct ProxyAsk<'a> {
    host: &'a str,
    port: u16,
    user: &'a str,
    commands: &'a Commands,
    link: &'a Link,
}

impl ProxyAsk<'_> {
    async fn password(&self) -> Result<Secret, Error> {
        let prompt = Prompt {
            host: format!("{}@{}:{}", self.user, self.host, self.port),
            name: "Proxy password".to_owned(),
            instructions: format!(
                "The proxy {} asks for the password of {}.",
                self.host, self.user
            ),
            fields: vec![PromptField {
                label: "Password".to_owned(),
                echo: false,
            }],
        };
        let answers = ask_or_skip(prompt, self.commands, self.link)
            .await?
            .ok_or(Error::Cancelled)?;
        Ok(answers.into_iter().next().unwrap_or_default())
    }
}

/// TCP to `host:port` in the family asked for, each address in turn; Nagle off and TCP keepalives on.
async fn tcp(host: &str, port: u16, family: AddressFamily) -> Result<TcpStream, Error> {
    let unreachable = |source| Error::Connect {
        host: host.to_owned(),
        source,
    };
    let addresses: Vec<SocketAddr> = tokio::net::lookup_host((host, port))
        .await
        .map_err(unreachable)?
        .filter(|a| match family {
            AddressFamily::Any => true,
            AddressFamily::V4 => a.is_ipv4(),
            AddressFamily::V6 => a.is_ipv6(),
        })
        .collect();
    let mut last = std::io::Error::new(
        std::io::ErrorKind::NotFound,
        match family {
            AddressFamily::Any => "the name has no address",
            AddressFamily::V4 => "the name has no IPv4 address",
            AddressFamily::V6 => "the name has no IPv6 address",
        },
    );
    for address in addresses {
        match TcpStream::connect(address).await {
            Ok(stream) => {
                // Both only make the connection better; without them it still works.
                let _ = stream.set_nodelay(true);
                let keepalive = socket2::TcpKeepalive::new().with_time(TCP_KEEPALIVE);
                let _ = socket2::SockRef::from(&stream).set_tcp_keepalive(&keepalive);
                return Ok(stream);
            }
            Err(e) => last = e,
        }
    }
    Err(unreachable(last))
}

fn proxy_error(text: impl Into<String>) -> Error {
    Error::Proxy(text.into())
}

/// RFC 1928 §6's replies.
fn socks5_reply(code: u8) -> &'static str {
    match code {
        1 => "a general failure",
        2 => "the proxy's rules do not allow it",
        3 => "the network cannot be reached",
        4 => "the host cannot be reached",
        5 => "the connection was refused",
        6 => "the time ran out",
        7 => "the command is not supported",
        8 => "the address type is not supported",
        _ => "an unknown reply",
    }
}

async fn socks5<S: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut S,
    target: &Target,
    proxy: &ProxyAsk<'_>,
) -> Result<(), Error> {
    let io = |e: std::io::Error| proxy_error(format!("the SOCKS 5 proxy stopped answering: {e}"));
    // RFC 1928 §3: no authentication, and user name/password (RFC 1929) when there is a user.
    let greeting: &[u8] = if proxy.user.is_empty() {
        &[5, 1, 0]
    } else {
        &[5, 2, 0, 2]
    };
    stream.write_all(greeting).await.map_err(io)?;
    let mut choice = [0; 2];
    stream.read_exact(&mut choice).await.map_err(io)?;
    match choice {
        [5, 0] => {}
        [5, 2] if !proxy.user.is_empty() => {
            let password = proxy.password().await?;
            let user = proxy.user.as_bytes();
            let secret = password.expose().as_bytes();
            let (Ok(user_len), Ok(secret_len)) =
                (u8::try_from(user.len()), u8::try_from(secret.len()))
            else {
                return Err(proxy_error(
                    "the user name or password is longer than SOCKS 5 allows (255 bytes)",
                ));
            };
            let mut request = vec![1, user_len];
            request.extend_from_slice(user);
            request.push(secret_len);
            request.extend_from_slice(secret);
            stream.write_all(&request).await.map_err(io)?;
            zeroize::Zeroize::zeroize(&mut request);
            let mut status = [0; 2];
            stream.read_exact(&mut status).await.map_err(io)?;
            if status[1] != 0 {
                return Err(proxy_error(
                    "the SOCKS 5 proxy refused the user name or password",
                ));
            }
        }
        [5, 2] => {
            return Err(proxy_error(
                "the SOCKS 5 proxy wants a user name: set one in the session",
            ));
        }
        [5, _] => {
            return Err(proxy_error(
                "the SOCKS 5 proxy accepts none of the ways Parley SSH signs on",
            ));
        }
        _ => return Err(proxy_error("that is not a SOCKS 5 proxy")),
    }
    // RFC 1928 §4: CONNECT to a domain name, which the proxy resolves.
    let host = target.host.as_bytes();
    let host_len = u8::try_from(host.len())
        .map_err(|_| proxy_error("the host name is longer than 255 bytes"))?;
    let mut request = vec![5, 1, 0, 3, host_len];
    request.extend_from_slice(host);
    request.extend_from_slice(&target.port.to_be_bytes());
    stream.write_all(&request).await.map_err(io)?;
    let mut reply = [0; 4];
    stream.read_exact(&mut reply).await.map_err(io)?;
    if reply[0] != 5 {
        return Err(proxy_error("that is not a SOCKS 5 proxy"));
    }
    if reply[1] != 0 {
        return Err(proxy_error(format!(
            "the SOCKS 5 proxy could not reach {} port {}: {}",
            target.host,
            target.port,
            socks5_reply(reply[1])
        )));
    }
    // The address the proxy bound, then its port: read and left.
    let address = match reply[3] {
        1 => 4,
        4 => 16,
        3 => {
            let mut len = [0; 1];
            stream.read_exact(&mut len).await.map_err(io)?;
            usize::from(len[0])
        }
        _ => {
            return Err(proxy_error(
                "the SOCKS 5 proxy answered with an unknown address type",
            ));
        }
    };
    let mut rest = vec![0; address + 2];
    stream.read_exact(&mut rest).await.map_err(io)?;
    Ok(())
}

/// `host:port` as a CONNECT request names it, with an IPv6 address in brackets.
fn authority(target: &Target) -> String {
    if target.host.contains(':') {
        format!("[{}]:{}", target.host, target.port)
    } else {
        format!("{}:{}", target.host, target.port)
    }
}

async fn http(
    target: &Target,
    family: AddressFamily,
    proxy: &ProxyAsk<'_>,
) -> Result<TcpStream, Error> {
    let mut stream = tcp(proxy.host, proxy.port, family).await?;
    let status = connect_request(&mut stream, target, None).await?;
    if status == 407 && !proxy.user.is_empty() {
        // The proxy wants a password: asked, then a new connection with it (the proxy may have closed this one).
        let password = proxy.password().await?;
        let mut credentials = format!("{}:{}", proxy.user, password.expose());
        let basic = data_encoding::BASE64.encode(credentials.as_bytes());
        zeroize::Zeroize::zeroize(&mut credentials);
        stream = tcp(proxy.host, proxy.port, family).await?;
        let status = connect_request(&mut stream, target, Some(&basic)).await?;
        return http_status(status, target).map(|()| stream);
    }
    if status == 407 {
        return Err(proxy_error(
            "the HTTP proxy wants a user name and password: set the user name in the session",
        ));
    }
    http_status(status, target).map(|()| stream)
}

fn http_status(status: u16, target: &Target) -> Result<(), Error> {
    match status {
        200..=299 => Ok(()),
        407 => Err(proxy_error(
            "the HTTP proxy refused the user name or password",
        )),
        403 => Err(proxy_error(format!(
            "the HTTP proxy does not allow {}",
            authority(target)
        ))),
        _ => Err(proxy_error(format!(
            "the HTTP proxy could not reach {} (status {status})",
            authority(target)
        ))),
    }
}

/// Sends CONNECT and reads the reply's header; returns its status.
async fn connect_request<S: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut S,
    target: &Target,
    basic: Option<&str>,
) -> Result<u16, Error> {
    let io = |e: std::io::Error| proxy_error(format!("the HTTP proxy stopped answering: {e}"));
    let to = authority(target);
    let mut request = format!("CONNECT {to} HTTP/1.1\r\nHost: {to}\r\n");
    if let Some(basic) = basic {
        // Writing to a String cannot fail.
        let _ = write!(request, "Proxy-Authorization: Basic {basic}\r\n");
    }
    request.push_str("\r\n");
    stream.write_all(request.as_bytes()).await.map_err(io)?;
    zeroize::Zeroize::zeroize(&mut request);
    // Byte by byte, so nothing after the header (the server's SSH greeting) is read here.
    let mut header = Vec::new();
    while !header.ends_with(b"\r\n\r\n") {
        if header.len() >= MAX_HEADER {
            return Err(proxy_error("the HTTP proxy's answer is too long"));
        }
        let mut byte = [0; 1];
        if stream.read(&mut byte).await.map_err(io)? == 0 {
            return Err(proxy_error("the HTTP proxy closed the connection"));
        }
        header.push(byte[0]);
    }
    parse_status(&header).ok_or_else(|| proxy_error("that is not an HTTP proxy"))
}

/// The status of `HTTP/1.x NNN …`.
fn parse_status(header: &[u8]) -> Option<u16> {
    let line = std::str::from_utf8(header).ok()?.lines().next()?;
    let mut words = line.split_whitespace();
    words.next().filter(|v| v.starts_with("HTTP/1."))?;
    words.next()?.parse().ok()
}

/// The proxy command with `%h`, `%p`, `%r` and `%%` replaced (`ssh_config(5)`, TOKENS).
fn proxy_command_line(command: &str, target: &Target) -> String {
    let mut out = String::new();
    let mut chars = command.chars();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('h') => out.push_str(&target.host),
            Some('p') => out.push_str(&target.port.to_string()),
            Some('r') => out.push_str(&target.user),
            Some('%') | None => out.push('%'),
            Some(other) => {
                out.push('%');
                out.push(other);
            }
        }
    }
    out
}

/// Runs `line` in the system's shell; its standard output and input are the connection.
fn spawn(line: &str) -> Result<impl Io + use<>, Error> {
    let mut command = if cfg!(windows) {
        let mut c = tokio::process::Command::new("cmd");
        c.arg("/C").arg(line);
        // Win32 process creation flags: `CREATE_NO_WINDOW`, or `cmd` opens a console window
        // beside Parley SSH's for as long as the connection lasts (R-SSH-60).
        #[cfg(windows)]
        c.creation_flags(0x0800_0000);
        c
    } else {
        let mut c = tokio::process::Command::new("sh");
        c.arg("-c").arg(line);
        c
    };
    let mut child = command
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| proxy_error(format!("the proxy command could not start: {e}")))?;
    let (Some(stdout), Some(stdin)) = (child.stdout.take(), child.stdin.take()) else {
        return Err(proxy_error("the proxy command has no input or output"));
    };
    // The command ends when its input closes with the connection; its exit status is of no use here.
    tokio::spawn(async move {
        let _ = child.wait().await;
    });
    Ok(tokio::io::join(stdout, stdin))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(host: &str) -> Target {
        Target {
            host: host.into(),
            port: 2222,
            user: "æøå".into(),
        }
    }

    #[test]
    fn a_proxy_command_has_its_tokens_replaced_r_ssh_29() {
        assert_eq!(
            proxy_command_line("nc -X 5 %h %p # %r %% %x %", &target("box")),
            "nc -X 5 box 2222 # æøå % %x %"
        );
    }

    #[test]
    fn an_http_reply_gives_its_status_and_ipv6_is_bracketed_r_ssh_29() {
        assert_eq!(
            parse_status(b"HTTP/1.1 200 Connection established\r\n\r\n"),
            Some(200)
        );
        assert_eq!(
            parse_status(b"HTTP/1.0 407 Proxy Authentication Required\r\n\r\n"),
            Some(407)
        );
        assert_eq!(parse_status(b"SSH-2.0-x\r\n\r\n"), None);
        assert_eq!(parse_status(b""), None);
        assert_eq!(authority(&target("::1")), "[::1]:2222");
        assert_eq!(authority(&target("box")), "box:2222");
    }
}
