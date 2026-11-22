mod client;
pub mod install;
mod verify;

use anyhow::{bail, Context, Result};
use client::{manifest_name, signature_name, Http, MAX_BINARY, MAX_MANIFEST, MAX_SIGNATURE};
pub use client::{CheckOutcome, Release, RELEASES_URL};
use std::{
    fs::File,
    io::{Seek, SeekFrom},
    os::unix::process::CommandExt,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread,
    time::{Duration, Instant},
};
#[cfg(test)]
use tempfile::NamedTempFile;

pub struct Prepared {
    pub release: Release,
    pub destination: install::Destination,
    manifest: Vec<u8>,
    signature: Vec<u8>,
    binary: File,
}

#[cfg(test)]
impl Prepared {
    pub(crate) fn fixture() -> Self {
        let (manifest, signature, _, _) = verify::signed_fixture();
        let binary = NamedTempFile::new().unwrap();
        Self {
            release: Release {
                tag_name: "v99.0.0".into(),
                html_url: format!("{RELEASES_URL}/tag/v99.0.0"),
                draft: false,
                prerelease: false,
                assets: Vec::new(),
            },
            destination: install::fixture_destination(binary.path()),
            manifest,
            signature,
            binary: binary.into_file(),
        }
    }
}

pub enum Event {
    Checked(Result<CheckOutcome, String>),
    Progress(u64, u64),
    Downloaded(Result<Prepared, String>),
}

pub enum State {
    Checking,
    Checked(CheckOutcome),
    Downloading,
    Ready(Prepared),
    Error(String),
    Cancelled,
}

impl State {
    pub fn summary(&self) -> String {
        match self {
            Self::Checking => "Actualizaciones: comprobando…".into(),
            Self::Checked(CheckOutcome::Current) => {
                format!("Actualizaciones: v{} al día", env!("CARGO_PKG_VERSION"))
            }
            Self::Checked(CheckOutcome::Available(release)) => {
                format!("Nueva versión {} · u: detalles", release.tag_name)
            }
            Self::Checked(CheckOutcome::NoRelease) => {
                "Actualizaciones: no hay release estable accesible".into()
            }
            Self::Checked(CheckOutcome::Unsupported) => {
                "Actualizaciones: arquitectura no compatible".into()
            }
            Self::Downloading => "Actualizaciones: descargando y verificando · u: progreso".into(),
            Self::Ready(_) => "Actualización verificada · u: confirmar instalación".into(),
            Self::Error(error) => format!("Actualizaciones: Error: {error} · u: detalles"),
            Self::Cancelled => "Actualización cancelada; el ejecutable no se ha modificado".into(),
        }
    }
}

pub struct Updater {
    pub state: State,
    pub progress: Option<(u64, u64)>,
    sender: mpsc::Sender<Event>,
    receiver: mpsc::Receiver<Event>,
    cancel: Arc<AtomicBool>,
    started: bool,
}

