use super::{
    client::{bounded_read, MAX_BINARY, MAX_MANIFEST, MAX_SIGNATURE},
    verify::{self, Manifest},
    Prepared,
};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    os::unix::{
        fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
        io::AsRawFd,
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Fingerprint {
    device: u64,
    inode: u64,
    hash: String,
}

#[derive(Debug)]
pub struct Destination {
    pub path: PathBuf,
    pub elevated: bool,
    pub fingerprint: Fingerprint,
}

#[cfg(test)]
pub(super) fn fixture_destination(path: &Path) -> Destination {
    Destination {
        path: path.into(),
        elevated: false,
        fingerprint: fingerprint(path).unwrap(),
    }
}

fn open_regular(path: &Path) -> Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .with_context(|| format!("No se pudo abrir {}", path.display()))?;
    if !file.metadata()?.is_file() {
        bail!("El destino no es un archivo regular");
    }
    Ok(file)
}

fn fingerprint(path: &Path) -> Result<Fingerprint> {
    let file = open_regular(path)?;
    let metadata = file.metadata()?;
    let (_, hash) = verify::digest(file, MAX_BINARY)?;
    Ok(Fingerprint {
        device: metadata.dev(),
        inode: metadata.ino(),
        hash,
    })
}

fn trusted_system_path(path: &Path) -> Result<()> {
    if path != Path::new("/usr/local/bin/core_menu") {
        bail!("Instalación del sistema: usa el gestor de paquetes o instala en /usr/local/bin/core_menu");
    }
    for ancestor in path.ancestors() {
        let metadata = fs::symlink_metadata(ancestor)?;
        if metadata.file_type().is_symlink() || metadata.uid() != 0 || metadata.mode() & 0o022 != 0
        {
            bail!(
                "Ruta de instalación del sistema no segura: {}",
                ancestor.display()
            );
        }
    }
    Ok(())
}

fn package_managed(path: &Path) -> Result<bool> {
    for (tool, query) in [("/usr/bin/dpkg-query", "-S"), ("/usr/bin/rpm", "-qf")] {
        if !Path::new(tool).exists() {
            continue;
        }
        let mut child = Command::new(tool)
            .arg(query)
            .arg(path)
            .env("LC_ALL", "C")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        let deadline = Instant::now() + Duration::from_secs(2);
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }
            if Instant::now() >= deadline {
                child.kill()?;
                child.wait()?;
                bail!("Tiempo de espera consultando el gestor de paquetes");
            }
            thread::sleep(Duration::from_millis(10));
        };
        if status.success() {
            return Ok(true);
        }
        if status.code() != Some(1) {
            bail!("No se pudo consultar el gestor de paquetes {tool}");
        }
    }
    Ok(false)
}

pub fn destination() -> Result<Destination> {
    if unsafe { libc::geteuid() } == 0 {
        bail!("Reinicia sin sudo para descargar e instalar; la elevación se solicita al final");
    }
    let path = std::env::current_exe()?.canonicalize()?;
    let metadata = fs::symlink_metadata(&path)?;
    let uid = unsafe { libc::geteuid() };
    let elevated = metadata.uid() == 0;
    if metadata.mode() & 0o6000 != 0 {
        bail!("No se actualizan ejecutables setuid/setgid");
    }
    if elevated {
        trusted_system_path(&path)?;
    } else if metadata.uid() != uid {
        bail!("El ejecutable pertenece a otro usuario");
    }
    if package_managed(&path)? {
        bail!("Este ejecutable pertenece a un paquete: actualízalo con el gestor de paquetes");
    }
    if !elevated {
        let parent = path.parent().context("Destino sin directorio")?;
        tempfile::tempfile_in(parent).context("El directorio de instalación no es escribible")?;
    }
    Ok(Destination {
        fingerprint: fingerprint(&path)?,
        path,
        elevated,
    })
}

pub fn install(prepared: &mut Prepared) -> Result<PathBuf> {
    if prepared.destination.elevated {
        elevated_install(prepared)
    } else {
        let manifest = verify::authenticate(&prepared.manifest, &prepared.signature)?;
        prepared.binary.seek(SeekFrom::Start(0))?;
        replace(
            &prepared.destination.path,
            &prepared.destination.fingerprint,
            &mut prepared.binary,
            &manifest,
        )
    }
}

