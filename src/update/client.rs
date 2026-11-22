use anyhow::{bail, Context, Result};
use reqwest::{blocking::Client, redirect::Policy, StatusCode, Url};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

pub const REPOSITORY: &str = "juancasanovalopez/core_menu";
pub const RELEASES_URL: &str = "https://github.com/juancasanovalopez/core_menu/releases";
pub const TARGET: &str = if cfg!(target_arch = "aarch64") {
    "aarch64-unknown-linux-musl"
} else {
    "x86_64-unknown-linux-musl"
};
pub const MAX_BINARY: u64 = 64 * 1024 * 1024;
pub const MAX_MANIFEST: u64 = 16 * 1024;
pub const MAX_SIGNATURE: u64 = 4096;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Asset {
    pub name: String,
    pub size: u64,
    pub browser_download_url: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Release {
    pub tag_name: String,
    pub html_url: String,
    pub draft: bool,
    pub prerelease: bool,
    pub assets: Vec<Asset>,
}

impl Release {
    pub fn version(&self) -> Result<Version> {
        stable_version(&self.tag_name)
    }

    pub fn asset(&self, name: &str) -> Result<&Asset> {
        let mut matches = self.assets.iter().filter(|asset| asset.name == name);
        let asset = matches
            .next()
            .with_context(|| format!("Falta el archivo {name} en el release"))?;
        if matches.next().is_some() {
            bail!("Archivo duplicado en el release: {name}");
        }
        validate_asset_url(&asset.browser_download_url, &self.tag_name, name)?;
        Ok(asset)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum CheckOutcome {
    Current,
    Available(Release),
    NoRelease,
    Unsupported,
}

pub fn stable_version(tag: &str) -> Result<Version> {
    let version = Version::parse(
        tag.strip_prefix('v')
            .context("El tag debe comenzar con v")?,
    )?;
    if !version.pre.is_empty() {
        bail!("El release no es una versión estable");
    }
    Ok(version)
}

pub fn binary_name(version: &Version) -> String {
    format!("core_menu-v{version}-{TARGET}")
}

pub fn manifest_name() -> String {
    format!("update-manifest-{TARGET}.json")
}

pub fn signature_name() -> String {
    format!("update-manifest-{TARGET}.sig")
}

fn approved_host(url: &Url) -> bool {
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none()
        && matches!(
            url.host_str(),
            Some(
                "api.github.com"
                    | "github.com"
                    | "release-assets.githubusercontent.com"
                    | "objects.githubusercontent.com"
            )
        )
}

fn validate_asset_url(url: &str, tag: &str, name: &str) -> Result<()> {
    let expected = format!("https://github.com/{REPOSITORY}/releases/download/{tag}/{name}");
    if url != expected {
        bail!("Origen del archivo de actualización no autorizado");
    }
    Ok(())
}

pub struct Http {
    client: Client,
}

impl Http {
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .user_agent(concat!("core_menu/", env!("CARGO_PKG_VERSION")))
            .https_only(true)
            .connect_timeout(Duration::from_secs(2))
            .redirect(Policy::custom(|attempt| {
                if attempt.previous().len() >= 5 || !approved_host(attempt.url()) {
                    attempt.error("Redirección de actualización no autorizada")
                } else {
                    attempt.follow()
                }
            }))
            .build()?;
        Ok(Self { client })
    }

    pub fn check(&self) -> Result<CheckOutcome> {
        if !cfg!(all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        )) {
            return Ok(CheckOutcome::Unsupported);
        }
        self.check_at(
            &format!("https://api.github.com/repos/{REPOSITORY}/releases/latest"),
            Duration::from_secs(5),
        )
    }

    fn check_at(&self, endpoint: &str, timeout: Duration) -> Result<CheckOutcome> {
        let response = self
            .client
            .get(endpoint)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .timeout(timeout)
            .send()
            .context("No se pudo comprobar la actualización")?;
        match response.status() {
            StatusCode::NOT_FOUND => return Ok(CheckOutcome::NoRelease),
            StatusCode::FORBIDDEN | StatusCode::TOO_MANY_REQUESTS => {
                bail!("GitHub ha limitado la consulta de actualizaciones")
            }
            status if !status.is_success() => bail!("Consulta GitHub fallida: HTTP {status}"),
            _ => {}
        }
        let bytes = bounded_read(response, 512 * 1024)?;
        assess_release(serde_json::from_slice(&bytes).context("Respuesta GitHub no válida")?)
    }

    pub fn download(
        &self,
        asset: &Asset,
        limit: u64,
        deadline: Instant,
        cancel: &AtomicBool,
        output: &mut impl Write,
        progress: impl FnMut(u64),
    ) -> Result<()> {
        if asset.size == 0 || asset.size > limit {
            bail!("Tamaño de archivo no permitido: {}", asset.name);
        }
        let url = Url::parse(&asset.browser_download_url)?;
        if !approved_host(&url) {
            bail!("Origen de descarga no autorizado");
        }
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .context("La descarga excedió el tiempo de espera")?;
        let response = self
            .client
            .get(url)
            .timeout(remaining)
            .send()
            .context("No se pudo descargar la actualización")?;
        if !response.status().is_success() {
            bail!("Descarga fallida: HTTP {}", response.status());
        }
        copy_download(
            response, asset.size, limit, deadline, cancel, output, progress,
        )
    }
}

fn copy_download(
    mut response: impl Read,
    expected: u64,
    limit: u64,
    deadline: Instant,
    cancel: &AtomicBool,
    output: &mut impl Write,
    mut progress: impl FnMut(u64),
) -> Result<()> {
    let mut bytes = [0; 32 * 1024];
    let mut received = 0;
    loop {
        if cancel.load(Ordering::Relaxed) {
            bail!("Descarga cancelada");
        }
        if Instant::now() >= deadline {
            bail!("La descarga excedió el tiempo de espera");
        }
        let size = response.read(&mut bytes).context("Descarga interrumpida")?;
        if size == 0 {
            break;
        }
        received += size as u64;
        if received > expected || received > limit {
            bail!("El archivo supera el tamaño anunciado");
        }
        output.write_all(&bytes[..size])?;
        progress(received);
    }
    if received != expected {
        bail!("Archivo descargado incompleto");
    }
    Ok(())
}

pub fn bounded_read(reader: impl Read, limit: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        bail!("Respuesta de actualización demasiado grande");
    }
    Ok(bytes)
}