impl Updater {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            state: State::Checking,
            progress: None,
            sender,
            receiver,
            cancel: Arc::new(AtomicBool::new(false)),
            started: false,
        }
    }

    pub fn start(&mut self) -> Result<()> {
        self.start_with(check_unprivileged)
    }

    fn start_with(
        &mut self,
        check: impl FnOnce() -> Result<CheckOutcome> + Send + 'static,
    ) -> Result<()> {
        if self.started {
            bail!("La comprobación ya se inició en esta ejecución");
        }
        self.started = true;
        let sender = self.sender.clone();
        thread::Builder::new()
            .name("update-check".into())
            .spawn(move || {
                let result = check().map_err(|error| format!("{error:#}"));
                let _ = sender.send(Event::Checked(result));
            })
            .context("No se pudo iniciar la comprobación de actualizaciones")?;
        Ok(())
    }

    pub fn poll(&mut self) {
        while let Ok(event) = self.receiver.try_recv() {
            match event {
                Event::Checked(Ok(result)) => self.state = State::Checked(result),
                Event::Checked(Err(error)) => self.state = State::Error(error),
                Event::Progress(bytes, total) => {
                    if matches!(self.state, State::Downloading) {
                        self.progress = Some((bytes, total));
                    }
                }
                Event::Downloaded(result) => {
                    if matches!(self.state, State::Cancelled) {
                        continue;
                    }
                    self.state = match result {
                        Ok(prepared) => State::Ready(prepared),
                        Err(error) => State::Error(error),
                    };
                }
            }
        }
    }

    pub fn download(&mut self) -> Result<()> {
        let State::Checked(CheckOutcome::Available(release)) = &self.state else {
            bail!("No hay actualización disponible para descargar");
        };
        let release = release.clone();
        let destination = install::destination()?;
        let sender = self.sender.clone();
        let cancel = self.cancel.clone();
        self.cancel.store(false, Ordering::Relaxed);
        thread::Builder::new()
            .name("update-download".into())
            .spawn(move || {
                let result = prepare(release, destination, &cancel, |bytes, total| {
                    let _ = sender.send(Event::Progress(bytes, total));
                })
                .map_err(|error| format!("{error:#}"));
                let _ = sender.send(Event::Downloaded(result));
            })
            .context("No se pudo iniciar la descarga")?;
        self.progress = None;
        self.state = State::Downloading;
        Ok(())
    }

    pub fn cancel(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        if matches!(self.state, State::Downloading | State::Ready(_)) {
            self.state = State::Cancelled;
        }
    }
}

