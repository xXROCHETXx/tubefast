# Autenticación de Tubefast Stats — versión 5

Fecha: 30 de octubre de 2026. Esta versión usa la sesión del navegador habitual mediante **Tubefast Connect**, una extensión con permiso explícito para YouTube. El panel nativo recupera su título, botón y presentación original sin instrucciones numeradas.

## Por qué se cambió el acceso

La versión 4 probó correctamente el control de un navegador aislado y la lectura de cookies ficticias, pero eso no comprobaba que Google aceptara un acceso real. El usuario recibió “No puedes acceder / Es posible que no sean seguros este navegador o la app”. Google documenta que puede rechazar navegadores controlados mediante software. La captura no identifica por sí sola una causa única.

Se retiró por completo ese lector y su apertura con depuración. La versión nueva no abre un perfil controlado ni intenta ocultar automatización para eludir el rechazo. Se conecta a una sesión de YouTube ya abierta normalmente en Edge/Chrome.

## Recorrido de la versión 5

1. El usuario instala Tubefast Connect en el perfil donde usa YouTube. El navegador concede el permiso de cookies para YouTube.
2. En la app pulsa **Sign in → Continue in browser**. Se abre YouTube Music normalmente y se habilita durante un tiempo limitado un servidor local, únicamente en `127.0.0.1:17889`.
3. Al pulsar **Conectar con Tubefast** en la extensión, primero se solicita el desafío del intento pendiente. Solo después se llama a la API de cookies del navegador para `https://music.youtube.com/`.
4. La extensión envía las cookies de YouTube al endpoint local, con el identificador de la extensión y un token aleatorio de 256 bits específico del intento.
5. La app verifica la sesión con YouTube mediante un cliente separado. Solo un resultado del intento actual conecta y guarda la cuenta. Cancelar invalida el intento y cierra el servidor.
6. En Windows se conserva el cifrado DPAPI de `session.bin`. En macOS/Linux continúa el archivo privado creado con modo `0600`, sin cifrado propio.

La extensión no solicita cookies de Google, contraseñas guardadas, historial ni acceso general a todos los sitios. No hay scripts de fondo que importen sesiones al visitar páginas. No usa portapapeles, analítica, almacenamiento de la extensión ni servidores del autor.

## Controles y límites

- El servidor comprueba el origen de la extensión, Host, rutas, tipos y tamaño de peticiones y el token del intento. Rechaza páginas de terceros y tokens de intentos anteriores.
- El filtrado de dominios se repite en la extensión y en Rust. Los dominios parecidos y valores que inyecten separadores/controles no se aceptan como credenciales válidas.
- La comunicación se limita al equipo local. No se pretende autenticar el ejecutable frente a malware del mismo usuario: un programa local malicioso sigue siendo una amenaza. Hay que confiar tanto en la extensión instalada como en el ejecutable.
- Las cookies son credenciales reutilizables. El cifrado DPAPI no aísla la sesión frente a todos los procesos del mismo usuario. Cerrar sesión en la app borra la copia local; no revoca una copia robada en Google.
- Si hay varias cuentas de Google dentro del mismo perfil, comprueba el nombre de cuenta mostrado por Tubefast: el cliente original usa el índice de autenticación 0.
- El indicador de máxima AAC sigue comparando los formatos anunciados en la respuesta del reproductor sin cookies; no certifica máxima calidad Premium.

## Estado de distribución

El complemento está preparado y probado localmente, pero no se ha publicado en Edge Add-ons/Chrome Web Store ni instalado en el perfil real del usuario. La prueba inicial requiere cargarlo como extensión de desarrollo. Para distribuir una instalación de un botón hay que publicar y pasar la revisión de la tienda. Si la tienda cambia su identificador, debe actualizarse la lista autorizada de la app antes de distribuirla.

El registro OAuth de Google no es necesario para esta vía. No se implementó OAuth ni un QR propio. Si hace falta volver a acceder a Google, se hace normalmente en el navegador; Google decide qué opciones de clave de acceso o Face ID ofrece.

## Verificación

- **37 pruebas de Rust** pasaron, incluyendo el envío por TCP local, bloqueo de orígenes ajenos, token del intento, cancelación y filtrado de cookies.
- **4 pruebas de JavaScript** pasaron: no leer cookies antes de una conexión iniciada explícitamente, no leerlas cuando la app no está disponible, exclusión de otros dominios y coincidencia del identificador de extensión con el servidor.
- Una prueba adicional con **Edge real en un perfil de prueba aislado** cargó la extensión y comprobó la lectura de una cookie HttpOnly ficticia de YouTube, exclusión de Google y envío local completo. No se accedió a cuentas reales.
- Clippy pasó sin advertencias. No se completó una conexión real de la cuenta del usuario; esa validación requiere instalar y usar la extensión con su permiso.

## Referencias

- [Google: navegadores admitidos y rechazo de automatización](https://support.google.com/accounts/answer/7675428?hl=es).
- [Chrome: API de cookies, permisos y HttpOnly](https://developer.chrome.com/docs/extensions/reference/api/cookies).
- [Chrome: peticiones desde una extensión](https://developer.chrome.com/docs/extensions/develop/concepts/network-requests).
- [Microsoft: alcance de CryptProtectData](https://learn.microsoft.com/en-us/windows/win32/SecCrypto/example-c-program-using-cryptprotectdata).
- [Código del fork](https://github.com/xXROCHETXx/tubefast).
