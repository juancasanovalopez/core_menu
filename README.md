# core_menu — Documentación

Gestor de sistema Linux con interfaz de menú interactiva (TUI) ejecutable en cualquier terminal compatible.

---

## Índice

| Documento | Descripción |
|-----------|-------------|
| [instalacion.md](doc/instalacion.md) | Requisitos, compilación e instalación del binario |
| [uso.md](doc/uso.md) | Navegación por los menús, atajos de teclado y submenús disponibles |
| [extension.md](doc/extension.md) | Cómo añadir nuevos submenús y comandos al sistema |

---

## Vista rápida

```
┌─────────────────────────────────────────────────────────┐
│               CORE MENU  —  Gestor de Sistema Linux     │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  ▶ Red               Estado de red y firewall           │
│    Usuarios          Usuarios y sesiones del sistema    │
│    Almacenamiento    Discos, particiones y uso espacio  │
│    Memoria           Estado y uso de la memoria RAM     │
│    CPU               Procesador, núcleos y temperatura  │
│    GPU               Tarjeta gráfica, VRAM y uso de GPU │
│    Virtualización    KVM, Docker/Podman, contenedores y redes │
│    ──────────────────────────────────────────────────   │
│    Salir                                                │
│                                                         │
├─────────────────────────────────────────────────────────┤
│    ↑↓/jk: Navegar   Enter: Seleccionar   q: Salir       │
└─────────────────────────────────────────────────────────┘
```

---

## Guía de uso

### Arranque

```bash
./core_menu          # binario en el directorio actual
# o si está en el PATH:
core_menu
```

La aplicación ocupa la pantalla completa en modo alternativo de terminal y la restaura al salir.

### Navegación

| Tecla | Acción |
|-------|--------|
| `↑` / `k` | Mover cursor arriba |
| `↓` / `j` | Mover cursor abajo |
| `Enter` | Seleccionar opción / ejecutar comando |
| `Esc` / `q` | Volver al nivel anterior |
| `Ctrl+C` | Salir inmediatamente |

La navegación funciona igual en el menú principal, dentro de los submenús y en la pantalla de salida de un comando.

### Pantallas

**Menú principal** — lista de submenús disponibles. Selecciona uno con `Enter`.

**Submenú** — lista de comandos del área elegida. Cada entrada muestra nombre y descripción. Pulsa `Enter` para ejecutar.

**Salida de comando** — texto devuelto por el comando. Desplázate con `↑`/`↓` o `j`/`k`. Vuelve con `Esc`.

**Entrada de parámetro** — algunos comandos solicitan un valor antes de ejecutarse (p.ej. índice de módem, ID de contenedor). Escribe el valor y confirma con `Enter`. Se muestra un valor predeterminado que puedes conservar o sobreescribir.

**Gráfico de barras** — comandos de almacenamiento y memoria pueden mostrar una pantalla de barras con código de color:
- Verde: uso < 70 %
- Amarillo: uso entre 70 % y 90 %
- Rojo: uso ≥ 90 %

---

## Submenús disponibles

### Red
Inspección del estado de la red y el firewall.

| Comando | Descripción |
|---------|-------------|
| Interfaces de red | Interfaces y sus IPs (`ip addr`) |
| Puertos en escucha | Puertos TCP/UDP activos (`ss -tulnp`) |
| Conexiones establecidas | Conexiones TCP activas (`ss -tnp`) |
| Tabla de rutas | Rutas de red activas (`ip route show`) |
| Firewall UFW | Reglas activas de UFW |
| Reglas iptables | Tabla filter de iptables (requiere root) |
| Configuración DNS | Servidores DNS configurados |
| Estadísticas de interfaces | Contadores de paquetes/bytes |
| ARP / vecinos | Tabla ARP e IPv6 ND |
| Módems detectados | Lista módems gestionados por ModemManager |
| Estado del módem | Info detallada del módem por índice *(entrada de parámetro)* |

### Usuarios
Gestión de usuarios, grupos y sesiones.

| Comando | Descripción |
|---------|-------------|
| Usuarios conectados | Sesiones activas (`who -a`) |
| Actividad actual | Usuarios, tiempo y procesos activos (`w`) |
| Últimos accesos | Historial de login (`last -n 30`) |
| Intentos fallidos | Accesos fallidos (`lastb`, requiere root) |
| Usuarios con shell de login | Cuentas que pueden iniciar sesión |
| Todos los usuarios | Listado completo de `/etc/passwd` |
| Grupos del sistema | Listado de `/etc/group` |
| Identidad actual | UID, GID y grupos del proceso actual |
| Privilegios sudo | Comandos permitidos al usuario (`sudo -l`) |

### Almacenamiento
Discos, particiones, sistemas de archivos y salud de dispositivos.