fn elevated_install(prepared: &mut Prepared) -> Result<PathBuf> {
    trusted_system_path(&prepared.destination.path)?;
    if !Path::new("/usr/bin/sudo").is_file() {
        bail!("sudo no está instalado");
    }
    println!(
        "Se necesita sudo para actualizar {}.",
        prepared.destination.path.display()
    );
    let mut child = Command::new("/usr/bin/sudo")
        .arg("--")
        .arg(&prepared.destination.path)
        .arg("--internal-install-update")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .context("No se pudo iniciar sudo")?;
    let write_result = (|| -> Result<()> {
        let mut input = child
            .stdin
            .take()
            .context("No se pudo abrir el canal del instalador")?;
        let expected = serde_json::to_vec(&prepared.destination.fingerprint)?;
        for data in [&expected, &prepared.manifest, &prepared.signature] {
            input.write_all(&(data.len() as u64).to_le_bytes())?;
            input.write_all(data)?;
        }
        prepared.binary.seek(SeekFrom::Start(0))?;
        std::io::copy(&mut prepared.binary, &mut input)?;
        Ok(())
    })();
    let output = bounded_read(
        child
            .stdout
            .take()
            .context("Sin respuesta del instalador")?,
        64 * 1024,
    );
    let status = child.wait()?;
    if !status.success() {
        bail!("Instalación rechazada o fallida (sudo: {status}); revisa el mensaje del instalador");
    }
    write_result.context("No se pudo transmitir la actualización verificada")?;
    serde_json::from_slice(&output?).context("Respuesta del instalador no válida")
}

pub fn helper() -> Result<()> {
    if unsafe { libc::geteuid() } != 0 {
        bail!("El ayudante de instalación necesita sudo");
    }
    let path = std::env::current_exe()?.canonicalize()?;
    trusted_system_path(&path)?;
    if package_managed(&path)? {
        bail!("El ejecutable está administrado por un gestor de paquetes");
    }
    let mut input = std::io::stdin().lock();
    let expected: Fingerprint = serde_json::from_slice(&read_frame(&mut input, 4096)?)?;
    let bytes = read_frame(&mut input, MAX_MANIFEST)?;
    let signature = read_frame(&mut input, MAX_SIGNATURE)?;
    let manifest = verify::authenticate(&bytes, &signature)?;
    let backup = replace(&path, &expected, &mut input, &manifest)?;
    serde_json::to_writer(std::io::stdout().lock(), &backup)?;
    Ok(())
}

fn read_frame(input: &mut impl Read, limit: u64) -> Result<Vec<u8>> {
    let mut length = [0; 8];
    input.read_exact(&mut length)?;
    let length = u64::from_le_bytes(length);
    if length == 0 || length > limit {
        bail!("Paquete de instalación fuera de rango");
    }
    let data = bounded_read(input.take(length), limit)?;
    if data.len() as u64 != length {
        bail!("Paquete de instalación incompleto");
    }
    Ok(data)
}

fn replace(
    path: &Path,
    expected: &Fingerprint,
    source: &mut impl Read,
    manifest: &Manifest,
) -> Result<PathBuf> {
    let parent = path.parent().context("Destino sin directorio")?;
    let metadata = fs::symlink_metadata(path)?;
    let uid = unsafe { libc::geteuid() };
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.uid() != uid
        || metadata.mode() & 0o6000 != 0
    {
        bail!("Propietario o tipo del ejecutable no autorizado");
    }
    let lock_path = parent.join(".core_menu.update.lock");
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&lock_path)?;
    let lock_meta = lock.metadata()?;
    if !lock_meta.is_file() || lock_meta.uid() != uid || lock_meta.mode() & 0o077 != 0 {
        bail!("Archivo de bloqueo no seguro");
    }
    // Holding this descriptor keeps the installation lock until every exit path.
    if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err(std::io::Error::last_os_error()).context("Otra actualización está en curso");
    }
    if &fingerprint(path)? != expected {
        bail!("El ejecutable cambió desde la confirmación; vuelve a iniciar la aplicación");
    }
    let staging = tempfile::Builder::new()
        .prefix(".core_menu-recovery-")
        .tempdir_in(parent)?;
    let new_path = staging.path().join("new");
    let backup = staging.path().join("previous");
    let mut new = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&new_path)?;
    let copied = std::io::copy(&mut source.take(manifest.size + 1), &mut new)?;
    if copied != manifest.size {
        bail!("Binario de instalación incompleto o demasiado grande");
    }
    new.seek(SeekFrom::Start(0))?;
    verify::binary(&mut new, manifest)?;
    new.set_permissions(fs::Permissions::from_mode(0o755))?;
    new.sync_all()?;
    fs::hard_link(path, &backup).context("No se pudo conservar la versión anterior")?;
    if &fingerprint(path)? != expected || &fingerprint(&backup)? != expected {
        bail!("El ejecutable cambió durante la instalación");
    }
    let directory = File::open(parent)?;
    File::open(staging.path())?.sync_all()?;
    directory.sync_all()?;
    fs::rename(&new_path, path).context("No se pudo reemplazar atómicamente el ejecutable")?;
    if let Err(error) = directory.sync_all() {
        if let Err(rollback) = fs::rename(&backup, path).and_then(|_| directory.sync_all()) {
            let recovery = staging.keep();
            bail!("Reemplazo efectuado pero no sincronizado: {error}; recuperación fallida: {rollback}. Copia en {}", recovery.display());
        }
        return Err(error)
            .context("No se pudo sincronizar la actualización; versión anterior restaurada");
    }
    let recovery = staging.keep();
    Ok(recovery.join("previous"))
}

