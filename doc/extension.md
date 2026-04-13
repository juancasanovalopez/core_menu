# Guía de extensión

Cómo ampliar `core_menu` añadiendo nuevos submenús y comandos sin modificar la lógica central.

---

## Arquitectura del sistema de menús

```
src/submenus/
├── mod.rs          ← tipos centrales + registro get_submenus() + run_shell()
├── network.rs      ← submenú "Red"
├── users.rs        ← submenú "Usuarios"
├── storage.rs      ← submenú "Almacenamiento"
└── <nuevo>.rs      ← ← ← aquí añades el tuyo
```

Cada submenú es un módulo Rust independiente que exporta una única función pública `create() -> SubMenu`.  
El registro central en `mod.rs` es el único punto de integración.

---

## Caso 1 — Añadir un comando a un submenú existente

Abre el archivo del submenú correspondiente (p.ej. [src/submenus/network.rs](../src/submenus/network.rs)) y añade un `MenuItem::new(...)` dentro del `vec![]` de la función `create()`:

```rust
MenuItem::new(
    "Nombre visible",          // ≤ 32 caracteres recomendado
    "Descripción breve",       // ≤ 42 caracteres recomendado
    || run_shell("comando"),   // función que ejecuta el comando
),
```

### Ejemplo real — añadir "ping a gateway" en network.rs

```rust
MenuItem::new(
    "Ping al gateway",
    "Comprueba conectividad con la puerta de enlace",
    || run_shell(
        "gw=$(ip route | awk '/default/{print $3; exit}'); \
         ping -c 4 \"$gw\" 2>/dev/null \
         || echo 'No se encontró gateway predeterminado'",
    ),
),
```

Recompila y el comando aparece automáticamente en el submenú:

```bash
cargo build --release
```

---

## Caso 2 — Añadir un nuevo submenú completo

### Paso 1 — Crear el archivo del submenú

Crea `src/submenus/<nombre>.rs`.  
Puedes copiar `network.rs` como plantilla y adaptar el contenido.

Estructura mínima:

```rust
//! # Submenú: <Nombre>
//!
//! Breve descripción de qué agrupa este submenú.

use super::{run_shell, MenuItem, SubMenu};

/// Construye y devuelve el submenú de <nombre>.
pub fn create() -> SubMenu {
    SubMenu::new(
        "<Nombre>",              // aparece en el menú principal
        "<Descripción corta>",   // aparece junto al nombre
        vec![
            MenuItem::new(
                "Primer comando",
                "Descripción del primer comando",
                || run_shell("comando_shell"),
            ),
            // Añade más MenuItem::new(...) aquí
        ],
    )
}
```

### Paso 2 — Declarar el módulo en mod.rs

Abre [src/submenus/mod.rs](../src/submenus/mod.rs) y añade la declaración del módulo en la sección indicada:

```rust
// ── Módulos de submenús ───────────────────────────────────────────────────────
pub mod network;
pub mod storage;
pub mod users;
pub mod <nombre>;   // ← añade aquí
```

### Paso 3 — Registrar el submenú en get_submenus()

En el mismo archivo `mod.rs`, añade la llamada a `create()` dentro de `get_submenus()`:

```rust
pub fn get_submenus() -> Vec<SubMenu> {
    vec![
        network::create(),
        users::create(),
        storage::create(),
        <nombre>::create(),   // ← añade aquí
    ]
}
```

El **orden del `vec![]` determina el orden de aparición** en el menú principal.

### Paso 4 — Compilar

```bash
cargo build --release
./target/release/core_menu
```

El nuevo submenú aparece en el menú principal de forma automática.

---

## Referencia de la función run_shell()

Definida en `src/submenus/mod.rs`. Ejecuta una cadena con `sh -c "..."` y devuelve la salida como `String`.

```rust
pub fn run_shell(cmd: &str) -> String
```

Comportamiento:

| Situación | Resultado devuelto |
|-----------|--------------------|
| Comando con salida en stdout | Texto de stdout |
| stdout vacío pero stderr con contenido | `[stderr]:\n<texto>` |
| Comando sin ninguna salida | `(Sin salida)` |
| Error al invocar el proceso | `Error al ejecutar el comando: <detalle>` |

Los códigos de escape ANSI se eliminan automáticamente antes de devolver el texto.

> **Nota de seguridad:** Usa `run_shell` solo con cadenas de comando **literales** definidas en el código fuente. Nunca construyas el argumento `cmd` con datos introducidos por el usuario.

---

## Referencia de tipos

### SubMenu

```rust
pub struct SubMenu {
    pub name:        &'static str,   // nombre en el menú principal
    pub description: &'static str,   // descripción junto al nombre
    pub items:       Vec<MenuItem>,  // lista de comandos
}
```

### MenuItem

```rust
pub struct MenuItem {
    pub name:        &'static str,   // nombre visible en el submenú
    pub description: &'static str,   // descripción junto al nombre
    pub command:     fn() -> String, // función que ejecuta el comando
}
```

El campo `command` es un **puntero de función** (no un closure con captura). Todas las lambdas del estilo `|| run_shell("...")` cumplen este requisito mientras no capturen variables del entorno.

---

## Ejemplo completo — submenú "Procesos"

`src/submenus/processes.rs`:

```rust
//! # Submenú: Procesos
//!
//! Inspección de procesos en ejecución y carga del sistema.

use super::{run_shell, MenuItem, SubMenu};

pub fn create() -> SubMenu {
    SubMenu::new(
        "Procesos",
        "Procesos en ejecución y carga del sistema",
        vec![
            MenuItem::new(
                "Top CPU",
                "20 procesos que más CPU consumen",
                || run_shell("ps aux --sort=-%cpu | head -21"),
            ),
            MenuItem::new(
                "Top memoria",
                "20 procesos que más RAM consumen",
                || run_shell("ps aux --sort=-%mem | head -21"),
            ),
            MenuItem::new(
                "Carga del sistema",
                "Uptime y promedios de carga (uptime)",
                || run_shell("uptime"),
            ),
            MenuItem::new(
                "Árbol de procesos",
                "Árbol de procesos (pstree -p)",
                || run_shell("pstree -p 2>/dev/null || ps axjf"),
            ),
            MenuItem::new(
                "Servicios systemd activos",
                "Unidades de sistema en estado 'running'",
                || run_shell("systemctl list-units --type=service --state=running --no-pager"),
            ),
        ],
    )
}
```

`src/submenus/mod.rs` — cambios necesarios:

```rust
pub mod network;
pub mod processes;   // ← declarar
pub mod storage;
pub mod users;

pub fn get_submenus() -> Vec<SubMenu> {
    vec![
        network::create(),
        users::create(),
        storage::create(),
        processes::create(),   // ← registrar
    ]
}
```
