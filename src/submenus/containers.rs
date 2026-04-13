//! # Submenú: Virtualización
//!
//! Agrupa comandos para gestionar e inspeccionar:
//! - Módulo KVM del kernel (AMD, Intel, ARM u otro)
//! - Motor de contenerización activo (Docker, Podman, Nerdctl…)
//! - Arrancar motores de contenedores
//! - Contenedores, imágenes, volúmenes, redes y estadísticas
//!
//! ## Añadir un nuevo comando
//!
//! Para comandos directos sin parámetros:
//! ```rust
//! MenuItem::new(
//!     "Nombre",                      // ≤ 32 chars
//!     "Descripción",                 // ≤ 42 chars
//!     || engine_cmd("subcomando"),   // llamada con cualquier argumento válido
//! ),
//! ```
//!
//! Para comandos que aceptan parámetros:
//! ```rust
//! MenuItem::with_input(
//!     "Inspeccionar contenedor",
//!     "docker inspect <nombre/id>",
//!     "Nombre o ID del contenedor:",
//!     "",
//!     |id| engine_cmd(&format!("inspect {id}")),
//! ),
//! ```
//!
//! ## Añadir soporte para un nuevo motor de contenerización
//!
//! Añade el nombre del binario al array `ENGINES` dentro de [`engine_cmd`].
//! El motor detectado se usa automáticamente en todos los comandos del submenú.

use super::{run_shell, MenuItem, SubMenu};

// ─────────────────────────────────────────────────────────────────────────────
// REGISTRO DEL SUBMENÚ
// ─────────────────────────────────────────────────────────────────────────────

