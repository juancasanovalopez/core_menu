# Guía de uso

---

## Pantallas de la aplicación

La aplicación tiene tres niveles de navegación que se transitan en orden:

```
Menú principal  →  Submenú  →  Salida del comando  →  Submenú  →  Menú principal
```

---

## Atajos de teclado

### Globales (disponibles en cualquier pantalla)

| Tecla | Acción |
|-------|--------|
| `Ctrl+C` | Salir de la aplicación inmediatamente |

### Menú principal

| Tecla | Acción |
|-------|--------|
| `↑` / `k` | Mover cursor hacia arriba |
| `↓` / `j` | Mover cursor hacia abajo |
| `Enter` | Entrar en el submenú seleccionado |
| `q` / `Q` | Salir de la aplicación |

### Dentro de un submenú

| Tecla | Acción |
|-------|--------|
| `↑` / `k` | Mover cursor hacia arriba |
| `↓` / `j` | Mover cursor hacia abajo |
| `Enter` | Ejecutar el comando seleccionado |
| `Esc` / `q` / `Retroceso` | Volver al menú principal |

### Pantalla de salida (resultado de un comando)

| Tecla | Acción |
|-------|--------|
| `↑` / `k` | Scroll hacia arriba |
| `↓` / `j` | Scroll hacia abajo |
| `Enter` / `Esc` / `q` / `Retroceso` | Volver al submenú |

---

## Submenús disponibles

### Red

Agrupa comandos de inspección de red y firewall.

| Comando | Descripción | Herramienta |
|---------|-------------|-------------|
| Interfaces de red | IPs y estado de cada interfaz | `ip addr show` |
| Puertos en escucha | Puertos TCP/UDP activos con el proceso asociado | `ss -tulnp` |
| Conexiones establecidas | Conexiones TCP activas | `ss -tnp` |
| Tabla de rutas | Rutas de red activas | `ip route show` |
| Firewall UFW | Reglas activas de UFW | `ufw status verbose` |
| Reglas iptables | Tabla filter de iptables (requiere root) | `iptables -L` |
| Configuración DNS | Servidores DNS configurados | `/etc/resolv.conf` |
| Estadísticas de interfaces | Contadores de paquetes y bytes | `ip -s link` |
| ARP / vecinos | Tabla ARP e IPv6 Neighbor Discovery | `ip neigh show` |

---

### Usuarios

Agrupa comandos para inspeccionar cuentas y sesiones del sistema.

| Comando | Descripción | Herramienta |
|---------|-------------|-------------|
| Usuarios conectados | Sesiones activas en el sistema | `who -a` |
| Actividad actual | Usuarios, tiempo activo y carga | `w` |
| Últimos accesos | Historial de inicio de sesión (últimas 30) | `last -n 30` |
| Intentos fallidos | Accesos fallidos (requiere root) | `lastb -n 20` |
| Usuarios con shell de login | Cuentas que pueden iniciar sesión | `/etc/passwd` filtrado |
| Todos los usuarios | Listado completo del sistema | `/etc/passwd` |
| Grupos del sistema | Listado de grupos | `/etc/group` |
| Identidad actual | UID, GID y grupos del proceso actual | `id` + `groups` |
| Privilegios sudo | Comandos permitidos al usuario actual | `sudo -l` |

---

### Almacenamiento

Agrupa comandos para inspeccionar discos, particiones y uso de espacio.

| Comando | Descripción | Herramienta |
|---------|-------------|-------------|
| Uso de disco | Espacio libre y usado por sistema de archivos | `df -h` |
| Uso de inodos | Disponibilidad de inodos | `df -i` |
| Dispositivos de bloque | Discos y particiones con detalles | `lsblk` |
| Info de discos físicos | Modelo, tamaño y tipo de transporte | `lsblk -d` |
| Puntos de montaje | Sistemas de archivos montados | `mount` |
| Directorios más grandes | Top 20 directorios por tamaño | `du` |
| Uso del directorio actual | Tamaño de cada entrada en el directorio actual | `du -sh *` |
| Salud SMART de discos | Estado SMART (requiere smartmontools) | `smartctl` |
| Cuota de usuario | Uso de cuota del usuario actual | `quota -s` |

---

## Comportamiento de los comandos

- Los comandos se ejecutan con `sh -c "..."` al pulsar `Enter`.
- La salida se muestra en pantalla completa con soporte de **scroll** (`↑↓`).
- Se eliminan automáticamente los códigos de escape ANSI para mostrar texto limpio.
- Si un comando produce salida en `stderr` (p.ej. requiere root), se muestra igualmente con el prefijo `[stderr]`.
- Si el comando no produce ninguna salida, se muestra el mensaje `(Sin salida)`.

---

## Ejemplo de sesión completa

```
1. Ejecuta la aplicación
   $ ./target/release/core_menu

2. Usa ↓ para seleccionar "Red" y pulsa Enter

3. Usa ↓ para seleccionar "Puertos en escucha" y pulsa Enter
   → El sistema ejecuta: ss -tulnp
   → La salida se muestra en pantalla
   → Usa ↑↓ para hacer scroll si la salida es larga

4. Pulsa Esc para volver al submenú "Red"

5. Pulsa Esc para volver al menú principal

6. Selecciona "Salir" o pulsa q para cerrar
```
