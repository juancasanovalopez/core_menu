# Instalación y ejecución

---

## Requisitos

| Requisito | Versión mínima | Notas |
|-----------|---------------|-------|
| Rust + Cargo | 1.70 | Instalado vía [rustup](https://rustup.rs) |
| Terminal | Cualquiera | xterm, GNOME Terminal, Alacritty, tmux, etc. |
| Sistema operativo | Linux (cualquier distribución) | — |

> **¿Por qué 1.70?** Es la versión estable más antigua compatible con las dependencias declaradas (`ratatui 0.26`, `crossterm 0.27`, `anyhow 1.0`).
> El proyecto fue desarrollado y verificado con **rustc 1.94.1**.

---

## 1. Instalar Rust (si no está instalado)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Sigue las instrucciones del instalador (opción predeterminada recomendada).  
Después, activa el entorno en la sesión actual:

```bash
source "$HOME/.cargo/env"
```

Verifica la instalación:

```bash
rustc --version
cargo --version
```

---

## 2. Obtener el código fuente

### Opción A — Clonar el repositorio (si está en Git)

```bash
git clone <URL_DEL_REPOSITORIO>
cd core_menu
```

### Opción B — Desde una copia local

```bash
cd /ruta/a/core_menu
```

---

## 3. Compilar

### Modo desarrollo (más rápido de compilar, binario sin optimizar)

```bash
cargo build
```

El binario se genera en:

```
target/debug/core_menu
```

### Modo release (recomendado para uso diario, binario optimizado ~963 KB)

```bash
cargo build --release
```

El binario se genera en:

```
target/release/core_menu
```

---

## 4. Ejecutar

### Desde el directorio del proyecto

```bash
# Modo debug
./target/debug/core_menu

# Modo release (recomendado)
./target/release/core_menu
```

### Atajo con cargo

```bash
cargo run           # compila en debug y ejecuta
cargo run --release # compila en release y ejecuta
```

---

## 5. Instalación global (opcional)

Para ejecutar `core_menu` desde cualquier directorio sin especificar la ruta:

```bash
cargo install --path .
```

Esto copia el binario a `~/.cargo/bin/core_menu`.  
Asegúrate de que `~/.cargo/bin` esté en tu `PATH` (el instalador de rustup lo configura automáticamente).

Verificar que está disponible globalmente:

```bash
core_menu
```

Para desinstalarlo:

```bash
cargo uninstall core_menu
```

---

## 6. Solución de problemas

### El terminal queda en estado raro tras un error

La aplicación instala un hook de pánico que restaura el terminal automáticamente.  
Si aun así la pantalla queda en modo raw, ejecuta:

```bash
reset
```

### La terminal no muestra colores o caracteres especiales

Asegúrate de que la variable `TERM` está configurada correctamente:

```bash
echo $TERM
# Esperado: xterm-256color, screen-256color, tmux-256color...
```

Si la variable no está definida, fuerza un valor compatible antes de ejecutar:

```bash
TERM=xterm-256color ./target/release/core_menu
```

### Algunos comandos devuelven "Se requieren permisos root"

Ciertos comandos del sistema (iptables, lastb, etc.) requieren privilegios elevados.  
Ejecuta la aplicación con `sudo` solo si confías en el entorno:

```bash
sudo ./target/release/core_menu
```

### Error: `command not found: cargo`

El entorno de Rust no está activado en la sesión actual. Ejecuta:

```bash
source "$HOME/.cargo/env"
```

Y añade esa línea a tu `~/.bashrc` o `~/.zshrc` para que se active automáticamente.