impl Drop for Updater {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

fn prepare(
    release: Release,
    destination: install::Destination,
    cancel: &AtomicBool,
    progress: impl Fn(u64, u64),
) -> Result<Prepared> {
    let http = Http::new()?;
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut manifest = Vec::new();
    let mut signature = Vec::new();
    http.download(
        release.asset(&manifest_name())?,
        MAX_MANIFEST,
        deadline,
        cancel,
        &mut manifest,
        |_| {},
    )?;
    http.download(
        release.asset(&signature_name())?,
        MAX_SIGNATURE,
        deadline,
        cancel,
        &mut signature,
        |_| {},
    )?;
    let info = verify::authenticate(&manifest, &signature)?;
    if info.version != release.version()? {
        bail!("El manifiesto no corresponde a la versión anunciada");
    }
    let asset = release.asset(&info.asset)?;
    if asset.size != info.size {
        bail!("El tamaño del release no coincide con el manifiesto firmado");
    }
    let mut binary = tempfile::tempfile()?;
    http.download(asset, MAX_BINARY, deadline, cancel, &mut binary, |bytes| {
        progress(bytes, info.size)
    })?;
    binary.seek(SeekFrom::Start(0))?;
    verify::binary(&mut binary, &info)?;
    binary.sync_all()?;
    if cancel.load(Ordering::Relaxed) {
        bail!("Descarga cancelada");
    }
    Ok(Prepared {
        release,
        destination,
        manifest,
        signature,
        binary,
    })
}

pub fn check() -> Result<CheckOutcome> {
    if unsafe { libc::geteuid() } == 0 {
        bail!("La comprobación de red debe ejecutarse sin privilegios");
    }
    Http::new()?.check()
}

fn check_unprivileged() -> Result<CheckOutcome> {
    if unsafe { libc::geteuid() } != 0 {
        return check();
    }
    let uid: u32 = std::env::var("SUDO_UID")
        .context("Iniciado como root: reinicia sin sudo para comprobar actualizaciones")?
        .parse()?;
    let gid: u32 = std::env::var("SUDO_GID")?.parse()?;
    if uid == 0 || gid == 0 {
        bail!("No hay una identidad sin privilegios para consultar actualizaciones");
    }
    // Command::uid clears supplementary groups before dropping privileges.
    let mut command = Command::new(std::env::current_exe()?);
    command
        .arg("--internal-update-check")
        .env_clear()
        .uid(uid)
        .gid(gid)
        .stdin(Stdio::null());
    let bytes = worker_output(&mut command, Duration::from_secs(6))?;
    serde_json::from_slice(&bytes).context("Respuesta del trabajador no válida")
}

fn worker_output(command: &mut Command, timeout: Duration) -> Result<Vec<u8>> {
    // Files avoid pipe backpressure while the parent monitors the worker deadline.
    let mut stdout = tempfile::tempfile()?;
    let mut stderr = tempfile::tempfile()?;
    command
        .stdout(Stdio::from(stdout.try_clone()?))
        .stderr(Stdio::from(stderr.try_clone()?));
    let mut child = command
        .spawn()
        .context("No se pudo iniciar la consulta sin privilegios")?;
    let deadline = Instant::now() + timeout;
    loop {
        let status = child.try_wait()?;
        if stdout.metadata()?.len() > 512 * 1024 || stderr.metadata()?.len() > 16 * 1024 {
            if status.is_none() {
                child.kill()?;
                child.wait()?;
            }
            bail!("Respuesta del trabajador demasiado grande");
        }
        if let Some(status) = status {
            if !status.success() {
                stderr.seek(SeekFrom::Start(0))?;
                let error = client::bounded_read(&mut stderr, 16 * 1024)?;
                bail!(
                    "Consulta sin privilegios fallida: {}",
                    String::from_utf8_lossy(&error).trim()
                );
            }
            stdout.seek(SeekFrom::Start(0))?;
            return client::bounded_read(&mut stdout, 512 * 1024);
        }
        if Instant::now() >= deadline {
            child.kill()?;
            child.wait()?;
            bail!("La consulta sin privilegios excedió el tiempo de espera");
        }
        thread::sleep(Duration::from_millis(20));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_is_asynchronous_and_can_only_be_started_once() {
        let mut updater = Updater::new();
        let (release_worker, wait) = mpsc::channel();
        let start = Instant::now();
        updater
            .start_with(move || {
                wait.recv().unwrap();
                Ok(CheckOutcome::Current)
            })
            .unwrap();
        assert!(start.elapsed() < Duration::from_millis(100));
        assert!(matches!(updater.state, State::Checking));
        assert!(updater.start().is_err());
        release_worker.send(()).unwrap();
        let event = updater
            .receiver
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        updater.sender.send(event).unwrap();
        updater.poll();
        assert!(matches!(
            updater.state,
            State::Checked(CheckOutcome::Current)
        ));
    }

    #[test]
    fn worker_output_exceeds_pipe_capacity_without_deadlock_and_is_bounded() {
        let mut command = Command::new("/usr/bin/head");
        command.args(["-c", "262144", "/dev/zero"]);
        assert_eq!(
            worker_output(&mut command, Duration::from_secs(2))
                .unwrap()
                .len(),
            262144
        );
        let mut command = Command::new("/usr/bin/head");
        command.args(["-c", "524289", "/dev/zero"]);
        assert!(worker_output(&mut command, Duration::from_secs(2)).is_err());
        let mut command = Command::new("/usr/bin/sleep");
        command.arg("2");
        let start = Instant::now();
        assert!(worker_output(&mut command, Duration::from_millis(50)).is_err());
        assert!(start.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn cancellation_discards_late_results() {
        let mut updater = Updater::new();
        updater.state = State::Downloading;
        updater.cancel();
        updater
            .sender
            .send(Event::Downloaded(Err("cancelled network".into())))
            .unwrap();
        updater.poll();
        assert!(matches!(updater.state, State::Cancelled));
        assert!(updater.cancel.load(Ordering::Relaxed));
        updater
            .sender
            .send(Event::Downloaded(Ok(Prepared::fixture())))
            .unwrap();
        updater.poll();
        assert!(matches!(updater.state, State::Cancelled));
    }

    #[test]
    fn ready_update_can_be_cancelled_without_changing_the_executable() {
        let executable = NamedTempFile::new().unwrap();
        std::fs::write(executable.path(), b"old executable").unwrap();
        let mut updater = Updater::new();
        let mut prepared = Prepared::fixture();
        prepared.destination = install::fixture_destination(executable.path());
        updater.state = State::Ready(prepared);
        updater.cancel();
        assert!(matches!(updater.state, State::Cancelled));
        assert_eq!(std::fs::read(executable.path()).unwrap(), b"old executable");
    }
}
