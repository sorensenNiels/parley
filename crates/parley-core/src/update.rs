//! Whether a newer version of Parley has been released (R-UI-11, D13): asks GitHub for the
//! latest published release and compares its version with this one. As in Tortuga.
//!
//! [`found`] reads GitHub's answer without I/O, so it is tested with fixtures; [`check`]
//! asks over HTTPS with the operating system's trust store.

use std::time::Duration;

use serde::Deserialize;

use crate::{Error, Result};

/// Where GitHub lists the latest published release. Drafts and pre-releases are not
/// returned, so a release is only seen once it is published.
pub const LATEST_RELEASE_URL: &str =
    "https://api.github.com/repos/sorensenNiels/parley/releases/latest";

/// How long a check may take; it runs in the background, but should not hang around.
const TIMEOUT: Duration = Duration::from_secs(10);
/// GitHub's answer is a few kilobytes; more is not a release.
const MAX_ANSWER: u64 = 2 * 1024 * 1024;

/// A published release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    /// Its version, without the tag's `v` ("0.3.0").
    pub version: String,
    /// Its page on GitHub, with the release notes and installers.
    pub page: String,
    /// Its files (installers) that can be downloaded over `https`.
    pub downloads: Vec<Download>,
}

/// A file of a release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Download {
    /// Its file name, such as `parley_0.3.0_amd64.deb`.
    pub name: String,
    pub url: String,
}

impl Download {
    /// The `curl` command that downloads it into the current folder, following GitHub's
    /// redirect and failing on an error page (R-UI-12).
    pub fn curl_command(&self) -> String {
        format!("curl -fLO '{}'", self.url)
    }
}

impl Release {
    /// Its Linux packages, `.deb` and `.AppImage`, in that order (R-UI-12).
    pub fn linux_packages(&self) -> Vec<&Download> {
        let ending = |suffix: &'static str| {
            self.downloads
                .iter()
                .filter(move |d| d.name.to_ascii_lowercase().ends_with(suffix))
        };
        ending(".deb").chain(ending(".appimage")).collect()
    }
}

/// What a check found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Found {
    /// No release is newer than this version (or none is published yet).
    UpToDate,
    Newer(Release),
}

/// What GitHub says about a release; only what the check needs.
#[derive(Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    assets: Vec<GitHubAsset>,
}

#[derive(Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

impl GitHubAsset {
    /// As a download, when its URL is `https` and safe to put in single quotes in a shell
    /// command: the answer comes from the network.
    fn download(self) -> Option<Download> {
        let safe = self.browser_download_url.starts_with("https://")
            && !self
                .browser_download_url
                .contains(|c: char| c == '\'' || c.is_whitespace() || c.is_control());
        safe.then_some(Download {
            name: self.name,
            url: self.browser_download_url,
        })
    }
}

/// Asks `url` (normally [`LATEST_RELEASE_URL`]) whether a release newer than `current`
/// exists. Sends nothing about the user: GitHub requires a `User-Agent`, which names the
/// app and its version. Blocks for up to ten seconds: run it off the UI thread.
pub fn check(url: &str, current: &str) -> Result<Found> {
    let tls = ureq::tls::TlsConfig::builder()
        .root_certs(ureq::tls::RootCerts::PlatformVerifier)
        .build();
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .http_status_as_error(false)
        .tls_config(tls)
        .build()
        .into();
    let failed = |e: ureq::Error| Error::UpdateCheck(e.to_string());
    let mut response = agent
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", user_agent())
        .call()
        .map_err(failed)?;
    let status = response.status().as_u16();
    let body = response
        .body_mut()
        .with_config()
        .limit(MAX_ANSWER)
        .read_to_vec()
        .map_err(failed)?;
    found(status, &body, current)
}

/// The `User-Agent` of a check: the app and its version, nothing more.
fn user_agent() -> String {
    format!("Parley/{}", env!("CARGO_PKG_VERSION"))
}

