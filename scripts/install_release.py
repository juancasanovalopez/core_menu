#!/usr/bin/env python3
"""Install an authenticated core_menu release without a Rust toolchain."""

import fcntl
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import stat
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request

REPOSITORY = "juancasanovalopez/core_menu"
PUBLIC_KEY = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIGUfkDLzRtKVOGStC7qK6cOQue5KQdI+y4U9vdBaQ3zF"
VERSION = re.compile(r"v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\Z")
HASH = re.compile(r"[0-9a-f]{64}\Z")
TARGETS = {
    "x86_64": "x86_64-unknown-linux-musl",
    "aarch64": "aarch64-unknown-linux-musl",
    "arm64": "aarch64-unknown-linux-musl",
}
HOSTS = {"api.github.com", "github.com", "release-assets.githubusercontent.com", "objects.githubusercontent.com"}
MAX_BINARY = 64 * 1024 * 1024
MAX_MANIFEST = 16 * 1024
MAX_SIGNATURE = 4096


class InstallError(Exception):
    pass


def version(tag):
    match = VERSION.fullmatch(tag)
    if not match:
        raise InstallError("El release no tiene un tag estable válido.")
    return tuple(int(part) for part in match.groups())


def approved_url(url):
    parsed = urllib.parse.urlsplit(url)
    return (
        parsed.scheme == "https"
        and parsed.hostname in HOSTS
        and parsed.username is None
        and parsed.password is None
        and parsed.port is None
    )


class SafeRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        if not approved_url(newurl):
            raise InstallError("Redirección de descarga no autorizada.")
        return super().redirect_request(req, fp, code, msg, headers, newurl)


def download(url, limit, deadline):
    if not approved_url(url):
        raise InstallError("Origen de descarga no autorizado.")
    remaining = deadline - time.monotonic()
    if remaining <= 0:
        raise InstallError("Tiempo de descarga agotado.")
    request = urllib.request.Request(url, headers={"User-Agent": "core_menu-installer", "Accept": "application/vnd.github+json"})
    opener = urllib.request.build_opener(SafeRedirect())
    with opener.open(request, timeout=min(remaining, 15)) as response:
        chunks = []
        size = 0
        while True:
            if time.monotonic() >= deadline:
                raise InstallError("Tiempo de descarga agotado.")
            chunk = response.read(min(65536, limit + 1 - size))
            if not chunk:
                break
            chunks.append(chunk)
            size += len(chunk)
            if size > limit:
                raise InstallError("Archivo descargado demasiado grande.")
        if size == 0:
            raise InstallError("Archivo descargado vacío.")
        return b"".join(chunks)


def release_asset(release, tag, name, limit):
    if any(not isinstance(asset, dict) for asset in release["assets"]):
        raise InstallError("Lista de archivos de release no válida.")
    matches = [asset for asset in release["assets"] if asset.get("name") == name]
    if len(matches) != 1:
        raise InstallError("Falta o está duplicado el archivo " + name)
    asset = matches[0]
    expected = "https://github.com/{}/releases/download/{}/{}".format(REPOSITORY, tag, name)
    if asset.get("browser_download_url") != expected:
        raise InstallError("URL de archivo de release no autorizada.")
    size = asset.get("size")
    if type(size) is not int or not 0 < size <= limit:
        raise InstallError("Tamaño de archivo de release no válido.")
    return expected, size