#[cfg(test)]
mod tests {
    use super::super::verify;
    use super::*;

    #[test]
    fn replacement_is_atomic_and_retains_original() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("binary with spaces");
        fs::write(&path, b"old executable").unwrap();
        let expected = fingerprint(&path).unwrap();
        let (bytes, _, _, content) = verify::signed_fixture();
        let manifest: Manifest = serde_json::from_slice(&bytes).unwrap();
        let backup = replace(&path, &expected, &mut &content[..], &manifest).unwrap();
        assert_eq!(fs::read(&path).unwrap(), content);
        assert_eq!(fs::read(backup).unwrap(), b"old executable");
        assert_eq!(fs::metadata(&path).unwrap().mode() & 0o7777, 0o755);
    }

    #[test]
    fn corrupted_changed_and_symlink_destinations_keep_old_binary() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("binary");
        fs::write(&path, b"old").unwrap();
        let expected = fingerprint(&path).unwrap();
        let (bytes, _, _, content) = verify::signed_fixture();
        let manifest: Manifest = serde_json::from_slice(&bytes).unwrap();
        assert!(replace(&path, &expected, &mut &b"bad"[..], &manifest).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"old");
        fs::write(&path, b"changed").unwrap();
        assert!(replace(&path, &expected, &mut &content[..], &manifest).is_err());
        let link = directory.path().join("link");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(fingerprint(&link).is_err());
        assert!(replace(&link, &expected, &mut &content[..], &manifest).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"changed");
    }

    #[test]
    fn invalid_frames_and_unsafe_system_paths_are_rejected() {
        assert!(read_frame(&mut &0u64.to_le_bytes()[..], 10).is_err());
        assert!(read_frame(&mut &11u64.to_le_bytes()[..], 10).is_err());
        assert!(read_frame(&mut &1u64.to_le_bytes()[..], 10).is_err());
        assert!(trusted_system_path(Path::new("/usr/bin/core_menu")).is_err());
        assert!(trusted_system_path(Path::new("/tmp/core_menu")).is_err());
    }

    #[test]
    fn concurrent_install_and_read_failures_preserve_the_original() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("binary");
        fs::write(&path, b"old").unwrap();
        let expected = fingerprint(&path).unwrap();
        let (bytes, _, _, content) = verify::signed_fixture();
        let manifest: Manifest = serde_json::from_slice(&bytes).unwrap();
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(directory.path().join(".core_menu.update.lock"))
            .unwrap();
        assert_eq!(
            unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
            0
        );
        assert!(replace(&path, &expected, &mut &content[..], &manifest).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"old");
        drop(lock);

        struct Interrupted;
        impl Read for Interrupted {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("simulated source failure"))
            }
        }
        assert!(replace(&path, &expected, &mut Interrupted, &manifest).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"old");
        assert!(replace(
            &path,
            &expected,
            &mut &b"too much data for this manifest"[..],
            &manifest
        )
        .is_err());
        assert_eq!(fs::read(&path).unwrap(), b"old");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 2);
    }
}