/// What GitHub's answer (`status`, `body`) says about a release newer than `current`.
pub fn found(status: u16, body: &[u8], current: &str) -> Result<Found> {
    match status {
        // GitHub answers 404 while no release is published.
        404 => Ok(Found::UpToDate),
        200..=299 => {
            let release: GitHubRelease = serde_json::from_slice(body)
                .map_err(|e| Error::UpdateCheck(format!("unexpected answer from GitHub: {e}")))?;
            let version = release
                .tag_name
                .strip_prefix('v')
                .unwrap_or(&release.tag_name)
                .to_owned();
            Ok(if is_newer(&version, current) {
                Found::Newer(Release {
                    version,
                    page: release.html_url,
                    downloads: release
                        .assets
                        .into_iter()
                        .filter_map(GitHubAsset::download)
                        .collect(),
                })
            } else {
                Found::UpToDate
            })
        }
        status => Err(Error::UpdateCheck(format!("GitHub answered {status}"))),
    }
}

/// Whether version `candidate` comes after `current` ("1.2.3", optionally with a
/// pre-release part as in "1.2.3-beta.1", which comes before "1.2.3"). A version that
/// can't be read is never newer.
pub fn is_newer(candidate: &str, current: &str) -> bool {
    match (parse(candidate), parse(current)) {
        (Some(a), Some(b)) => a > b,
        _ => false,
    }
}