def verify_manifest(data, signature, tag, target, key=None):
    if key is None:
        key = PUBLIC_KEY
    if not 0 < len(data) <= MAX_MANIFEST or not 0 < len(signature) <= MAX_SIGNATURE:
        raise InstallError("Manifiesto o firma fuera de los límites.")
    with tempfile.TemporaryDirectory(prefix="core-menu-verify-") as directory:
        signers = Path(directory, "signers")
        signature_path = Path(directory, "signature")
        signers.write_text("core_menu " + key + "\n", encoding="utf-8")
        signature_path.write_bytes(signature)
        try:
            result = subprocess.run(
                ["ssh-keygen", "-Y", "verify", "-f", str(signers), "-I", "core_menu",
                 "-n", "core_menu-release", "-s", str(signature_path)],
                input=data, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=15,
            )
        except FileNotFoundError as error:
            raise InstallError("Falta ssh-keygen de OpenSSH.") from error
        if result.returncode:
            raise InstallError("Firma no válida o ssh-keygen sin soporte para -Y verify.")
    manifest = json.loads(data)
    expected_keys = {"schema", "version", "target", "asset", "size", "sha256"}
    if not isinstance(manifest, dict) or set(manifest) != expected_keys:
        raise InstallError("Campos del manifiesto no válidos.")
    expected_asset = "core_menu-{}-{}".format(tag, target)
    size = manifest["size"]
    digest = manifest["sha256"]
    if (type(manifest["schema"]) is not int or manifest["schema"] != 1
            or manifest["version"] != tag[1:] or manifest["target"] != target
            or manifest["asset"] != expected_asset
            or type(size) is not int or not 0 < size <= MAX_BINARY
            or not isinstance(digest, str) or not HASH.fullmatch(digest)):
        raise InstallError("Manifiesto incompatible con el release.")
    version(tag)
    return manifest


def verify_binary(data, manifest):
    if len(data) != manifest["size"] or hashlib.sha256(data).hexdigest() != manifest["sha256"]:
        raise InstallError("El tamaño o SHA-256 del binario no coincide.")


def fetch_release(target):
    deadline = time.monotonic() + 120
    url = "https://api.github.com/repos/{}/releases/latest".format(REPOSITORY)
    release = json.loads(download(url, 512 * 1024, deadline))
    if not isinstance(release, dict) or release.get("draft") is not False or release.get("prerelease") is not False:
        raise InstallError("El release latest no es estable.")
    tag = release["tag_name"]
    version(tag)
    if not isinstance(release.get("assets"), list):
        raise InstallError("Lista de archivos de release no válida.")
    manifest_url, manifest_size = release_asset(release, tag, "update-manifest-{}.json".format(target), MAX_MANIFEST)
    signature_url, signature_size = release_asset(release, tag, "update-manifest-{}.sig".format(target), MAX_SIGNATURE)
    data = download(manifest_url, MAX_MANIFEST, deadline)
    signature = download(signature_url, MAX_SIGNATURE, deadline)
    if len(data) != manifest_size or len(signature) != signature_size:
        raise InstallError("Descarga de manifiesto o firma incompleta.")
    manifest = verify_manifest(data, signature, tag, target)
    binary_url, binary_size = release_asset(release, tag, manifest["asset"], MAX_BINARY)
    if binary_size != manifest["size"]:
        raise InstallError("Tamaño del binario incompatible con el manifiesto.")
    binary = download(binary_url, manifest["size"], deadline)
    verify_binary(binary, manifest)
    return tag, data, signature, binary


def confirm(message):
    try:
        return input(message + " [s/N] ").strip().lower() in ("s", "si", "sí")
    except EOFError as error:
        raise InstallError("Se necesita una terminal para confirmar la instalación.") from error


def package_managed(path):
    for tool, option in (("dpkg-query", "-S"), ("rpm", "-qf"), ("pacman", "-Qo")):
        executable = shutil.which(tool)
        if executable:
            result = subprocess.run([executable, option, str(path)], stdout=subprocess.DEVNULL,
                                    stderr=subprocess.DEVNULL, timeout=5)
            if result.returncode == 0:
                return True
            if result.returncode != 1:
                raise InstallError("No se pudo consultar el gestor de paquetes " + tool)
    return False


def fingerprint(path, owner):
    try:
        info = path.lstat()
    except FileNotFoundError:
        return None
    if not stat.S_ISREG(info.st_mode) or info.st_uid != owner or info.st_mode & 0o6000:
        raise InstallError("El destino no es un archivo regular seguro del propietario esperado.")
    if package_managed(path):
        raise InstallError("El destino pertenece a un paquete; usa su gestor de paquetes.")
    flags = os.O_RDONLY | os.O_NOFOLLOW
    with os.fdopen(os.open(path, flags), "rb") as source:
        opened = os.fstat(source.fileno())
        if (opened.st_dev, opened.st_ino) != (info.st_dev, info.st_ino):
            raise InstallError("El destino cambió durante la comprobación.")
        digest = hashlib.sha256()
        for chunk in iter(lambda: source.read(65536), b""):
            digest.update(chunk)
    return [info.st_dev, info.st_ino, digest.hexdigest()]


