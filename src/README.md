# Lumineria Launcher — reestructuración de carpetas

Este paquete es tu mismo código (HTML/CSS/JS del frontend), reorganizado en una
estructura por capas/feature. **No cambié ninguna lógica de negocio** — solo moví
código, dividí archivos grandes en piezas más chicas, arreglé rutas relativas que
se rompían al mover archivos, y quité un par de imports que no se usaban.
Los bugs de comportamiento que encontré los dejé **intactos** y los listo abajo
para que decidas si los corrijo.

## Estructura nueva

```
index.html
assets/                          (tus imágenes/cursor van acá, sin cambios)
css/
  base/
    variables.css                (:root — tokens de diseño)
    reset.css                    (*, body, .hidden, .mouse-glow)
  layout/
    layout.css                   (contenedor, sidebar, content, vistas)
  components/
    sidebar.css
    hero.css
    panel.css                    (action-panel / status-text)
    buttons.css                  (primary-btn / secondary-btn)
    forms.css                    (inputs de ajustes, <select>)
    cards.css                    (grid + profile-card + progreso + dropdown)
    modals.css
    mods.css                     (lista de mods/resourcepacks + toggle-switch)
    console.css
    loader-picker.css
    instance-detail.css          (tabs, logs, controles de la vista de detalle)
js/
  core/
    tauri.js                     (NUEVO: único punto de acceso a window.__TAURI__)
    state.js
    main.js
  ui/
    ui.js
    dialogs.js
    console.js
  features/
    auth/
      auth.js
    instances/
      creator.js
      instanceDetail.js
      launcher.js
      mods.js
      resourcePacks.js
    explore/
      explore.js
```

**Por qué así:** `core` es lo que no depende de nada (estado + acceso a Tauri),
`ui` es todo lo que dibuja cosas genéricas en pantalla (tarjetas, diálogos, consola),
y `features` agrupa cada área funcional (auth, instancias, explorar) en su propia
carpeta. Cuando agregues algo nuevo (por ejemplo, una pestaña de "screenshots" o un
sistema de logros), le creás su propia carpeta en `features/` sin tocar el resto.

## Cambios mecánicos que hice (no de comportamiento)

- **`js/core/tauri.js` es nuevo.** Antes cada archivo repetía
  `const { invoke } = window.__TAURI__.core;` (y a veces `listen`). Ahora todos
  importan `invoke`/`listen` desde ahí. Un solo lugar para tocar si algún día cambia
  la forma de hablar con Tauri.
- **Rutas relativas de imágenes en CSS.** `layout.css` y `reset.css` ahora están un
  nivel más profundo (`css/layout/`, `css/base/`), así que sus `url('../assets/...')`
  pasaron a `url('../../assets/...')`. Si no ajustaba esto, el fondo de galaxia, el
  cristal central y el cursor personalizado se hubieran roto.
- **CSS duplicado, unificado:** `.view-section`, `.hero` y `.action-panel` estaban
  definidos dos veces cada uno (una vez en el viejo `components.css`, y `.view-section`
  también en `layout.css`). Dejé una sola definición por selector con el resultado
  final que ya se estaba aplicando (no cambia nada visualmente, solo es más fácil de
  mantener).
- Quité 2-3 imports que no se usaban (`PROFILES` en `creator.js` y `mods.js`).

## Cosas que encontré y que sí te recomiendo revisar

### 🔴 Importante — instancias "Vanilla" personalizadas usan tu carpeta `.minecraft` real

En `state.js`, `getInstanceDir()` tiene esto:

```js
export async function getInstanceDir(profileId) {
    const profile = PROFILES[profileId];
    if (!profile || profile.loader_name.toLowerCase() === 'vanilla') {
        return await invoke('get_minecraft_default_path');
    }
    ...
}
```

Esto se pensó para las instancias que el launcher **detecta** en tu `.minecraft` real
(las de "Detectado en .minecraft (PC)"), pero como la función solo mira
`loader_name`, también afecta a **cualquier instancia que vos crees a mano con
Cargador = "Vanilla"** desde "+ Nueva Instancia". Esas instancias:
- no consiguen su propia carpeta en `instances/<id>` (a diferencia de Fabric/Forge/NeoForge),
- se instalan directo en tu `.minecraft` real,
- y si le das clic a **"Reinstalar"** (que llama a `resetInstanceLibraries`), el juego
  ejecuta `reset_instance_libraries` **sobre tu `.minecraft` real**, no sobre una copia
  aislada.

Si tu intención era que las instancias Vanilla creadas por el usuario también estén
aisladas (como Fabric/Forge), esto es un bug con riesgo real de que "Reinstalar" te
borre/resetee tu instalación real de Minecraft. La forma más simple de arreglarlo es
que `getInstanceDir` reciba explícitamente si el perfil es una detección local
(`isLocal`) en vez de inferirlo del nombre del loader — hoy esa distinción existe en
otras partes del código (`launcher.js`, `instanceDetail.js`) pero no llega hasta acá.

Decime si querés que lo arregle y lo hago en un mensaje aparte (no lo toqué en este
paquete para no mezclar "reestructurar" con "cambiar comportamiento").

### 🟡 Estado de "instancia corriendo" duplicado en dos lugares

`ui.js` mantiene su propio `Set` (`runningInstances`) y `instanceDetail.js` mantiene su
propio objeto (`INSTANCE_STATE`), y los dos escuchan `game-started`/`game-stopped` por
separado. Funciona porque hoy están sincronizados, pero es fácil que se desincronicen
si a futuro se agrega un tercer lugar que necesite saber "¿está corriendo esta
instancia?". A futuro convendría que ese estado viva en un solo lugar (por ejemplo en
`state.js`) y que `ui.js` / `instanceDetail.js` solo lo lean.

### 🟢 Menores

- La clase `.local-pc-card` se agrega en `ui.js` a las tarjetas detectadas en tu
  `.minecraft` real, pero nunca tuvo una regla CSS propia — hoy no hace nada visualmente
  aparte del badge "(PC)". Si querés diferenciarlas más (borde de otro color, por
  ejemplo), avisame.
- `getSystemRamMb` se importa en `ui.js` pero no se usa ahí — quedó de algo que se
  sacó o que se va a usar en el panel de ajustes. Lo dejé tal cual, solo para que lo
  tengas en el radar.

## Cómo lo uso en el proyecto Tauri

Copiá el contenido de este paquete dentro de la carpeta que tu `tauri.conf.json`
apunta como `frontendDist` (probablemente reemplazando tu `src/` actual), y copiá
tu carpeta `assets/` real ahí adentro (no la incluí porque no tengo tus imágenes).
No hace falta tocar nada del lado de Rust — todos los `invoke(...)` quedaron con el
mismo nombre y los mismos argumentos.