fn assess_release(release: Release) -> Result<CheckOutcome> {
    if release.draft || release.prerelease {
        bail!("GitHub no devolvió un release estable publicado");
    }
    let version = release.version()?;
    if release.html_url != format!("{RELEASES_URL}/tag/{}", release.tag_name) {
        bail!("Enlace del release no autorizado");
    }
    if !version
        .cmp_precedence(&Version::parse(env!("CARGO_PKG_VERSION"))?)
        .is_gt()
    {
        return Ok(CheckOutcome::Current);
    }
    for (name, limit) in [
        (binary_name(&version), MAX_BINARY),
        (manifest_name(), MAX_MANIFEST),
        (signature_name(), MAX_SIGNATURE),
    ] {
        let asset = release.asset(&name)?;
        if asset.size == 0 || asset.size > limit {
            bail!("Tamaño de archivo no permitido: {name}");
        }
    }
    Ok(CheckOutcome::Available(release))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{net::TcpListener, thread};

    pub(crate) fn release(version: &str) -> Release {
        let tag = format!("v{version}");
        Release {
            tag_name: tag.clone(),
            html_url: format!("{RELEASES_URL}/tag/{tag}"),
            draft: false,
            prerelease: false,
            assets: [
                format!("core_menu-{tag}-{TARGET}"),
                manifest_name(),
                signature_name(),
            ]
            .into_iter()
            .map(|name| Asset {
                browser_download_url: format!(
                    "https://github.com/{REPOSITORY}/releases/download/{tag}/{name}"
                ),
                name,
                size: 100,
            })
            .collect(),
        }
    }

    #[test]
    fn stable_versions_are_compared_numerically() {
        assert!(matches!(
            assess_release(release(env!("CARGO_PKG_VERSION"))).unwrap(),
            CheckOutcome::Current
        ));
        assert!(matches!(
            assess_release(release("0.1.0")).unwrap(),
            CheckOutcome::Current
        ));
        assert!(matches!(
            assess_release(release("99.10.0")).unwrap(),
            CheckOutcome::Available(_)
        ));
        for tag in ["1.0.1", "v1.0.1-beta", "vbad", "v01.0.0"] {
            assert!(stable_version(tag).is_err());
        }
        let mut invalid = release("99.0.0");
        invalid.prerelease = true;
        assert!(assess_release(invalid).is_err());
        assert!(stable_version("v1.10.0").unwrap() > stable_version("v1.9.0").unwrap());
        let current = Version::parse(env!("CARGO_PKG_VERSION")).unwrap();
        let versions = [
            format!("{}.{}.{}", current.major, current.minor, current.patch + 1),
            format!("{}.{}.0", current.major, current.minor + 1),
            format!("{}.0.0", current.major + 1),
        ];
        for version in versions {
            assert!(matches!(
                assess_release(release(&version)).unwrap(),
                CheckOutcome::Available(_)
            ));
        }
        let same = format!(
            "{}.{}.{}+build.2",
            current.major, current.minor, current.patch
        );
        assert!(matches!(
            assess_release(release(&same)).unwrap(),
            CheckOutcome::Current
        ));
        assert!(matches!(
            assess_release(release("99.0.0+build.2")).unwrap(),
            CheckOutcome::Available(_)
        ));
    }

    #[test]
    fn assets_and_origins_are_checked() {
        let mut value = release("99.0.0");
        value.assets[0].browser_download_url = "https://example.org/binary".into();
        assert!(assess_release(value).is_err());
        let mut value = release("99.0.0");
        value.assets.pop();
        assert!(assess_release(value).is_err());
        for url in [
            "http://github.com/file",
            "https://github.com.evil.test/file",
            "https://user@github.com/file",
            "https://github.com:444/file",
        ] {
            assert!(!approved_host(&Url::parse(url).unwrap()));
        }
        assert!(bounded_read(&b"abc"[..], 2).is_err());
        let mut value = release("99.0.0");
        value.assets.push(value.assets[0].clone());
        assert!(assess_release(value).is_err());
        let mut value = release("99.0.0");
        value.html_url = format!("{RELEASES_URL}/v99.0.0");
        assert!(assess_release(value).is_err());
        let mut value = release("99.0.0");
        value.assets[0].size = MAX_BINARY + 1;
        assert!(assess_release(value).is_err());
        let mut value = release("99.0.0");
        value.assets[0].name = "core_menu-v99.0.0-wrong-target".into();
        assert!(assess_release(value).is_err());
    }

    #[test]
    fn asset_contract_uses_the_native_target_consistently() {
        assert_eq!(manifest_name(), format!("update-manifest-{TARGET}.json"));
        assert_eq!(signature_name(), format!("update-manifest-{TARGET}.sig"));
        assert_eq!(
            binary_name(&Version::parse("1.2.3").unwrap()),
            format!("core_menu-v1.2.3-{TARGET}")
        );
        if cfg!(target_arch = "aarch64") {
            assert_eq!(TARGET, "aarch64-unknown-linux-musl");
        } else {
            assert_eq!(TARGET, "x86_64-unknown-linux-musl");
        }
    }

    fn mock(
        status: u16,
        body: impl Into<String>,
        delay: Duration,
    ) -> (String, thread::JoinHandle<()>) {
        let body = body.into();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let worker = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 2048];
            assert!(stream.read(&mut request).unwrap() > 0);
            thread::sleep(delay);
            let response = format!(
                "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            // A timeout test closes the connection before the response.
            let _ = stream.write_all(response.as_bytes());
        });
        (format!("http://{address}"), worker)
    }

    #[test]
    fn http_errors_and_timeout_do_not_produce_available_updates() {
        let http = Http {
            client: Client::builder().no_proxy().build().unwrap(),
        };
        for (status, body) in [(404, ""), (403, ""), (429, ""), (500, ""), (200, "{bad")] {
            let (url, worker) = mock(status, body, Duration::ZERO);
            let result = http.check_at(&url, Duration::from_secs(1));
            if status == 404 {
                assert!(matches!(result.unwrap(), CheckOutcome::NoRelease));
            } else {
                assert!(result.is_err());
            }
            worker.join().unwrap();
        }
        let (url, worker) = mock(200, "{}", Duration::from_millis(200));
        let start = Instant::now();
        assert!(http.check_at(&url, Duration::from_millis(50)).is_err());
        assert!(start.elapsed() < Duration::from_millis(180));
        worker.join().unwrap();
    }

    #[test]
    fn github_response_with_real_tag_url_offers_a_complete_release() {
        let http = Http {
            client: Client::builder().no_proxy().build().unwrap(),
        };
        let body = serde_json::to_string(&release("99.0.0")).unwrap();
        let (url, worker) = mock(200, body, Duration::ZERO);
        assert!(matches!(
            http.check_at(&url, Duration::from_secs(1)).unwrap(),
            CheckOutcome::Available(_)
        ));
        worker.join().unwrap();
    }

    #[test]
    fn download_stream_enforces_size_cancellation_deadline_and_progress() {
        let cancel = AtomicBool::new(false);
        let deadline = Instant::now() + Duration::from_secs(1);
        let mut output = Vec::new();
        let mut progress = Vec::new();
        copy_download(
            &b"abc"[..],
            3,
            10,
            deadline,
            &cancel,
            &mut output,
            |bytes| progress.push(bytes),
        )
        .unwrap();
        assert_eq!(output, b"abc");
        assert_eq!(progress, vec![3]);
        for (body, expected, limit) in [
            (&b"ab"[..], 3, 10),
            (&b"abcd"[..], 3, 10),
            (&b"abc"[..], 3, 2),
        ] {
            assert!(copy_download(
                body,
                expected,
                limit,
                deadline,
                &cancel,
                &mut Vec::new(),
                |_| {}
            )
            .is_err());
        }
        cancel.store(true, Ordering::Relaxed);
        let mut output = Vec::new();
        assert!(copy_download(&b"abc"[..], 3, 10, deadline, &cancel, &mut output, |_| {}).is_err());
        assert!(output.is_empty());
        cancel.store(false, Ordering::Relaxed);
        assert!(copy_download(
            &b"abc"[..],
            3,
            10,
            Instant::now(),
            &cancel,
            &mut output,
            |_| {}
        )
        .is_err());
        assert!(output.is_empty());
    }
}