def system_directory(path):
    if path != Path("/usr/local/bin/core_menu"):
        raise InstallError("Destino de sistema no permitido.")
    for part in (path.parent, *path.parent.parents):
        info = part.lstat()
        if not stat.S_ISDIR(info.st_mode) or info.st_uid != 0 or info.st_mode & 0o022:
            raise InstallError("Directorio de sistema no seguro: " + str(part))


def install(path, expected, binary, owner):
    lock_path = path.parent / ".core_menu.update.lock"
    fd = os.open(lock_path, os.O_CREAT | os.O_RDWR | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, "r+b") as lock:
        info = os.fstat(lock.fileno())
        if not stat.S_ISREG(info.st_mode) or info.st_uid != owner or info.st_mode & 0o077:
            raise InstallError("Archivo de bloqueo no seguro.")
        fcntl.flock(lock.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
        if fingerprint(path, owner) != expected:
            raise InstallError("El destino cambió tras la confirmación.")
        directory_fd = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY)
        try:
            with tempfile.TemporaryDirectory(prefix=".core_menu-install-", dir=path.parent) as directory:
                new = Path(directory, "new")
                with new.open("xb") as file:
                    file.write(binary)
                    file.flush()
                    os.fchmod(file.fileno(), 0o755)
                    os.fsync(file.fileno())
                backup = None
                if expected is not None:
                    recovery = Path(tempfile.mkdtemp(prefix=".core_menu-recovery-", dir=path.parent))
                    backup = recovery / "previous"
                    replaced = False
                    try:
                        os.link(path, backup, follow_symlinks=False)
                        if fingerprint(path, owner) != expected or fingerprint(backup, owner) != expected:
                            raise InstallError("El destino cambió durante la instalación.")
                        recovery_fd = os.open(recovery, os.O_RDONLY | os.O_DIRECTORY)
                        try:
                            os.fsync(recovery_fd)
                        finally:
                            os.close(recovery_fd)
                        os.fsync(directory_fd)
                        os.replace(new, path)
                        replaced = True
                    finally:
                        if not replaced:
                            backup.unlink(missing_ok=True)
                            recovery.rmdir()
                else:
                    os.link(new, path)
                try:
                    os.fsync(directory_fd)
                except OSError as error:
                    try:
                        if backup:
                            os.replace(backup, path)
                        else:
                            path.unlink()
                        os.fsync(directory_fd)
                    except OSError as rollback_error:
                        raise InstallError(
                            "No se pudo sincronizar ni restaurar el destino; copia de recuperación: "
                            + str(backup) + "; " + str(rollback_error)
                        ) from error
                    raise InstallError("No se pudo sincronizar la instalación; se restauró el destino.") from error
                return backup
        finally:
            os.close(directory_fd)


def installed_version(path):
    try:
        result = subprocess.run([str(path), "--version"], stdout=subprocess.PIPE,
                                stderr=subprocess.DEVNULL, timeout=5, text=True)
        match = re.fullmatch(r"core_menu (.+)\s*", result.stdout)
        if result.returncode == 0 and match:
            return version("v" + match.group(1).strip())
    except (OSError, subprocess.TimeoutExpired, InstallError):
        pass
    return None


def helper(directory, tag, target, expected_json):
    if os.geteuid() != 0:
        raise InstallError("La instalación de sistema requiere privilegios.")
    os.environ["PATH"] = "/usr/sbin:/usr/bin:/sbin:/bin"
    if target not in TARGETS.values() or TARGETS.get(platform.machine()) != target:
        raise InstallError("Arquitectura de instalación incompatible.")
    version(tag)
    path = Path("/usr/local/bin/core_menu")
    system_directory(path)
    directory = Path(directory)
    data = read_staged(directory / "manifest", MAX_MANIFEST)
    signature = read_staged(directory / "signature", MAX_SIGNATURE)
    binary = read_staged(directory / "binary", MAX_BINARY)
    manifest = verify_manifest(data, signature, tag, target)
    verify_binary(binary, manifest)
    expected = json.loads(expected_json)
    backup = install(path, expected, binary, 0)
    print("Instalado en", path)
    if backup:
        print("Copia de recuperación:", backup)


