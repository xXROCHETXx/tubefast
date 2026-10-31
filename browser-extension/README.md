# Tubefast Connect

Conecta la sesión de YouTube que ya tienes abierta en Edge o Chrome. Usa la API de cookies con permiso explícito del navegador; no modifica el inicio de sesión de Google ni abre un navegador controlado.

## Instalación inicial de esta versión de desarrollo

La extensión está preparada para pruebas locales y **todavía no está publicada en Edge Add-ons ni Chrome Web Store**.

1. Descomprime el paquete. Conserva la carpeta `browser-extension` junto a la aplicación.
2. En Edge abre `edge://extensions`. Activa **Modo de desarrollador** y pulsa **Cargar desempaquetada**.
3. Selecciona la carpeta `browser-extension` que contiene `manifest.json`. Comprueba que el nombre sea **Tubefast Connect** y el identificador `mhcegfjhlladfhgpdoejkliodmkeooao`.
4. Puedes fijarla en la barra del navegador desde el icono de extensiones. En Chrome el equivalente es `chrome://extensions`.

Hazlo en el perfil de navegador donde tienes tu cuenta de YouTube abierta. No hace falta exportar ni pegar cookies.

## Conectar

1. En Tubefast pulsa **Sign in → Continue in browser**.
2. En Edge abre **Tubefast Connect** y pulsa **Conectar con Tubefast**.
3. La app comprueba la sesión con YouTube y conecta tu biblioteca. Si no has iniciado sesión, abre YouTube Music normalmente en ese navegador y vuelve a pulsar Conectar.

## Permisos

Solo `cookies`, acceso a `https://*.youtube.com/*` y comunicación con `127.0.0.1`. La política de contenido limita esa comunicación al puerto local 17889. No hay scripts de fondo que lean sesiones automáticamente. [Política de privacidad](PRIVACY.md).

## Distribución a usuarios finales

Para que la instalación sea un botón **Obtener extensión**, el editor del fork debe publicar el complemento en Edge Add-ons/Chrome Web Store. Esa publicación requiere una cuenta de editor y la revisión de la tienda; no se ha realizado. Si la tienda asigna otro identificador, el editor debe actualizar el identificador autorizado en `src/extension_login.rs` y reconstruir la app. No se deben eliminar las comprobaciones de origen ni aceptar identificadores arbitrarios.

Las pruebas locales usan cookies ficticias en un perfil de Edge aislado. No equivalen a una certificación de Google ni de la tienda.