/// Construye y devuelve el submenú de virtualización.
pub fn create() -> SubMenu {
    SubMenu::new(
        "Virtualización",
        "KVM, Docker/Podman, contenedores y redes",
        vec![
            // ── KVM ───────────────────────────────────────────────────────
            MenuItem::new(
                "Estado módulo KVM",
                "kvm_amd / kvm_intel / kvm según la CPU",
                kvm_status,
            ),
            MenuItem::new(
                "Activar / desactivar KVM",
                "Carga o descarga el módulo KVM activo (sudo)",
                kvm_toggle,
            ),
            // ── Motor de contenerización ──────────────────────────────────
            MenuItem::new(
                "Motor detectado",
                "Docker, Podman, Nerdctl instalados en el PATH",
                detect_engines,
            ),
            MenuItem::new(
                "Estado del servicio",
                "Estado systemd del motor de contenedores",
                engine_service_status,
            ),
            // ── Contenedores ──────────────────────────────────────────────
            MenuItem::new(
                "Contenedores activos",
                "Contenedores en ejecución",
                || engine_cmd("ps"),
            ),
            MenuItem::new(
                "Todos los contenedores",
                "Contenedores activos y parados (ps -a)",
                || engine_cmd("ps -a"),
            ),
            MenuItem::new(
                "Número de contenedores",
                "Conteo de contenedores activos y totales",
                container_count,
            ),
            // ── Imágenes ──────────────────────────────────────────────────
            MenuItem::new(
                "Imágenes",
                "Imágenes locales disponibles",
                || engine_cmd("images"),
            ),
            MenuItem::new(
                "Imágenes colgantes",
                "Imágenes sin etiqueta (dangling)",
                || engine_cmd("images -f dangling=true"),
            ),
            // ── Volúmenes ─────────────────────────────────────────────────
            MenuItem::new(
                "Volúmenes",
                "Volúmenes gestionados por el motor",
                || engine_cmd("volume ls"),
            ),
            MenuItem::new(
                "Volúmenes no usados",
                "Volúmenes sin contenedor asociado",
                || engine_cmd("volume ls -f dangling=true"),
            ),
            // ── Redes ─────────────────────────────────────────────────────
            MenuItem::new(
                "Redes virtuales",
                "Redes del motor de contenedores",
                || engine_cmd("network ls"),
            ),
            // ── Arrancar motor ────────────────────────────────────────────
            MenuItem::new(
                "Arrancar Docker",
                "Inicia el servicio Docker (sudo systemctl)",
                || start_engine("docker"),
            ),
            MenuItem::new(
                "Arrancar Podman",
                "Inicia el socket/servicio Podman (sudo)",
                || start_engine("podman"),
            ),
            MenuItem::new(
                "Arrancar Nerdctl (containerd)",
                "Inicia el servicio containerd (sudo systemctl)",
                || start_engine("nerdctl"),
            ),
            MenuItem::new(
                "Arrancar Lima",
                "Inicia la VM Lima por defecto (limactl start)",
                || start_engine("lima"),
            ),
            // ── Recursos y espacio ────────────────────────────────────────
            MenuItem::new(
                "Estadísticas de recursos",
                "CPU y RAM de contenedores activos",
                || engine_cmd("stats --no-stream"),
            ),
            MenuItem::new(
                "Uso de espacio",
                "Espacio: imágenes, contenedores, volúmenes",
                || engine_cmd("system df"),
            ),
            MenuItem::new(
                "Información del sistema",
                "Versión y configuración del motor",
                || engine_cmd("info"),
            ),
        ],
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// KVM
// ─────────────────────────────────────────────────────────────────────────────

/// Determina el módulo KVM apropiado para la CPU del sistema.
///
/// Lógica de detección:
/// 1. Lee `vendor_id` de `/proc/cpuinfo` → `kvm_amd` | `kvm_intel`
/// 2. Si la arquitectura es ARM/AArch64 → `kvm` (módulo genérico)
/// 3. Como fallback: busca el módulo `kvm_*` ya cargado en `lsmod`
/// 4. Si nada se detecta → `kvm`
fn cpu_kvm_module() -> String {
    let vendor = run_shell("grep -m1 'vendor_id' /proc/cpuinfo 2>/dev/null");
    if vendor.contains("AuthenticAMD") {
        return "kvm_amd".to_string();
    }
    if vendor.contains("GenuineIntel") {
        return "kvm_intel".to_string();
    }
    // ARM / AArch64
    let arch = run_shell("uname -m 2>/dev/null");
    let arch = arch.trim();
    if arch.starts_with("aarch64") || arch.starts_with("armv") {
        return "kvm".to_string();
    }
    // Fallback: buscar módulo ya cargado
    let loaded =
        run_shell("lsmod 2>/dev/null | awk '/^kvm_/{print $1; exit}'");
    let loaded = loaded.trim().to_string();
    if !loaded.is_empty() {
        return loaded;
    }
    "kvm".to_string()
}

/// Muestra el estado completo del módulo KVM para la CPU detectada.
fn kvm_status() -> String {
    let module = cpu_kvm_module();
    let vendor = run_shell("grep -m1 'vendor_id' /proc/cpuinfo 2>/dev/null");
    let cpu_name = run_shell(
        "grep -m1 'model name' /proc/cpuinfo 2>/dev/null \
         | sed 's/model name.*: //'",
    );
    let cpu_type = if vendor.contains("AuthenticAMD") {
        "AMD"
    } else if vendor.contains("GenuineIntel") {
        "Intel"
    } else {
        "Otro / ARM"
    };

    let virt_cores =
        run_shell("grep -Ec '(vmx|svm)' /proc/cpuinfo 2>/dev/null");
    let lsmod_out = run_shell("lsmod 2>/dev/null | grep -E '^kvm'");
    let dev_kvm =
        run_shell("ls -la /dev/kvm 2>/dev/null || echo '(no disponible)'");
    let modinfo = run_shell(&format!(
        "modinfo {module} 2>/dev/null \
         | grep -E '^(filename|version|description|author):'"
    ));

    format!(
        "Procesador  : {tipo} — {modelo}\n\
         Módulo KVM  : {module}\n\
         Núcleos VT  : {vt} (núcleos con vmx/svm)\n\
         \n\
         ── Módulos KVM cargados (lsmod) ─────────────────────\n\
         {cargados}\n\
         \n\
         ── /dev/kvm ─────────────────────────────────────────\n\
         {dev}\n\
         \n\
         ── Información del módulo ───────────────────────────\n\
         {info}",
        tipo     = cpu_type,
        modelo   = cpu_name.trim(),
        vt       = virt_cores.trim(),
        cargados = if lsmod_out.trim().is_empty() { "(ninguno cargado)" } else { lsmod_out.trim() },
        dev      = dev_kvm.trim(),
        info     = if modinfo.trim().is_empty() { "(no disponible)" } else { modinfo.trim() },
    )
}

/// Activa el módulo KVM si está descargado, o lo desactiva si está cargado.
///
/// Requiere permisos `sudo`. Si el usuario no tiene acceso, la salida
/// del comando mostrará el error de `sudo` de forma informativa.
fn kvm_toggle() -> String {
    const MODULE: &str = "kvm_amd";
    // Comprobar si está cargado: `lsmod | grep -c` devuelve 0 o N
    let count_str =
        run_shell(&format!("lsmod 2>/dev/null | grep -c '^{MODULE} '"));
    let is_loaded = count_str.trim().parse::<u32>().unwrap_or(0) > 0;

    if is_loaded {
        let out = run_shell(&format!("sudo modprobe -r {MODULE} 2>&1"));
        format!(
            "Acción      : DESACTIVAR {MODULE}\n\
             Comando     : sudo modprobe -r {MODULE}\n\
             \n\
             {result}",
            result = if out.trim().is_empty() {
                format!("✓ Módulo {MODULE} descargado correctamente.")
            } else {
                out
            }
        )
    } else {
        let out = run_shell(&format!("sudo modprobe {MODULE} 2>&1"));
        format!(
            "Acción      : ACTIVAR {MODULE}\n\
             Comando     : sudo modprobe {MODULE}\n\
             \n\
             {result}",
            result = if out.trim().is_empty() {
                format!("✓ Módulo {MODULE} cargado correctamente.")
            } else {
                out
            }
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// MOTOR DE CONTENERIZACIÓN
// ─────────────────────────────────────────────────────────────────────────────

/// Motores de contenerización soportados, en orden de preferencia.
///
/// Para añadir soporte a un nuevo motor, añade su binario a este array.
/// La detección es automática: se usa el primero que esté disponible en el PATH.
const ENGINES: &[&str] = &["docker", "podman", "nerdctl", "lima"];

/// Devuelve el binario del primer motor disponible en el PATH, o `None`.
fn find_engine() -> Option<&'static str> {
    ENGINES.iter().find(|&&e| {
        !run_shell(&format!("command -v {e} 2>/dev/null")).trim().is_empty()
    }).copied()
}

/// Ejecuta `<motor> <args>` con el motor detectado y devuelve la salida.
///
/// Si no se detecta ningún motor, devuelve un mensaje informativo con
/// instrucciones de instalación.
fn engine_cmd(args: &str) -> String {
    match find_engine() {
        Some(engine) => run_shell(&format!("{engine} {args} 2>&1")),
        None => format!(
            "No se detectó ningún motor de contenerización en el PATH.\n\
             \n\
             Motores soportados: {}\n\
             \n\
             Instala uno y asegúrate de que su binario esté en el PATH:\n\
             · Docker :  https://docs.docker.com/engine/install/\n\
             · Podman :  sudo apt install podman\n\
             · Nerdctl:  https://github.com/containerd/nerdctl",
            ENGINES.join(", ")
        ),
    }
}

/// Lista todos los motores instalados con su versión.
fn detect_engines() -> String {
    let active = find_engine();
    let mut out = format!(
        "Motor activo : {}\n\
         Preferencia  : {}\n\
         \n\
         ── Estado de cada motor ─────────────────────────────\n",
        active.unwrap_or("(ninguno detectado)"),
        ENGINES.join(" → "),
    );
    for &engine in ENGINES {
        let path = run_shell(&format!("command -v {engine} 2>/dev/null"));
        if path.trim().is_empty() {
            out.push_str(&format!("  {engine:<12} ✗  no instalado\n"));
        } else {
            let ver = run_shell(&format!("{engine} --version 2>/dev/null"));
            let first = ver.lines().next().unwrap_or("").trim();
            let marker = if active == Some(engine) { "★ " } else { "  " };
            out.push_str(&format!(
                "{marker}{engine:<12} ✓  {ver}\n                  en {path}\n",
                ver  = first,
                path = path.trim(),
            ));
        }
    }
    out
}

/// Arranca el servicio del motor indicado.
///
/// Cada motor tiene su propio mecanismo de arranque:
/// - `docker`  → `sudo systemctl start docker`
/// - `podman`  → `sudo systemctl start podman.socket`
/// - `nerdctl` → `sudo systemctl start containerd`
/// - `lima`    → `limactl start` (sin sudo; gestiona su propia VM)
///
/// Si el binario no está instalado, devuelve un aviso sin ejecutar nada.
fn start_engine(engine: &str) -> String {
    let path = run_shell(&format!("command -v {engine} 2>/dev/null"));
    if path.trim().is_empty() {
        return format!(
            "Motor       : {engine}\n\
             Estado      : no instalado en el PATH\n\
             \n\
             Instala el motor antes de intentar arrancarlo."
        );
    }

    let (cmd, service) = match engine {
        "docker"  => ("sudo systemctl start docker 2>&1",        "docker"),
        "podman"  => ("sudo systemctl start podman.socket 2>&1", "podman.socket"),
        "nerdctl" => ("sudo systemctl start containerd 2>&1",    "containerd"),
        "lima"    => ("limactl start 2>&1",                       "lima"),
        other     => return format!("Motor '{other}' no reconocido."),
    };

    let out = run_shell(cmd);
    let active = run_shell(&format!("systemctl is-active {service} 2>/dev/null"));
    let status = active.trim().to_string();

    format!(
        "Motor       : {engine}\n\
         Comando     : {cmd}\n\
         Estado      : {status}\n\
         \n\
         {result}",
        result = if out.trim().is_empty() || out.contains("(Sin salida)") {
            format!("✓ Servicio {service} iniciado correctamente.")
        } else {
            out.trim().to_string()
        },
    )
}

/// Estado systemd de los servicios de contenerización conocidos.
fn engine_service_status() -> String {
    let services = ["docker", "podman", "containerd", "crio"];
    let mut out = String::new();
    for svc in &services {
        let active =
            run_shell(&format!("systemctl is-active {svc} 2>/dev/null"));
        if !active.trim().is_empty() && active.trim() != "inactive" {
            out.push_str(&format!("══ {svc} ({}) ══\n", active.trim()));
            out.push_str(&run_shell(&format!(
                "systemctl status {svc} --no-pager -l -n 10 2>/dev/null"
            )));
            out.push_str("\n\n");
        }
    }
    if out.is_empty() {
        "No se encontraron servicios de contenerización activos gestionados por systemd.\n\
         \n\
         Servicios comprobados: docker, podman, containerd, crio\n\
         Verifica con: systemctl status docker"
            .to_string()
    } else {
        out
    }
}

/// Muestra el conteo de contenedores activos y totales.
fn container_count() -> String {
    let running = run_shell("docker ps -q 2>/dev/null || podman ps -q 2>/dev/null");
    let all     = run_shell("docker ps -aq 2>/dev/null || podman ps -aq 2>/dev/null");
    let images  = run_shell("docker images -q 2>/dev/null || podman images -q 2>/dev/null");
    let volumes = run_shell("docker volume ls -q 2>/dev/null || podman volume ls -q 2>/dev/null");
    let networks = run_shell("docker network ls -q 2>/dev/null || podman network ls -q 2>/dev/null");

    let count_lines = |s: &str| s.lines().filter(|l| !l.trim().is_empty()).count();

    format!(
        "Motor activo   : {engine}\n\
         \n\
         Contenedores en ejecución : {running}\n\
         Contenedores totales      : {all}\n\
         Imágenes locales          : {images}\n\
         Volúmenes                 : {volumes}\n\
         Redes virtuales           : {networks}",
        engine   = find_engine().unwrap_or("(ninguno detectado)"),
        running  = count_lines(&running),
        all      = count_lines(&all),
        images   = count_lines(&images),
        volumes  = count_lines(&volumes),
        networks = count_lines(&networks).saturating_sub(1), // restar cabecera
    )
}