def read_staged(path, limit):
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    with os.fdopen(fd, "rb") as source:
        info = os.fstat(source.fileno())
        if not stat.S_ISREG(info.st_mode) or not 0 < info.st_size <= limit:
            raise InstallError("Archivo temporal no válido o demasiado grande.")
        data = source.read(limit + 1)
        if len(data) != info.st_size:
            raise InstallError("Archivo temporal modificado durante la lectura.")
        return data


def helper_source():
    path = Path(__file__).absolute()
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    with os.fdopen(fd, "rb") as source:
        info = os.fstat(source.fileno())
        if (not stat.S_ISREG(info.st_mode) or info.st_uid not in (0, os.geteuid())
                or info.st_mode & 0o022 or not 0 < info.st_size <= 128 * 1024):
            raise InstallError("El instalador no es un archivo seguro para sudo; comprueba el propietario y ejecuta chmod go-w install_release.py.")
        data = source.read(128 * 1024 + 1)
        if len(data) != info.st_size:
            raise InstallError("El instalador cambió durante la lectura.")
        return data.decode("utf-8")


def main():
    if len(sys.argv) == 6 and sys.argv[1] == "--system-helper":
        helper(*sys.argv[2:])
        return
    if len(sys.argv) != 1:
        raise InstallError("Uso: sh install.sh")
    if os.geteuid() == 0:
        raise InstallError("Ejecuta el instalador sin sudo; se solicitará cuando sea necesario.")
    if platform.system() != "Linux" or platform.machine() not in TARGETS:
        raise InstallError("Sistema o arquitectura no compatible: " + platform.machine())
    target = TARGETS[platform.machine()]
    print("Arquitectura:", target)
    choice = input("Destino: [1] usuario (~/.local/bin) o [2] sistema (/usr/local/bin): ").strip()
    if choice == "1":
        path = Path.home() / ".local/bin/core_menu"
        path.parent.mkdir(mode=0o755, parents=True, exist_ok=True)
        owner = os.geteuid()
    elif choice == "2":
        path = Path("/usr/local/bin/core_menu")
        system_directory(path)
        owner = 0
    else:
        raise InstallError("Destino no válido; instalación cancelada.")
    expected = fingerprint(path, owner)
    tag, data, signature, binary = fetch_release(target)
    if expected is not None:
        print("Ya existe", path)
        if not confirm("¿Reemplazar el ejecutable existente?"):
            raise InstallError("Instalación cancelada.")
        current = installed_version(path)
        if current is None:
            print("No se pudo identificar la versión instalada.")
            if not confirm("¿Reemplazarla igualmente?"):
                raise InstallError("Instalación cancelada.")
        elif current > version(tag):
            print("Advertencia: {} es más antigua que la versión instalada.".format(tag))
            if not confirm("¿Confirmar el retroceso de versión?"):
                raise InstallError("Instalación cancelada.")
    if choice == "1":
        backup = install(path, expected, binary, owner)
        print("Instalado en", path)
        if backup:
            print("Copia de recuperación:", backup)
        print("Comprueba que ~/.local/bin está en PATH.")
    else:
        if not shutil.which("sudo"):
            raise InstallError("Falta sudo para la instalación de sistema.")
        if not Path("/usr/bin/python3").is_file():
            raise InstallError("La instalación de sistema requiere /usr/bin/python3.")
        source = helper_source()
        with tempfile.TemporaryDirectory(prefix="core-menu-download-") as directory:
            for name, content in (("manifest", data), ("signature", signature), ("binary", binary)):
                Path(directory, name).write_bytes(content)
            result = subprocess.run(["sudo", "--", "/usr/bin/python3", "-I", "-c", source,
                                     "--system-helper", directory, tag, target, json.dumps(expected)])
            if result.returncode:
                raise InstallError("La instalación de sistema falló.")


if __name__ == "__main__":
    try:
        main()
    except (InstallError, OSError, EOFError, ValueError, KeyError, TypeError, subprocess.SubprocessError,
            urllib.error.URLError) as error:
        print("Error:", error, file=sys.stderr)
        sys.exit(1)