| Comando | Descripción |
|---------|-------------|
| Uso de disco | Espacio libre/usado por sistema de archivos (`df -h`) |
| Uso de inodos | Disponibilidad de inodos (`df -i`) |
| Dispositivos de bloque | Discos y particiones con detalles (`lsblk`) |
| Info de discos físicos | Modelo, tamaño y tipo de transporte |
| Puntos de montaje | Sistemas de archivos montados |
| Directorios más grandes | Top 20 directorios por tamaño |
| Uso del directorio actual | Tamaño de cada entrada en `$PWD` |
| Salud SMART de discos | Estado SMART (requiere `smartmontools`) |
| Gráfico de uso | *(pantalla de barras)* Uso por sistema de archivos |

### Memoria
Estado y uso de la RAM, swap y caché del kernel.

| Comando | Descripción |
|---------|-------------|
| Resumen de memoria | RAM y swap libre/usada (`free -h`) |
| Resumen detallado | RAM en MB con columnas extendidas |
| Info completa del kernel | Todos los campos de `/proc/meminfo` |
| Memoria disponible | RAM total, libre y disponible |
| Estado del swap | Particiones y archivos de intercambio |
| Top procesos por RAM | 20 procesos con mayor consumo |
| Procesos con RSS > 100 MB | Procesos que usan más de 100 MB |
| Uso de caché del kernel | Buffers, caché de página y slab |
| Gráfico de memoria | *(pantalla de barras)* RAM y swap |

### CPU
Estado del procesador, rendimiento por núcleo y temperatura.

| Comando | Descripción |
|---------|-------------|
| Información del procesador | Modelo, núcleos y frecuencias (`lscpu`) |
| Carga del sistema | Promedios de carga 1, 5 y 15 min (`uptime`) |
| Procesos activos (CPU) | Top 20 procesos ordenados por uso de CPU |
| Estadísticas de CPU (vmstat) | Actividad de CPU en 5 muestras de 1 s |
| Frecuencia por núcleo | Frecuencia actual de cada núcleo (MHz) |
| Temperatura de la CPU | Sensores de temperatura (`sensors` / sysfs) |
| Caché del procesador | Tamaños de L1, L2 y L3 (`lscpu -C`) |
| Interrupciones por núcleo | Primeras 30 líneas de `/proc/interrupts` |
| Uso por núcleo (gráfico) | *(pantalla de barras)* Uso de CPU por núcleo — muestra de 250 ms |

### GPU
Estado de la tarjeta gráfica, VRAM y temperatura. Compatible con NVIDIA (`nvidia-smi`) y AMD (driver `amdgpu` vía sysfs).

| Comando | Descripción |
|---------|-------------|
| GPUs detectadas | Tarjetas gráficas instaladas (`lspci`) |
| Driver activo | Driver en uso por cada GPU |
| Estado NVIDIA (nvidia-smi) | Resumen completo de `nvidia-smi` |
| Procesos en GPU NVIDIA | Procesos que usan la GPU NVIDIA |
| Estado AMD (sysfs) | Uso, VRAM y temperatura via driver `amdgpu` |
| Temperatura GPU | Temperatura vía `sensors` o sysfs |
| Dispositivos DRM | Nodos de renderizado del kernel (`/dev/dri`) |
| Uso de GPU (gráfico) | *(pantalla de barras)* Utilización y VRAM por GPU |

### Virtualización
Gestión de KVM y motores de contenedores (Docker, Podman, Nerdctl…).

| Comando | Descripción |
|---------|-------------|
| Estado módulo KVM | kvm_amd / kvm_intel / kvm según la CPU |
| Activar / desactivar KVM | Carga o descarga el módulo KVM (sudo) |
| Motor detectado | Docker, Podman, Nerdctl instalados en el PATH |
| Estado del servicio | Estado systemd del motor de contenedores |
| Contenedores activos | Contenedores en ejecución |
| Todos los contenedores | Contenedores activos y parados |
| Número de contenedores | Conteo de contenedores activos y totales |
| Imágenes | Imágenes locales disponibles |
| Imágenes colgantes | Imágenes sin etiqueta (dangling) |
| Volúmenes | Volúmenes gestionados por el motor |
| Volúmenes no usados | Volúmenes sin contenedor asociado |
| Redes virtuales | Redes del motor de contenedores |
| Arrancar Docker | Inicia el servicio Docker (sudo systemctl) |
| Arrancar Podman | Inicia el socket/servicio Podman (sudo) |
| Arrancar Nerdctl (containerd) | Inicia el servicio containerd (sudo systemctl) |
| Arrancar Lima | Inicia la VM Lima por defecto (limactl start) |
| Estadísticas de recursos | CPU y RAM de contenedores activos |
| Uso de espacio | Espacio: imágenes, contenedores, volúmenes |
| Información del sistema | Versión y configuración del motor |

---

## Tecnología

- **Lenguaje:** Rust 2021 edition
- **TUI:** [ratatui](https://ratatui.rs) v0.26 sobre [crossterm](https://github.com/crossterm-rs/crossterm) v0.27
- **Gestión de errores:** [anyhow](https://github.com/dtolnay/anyhow) v1.0
- **Binario final:** ~963 KB (compilado en modo `--release`)