/// `major.minor.patch`, and whether it is a final release (sorts after its pre-releases).
fn parse(version: &str) -> Option<([u64; 3], bool)> {
    let (numbers, pre) = match version.trim().split_once('-') {
        Some((numbers, pre)) => (numbers, Some(pre)),
        None => (version.trim(), None),
    };
    // Build metadata ("+abc") doesn't change the order.
    let numbers = numbers.split('+').next().unwrap_or_default();
    let mut parts = numbers.split('.').map(|p| p.parse::<u64>().ok());
    let triple = [parts.next()??, parts.next()??, parts.next()??];
    parts.next().is_none().then_some((triple, pre.is_none()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// R-UI-11: versions compare by number, not as text, and a pre-release comes before
    /// its release.
    #[test]
    fn newer_versions_compare_by_number() {
        assert!(is_newer("0.10.0", "0.9.9"));
        assert!(is_newer("1.0.0", "0.99.99"));
        assert!(is_newer("0.6.1", "0.6.0"));
        assert!(!is_newer("0.6.0", "0.6.0"));
        assert!(!is_newer("0.5.9", "0.6.0"));
        assert!(is_newer("1.0.0", "1.0.0-beta.1"));
        assert!(!is_newer("1.0.0-beta.1", "1.0.0"));
        assert!(is_newer("1.0.0-beta.1", "0.9.0"));
        assert!(!is_newer("1.0.0+build.5", "1.0.0"));
    }

    /// R-UI-11: a tag that isn't a version is never offered as an update.
    #[test]
    fn unreadable_versions_are_never_newer() {
        for odd in ["", "latest", "1.0", "1.0.0.0", "æøå😀", "1.x.0", "-1.0.0"] {
            assert!(!is_newer(odd, "0.6.0"), "{odd:?}");
        }
        assert!(!is_newer("9.9.9", "not a version"));
    }

    const ANSWER: &str = r#"{
        "tag_name": "v0.3.0",
        "html_url": "https://github.com/sorensenNiels/parley/releases/tag/v0.3.0",
        "draft": false,
        "assets": [
            {"name": "parley_0.3.0_amd64.deb",
             "browser_download_url": "https://github.com/sorensenNiels/parley/releases/download/v0.3.0/parley_0.3.0_amd64.deb"},
            {"name": "parley_0.3.0_x64_en-US.msi",
             "browser_download_url": "https://github.com/sorensenNiels/parley/releases/download/v0.3.0/parley_0.3.0_x64_en-US.msi"},
            {"name": "evil.deb", "browser_download_url": "https://x.test/'; rm -rf ~; '"}
        ]
    }"#;

    /// R-UI-11: GitHub's answer for a newer release gives its version, page and safe files.
    #[test]
    fn a_newer_release_is_read_from_githubs_answer() {
        let Ok(Found::Newer(release)) = found(200, ANSWER.as_bytes(), "0.2.0") else {
            panic!("a newer release");
        };
        assert_eq!(release.version, "0.3.0");
        assert!(release.page.ends_with("/releases/tag/v0.3.0"));
        let names: Vec<&str> = release.downloads.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(
            names,
            ["parley_0.3.0_amd64.deb", "parley_0.3.0_x64_en-US.msi"]
        );
    }

    /// R-UI-11: the same version, no release yet (404), an error and a garbled answer.
    #[test]
    fn up_to_date_missing_and_broken_answers() {
        assert_eq!(
            found(200, ANSWER.as_bytes(), "0.3.0").ok(),
            Some(Found::UpToDate)
        );
        assert_eq!(found(404, b"", "0.2.0").ok(), Some(Found::UpToDate));
        assert!(found(500, b"", "0.2.0").is_err());
        assert!(found(200, b"{not json", "0.2.0").is_err());
        assert!(found(200, b"", "0.2.0").is_err());
    }

    fn download(name: &str) -> Download {
        Download {
            name: name.into(),
            url: format!("https://github.com/o/r/releases/download/v1.0.0/{name}"),
        }
    }

    /// R-UI-12: the Linux packages are the `.deb` and the `.AppImage`, whatever the case
    /// of their ending, and nothing else.
    #[test]
    fn linux_packages_are_the_deb_and_the_appimage() {
        let release = Release {
            version: "1.0.0".into(),
            page: String::new(),
            downloads: vec![
                download("parley_1.0.0_x86_64.APPIMAGE"),
                download("parley_1.0.0_x64_en-US.msi"),
                download("parley_1.0.0_aarch64.dmg"),
                download("parley_1.0.0_amd64.deb"),
                download("æøå😀"),
            ],
        };
        let names: Vec<&str> = release
            .linux_packages()
            .iter()
            .map(|d| d.name.as_str())
            .collect();
        assert_eq!(
            names,
            ["parley_1.0.0_amd64.deb", "parley_1.0.0_x86_64.APPIMAGE"]
        );
    }

    /// R-UI-12: the command downloads the file into the current folder.
    #[test]
    fn the_curl_command_downloads_the_file() {
        assert_eq!(
            download("p.deb").curl_command(),
            "curl -fLO 'https://github.com/o/r/releases/download/v1.0.0/p.deb'"
        );
    }

    /// R-UI-12: a download whose URL is not `https`, or could break out of the command's
    /// quotes, is left out.
    #[test]
    fn unsafe_download_urls_are_left_out() {
        let asset = |url: &str| GitHubAsset {
            name: "p.deb".into(),
            browser_download_url: url.into(),
        };
        assert!(asset("https://github.com/p.deb").download().is_some());
        for bad in [
            "http://github.com/p.deb",
            "https://x.test/'; rm -rf ~; '",
            "https://x.test/a b.deb",
            "https://x.test/a\nb.deb",
            "",
        ] {
            assert_eq!(asset(bad).download(), None, "{bad:?}");
        }
    }

    /// R-UI-11: the request names the app and nothing else about the user: checked against
    /// a fake server on this machine, never GitHub.
    #[test]
    fn the_request_only_names_the_app() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a local port");
        let url = format!("http://{}/latest", listener.local_addr().expect("address"));
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("the check connects");
            let mut request = Vec::new();
            let mut buf = [0; 4096];
            while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                let n = stream.read(&mut buf).expect("the request");
                request.extend_from_slice(&buf[..n]);
            }
            stream
                .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n")
                .expect("the answer");
            String::from_utf8(request).expect("an ASCII request")
        });
        assert_eq!(check(&url, "0.2.0").ok(), Some(Found::UpToDate));
        let request = server.join().expect("the fake server").to_ascii_lowercase();
        assert!(request.starts_with("get /latest http/1.1\r\n"), "{request}");
        assert!(request.contains("accept: application/vnd.github+json\r\n"));
        let agent = format!("user-agent: parley/{}\r\n", env!("CARGO_PKG_VERSION"));
        assert!(request.contains(&agent), "{request}");
        let headers: Vec<&str> = request.lines().skip(1).filter(|l| !l.is_empty()).collect();
        let names: Vec<&str> = headers.iter().filter_map(|h| h.split(':').next()).collect();
        for name in &names {
            assert!(
                ["host", "accept", "user-agent", "accept-encoding"].contains(name),
                "unexpected header {name} in {request}"
            );
        }
    }
}
