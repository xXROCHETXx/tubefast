# Autenticación de Tubefast Stats — versión 4

Fecha: 30 de octubre de 2026. Esta versión reemplaza el acceso manual de la versión 3 por conexión automática, usando una ventana normal de Edge o Chrome. No se inició sesión con cuentas reales del usuario durante el desarrollo.

## Experiencia de acceso

1. Pulsa **Conectar con Google** en Tubefast.
2. Se abre una ventana normal, con barra de direcciones y pestañas. En Windows se prefiere el navegador predeterminado cuando es Edge o Chrome; en caso contrario se busca uno compatible instalado.
3. Inicia sesión en la página oficial de Google. Si Google ofrece una clave de acceso desde otro dispositivo, se puede usar el QR con el iPhone y Face ID.
4. Tubefast detecta la sesión de YouTube, la valida contra YouTube y conecta la cuenta automáticamente. Después cierra su ventana temporal. No hay que copiar cookies, pegar datos ni usar herramientas de desarrollo.

El acceso requiere Edge o Chrome instalado. No se implementó OAuth ni un QR propio: Google gestiona la autenticación y Tubefast recibe la sesión resultante de su propio perfil temporal.

## Fallo corregido

El código original vigilaba el proceso que arrancaba Edge/Chrome. En esta máquina ese proceso puede terminar mientras el navegador real sigue abierto. La prueba original, con cookies ficticias, reprodujo el falso aviso de cierre.

El nuevo flujo vigila respuestas de la conexión local del navegador y sus páginas, no la vida del lanzador. Tolera el arranque y desconexiones breves. Cada intento usa un perfil nuevo y un puerto elegido por el navegador, descubierto en el archivo de ese perfil, lo que evita reutilizar un intento anterior o competir por un puerto preseleccionado.

La sesión se comprueba antes de cerrar la ventana. Una cookie todavía incompleta no se guarda como conexión exitosa. La validación usa un cliente separado y los resultados llevan un identificador de intento: cancelar y volver a probar no permite que un resultado anterior conecte otra cuenta.

## Qué datos maneja la app

- La contraseña y la verificación biométrica se introducen en Google y el sistema; Tubefast no solicita contraseñas ni recibe datos de Face ID.
- El navegador usa un perfil separado de las sesiones habituales, con extensiones y sincronización desactivadas para ese arranque.
- Un canal de depuración local, conectado a `127.0.0.1`, recupera las cookies del perfil temporal. El código filtra dominios `youtube.com` y subdominios reales; excluye Google y dominios parecidos de terceros.
- La app valida la cookie mediante las solicitudes HTTPS de `src/ytm.rs` a `music.youtube.com`. No encontré que se adjunte a GitHub, Discord, lrclib o un servidor del autor.
- En Windows la cookie aceptada se guarda cifrada con DPAPI en `session.bin`. En macOS/Linux el código sigue usando un archivo sin cifrado creado con modo `0600`.
- La limpieza normal intenta cerrar el navegador temporal y eliminar únicamente el perfil creado para ese intento. Comprueba que la ruta resuelta permanezca bajo su directorio antes de eliminarla.

## Límites

DPAPI no aísla el archivo frente a todo programa ejecutado con el mismo usuario. Las cookies son credenciales reutilizables y el puerto local es sensible mientras el acceso está abierto. Un bloqueo del sistema, cierre forzado o restricción empresarial puede impedir el acceso o la limpieza; no se garantiza el borrado de respaldos ni de todos los residuos de un cierre abrupto.

Cerrar sesión en Tubefast elimina la sesión local, pero no revoca una copia robada en Google. En macOS/Linux el modo de creación tampoco corrige permisos más amplios de un archivo que ya existiera.

El endpoint de audio sigue consultando sin cookies. El indicador compara AAC anunciados en esa respuesta; no certifica la máxima calidad Premium de la cuenta.

Esta revisión acotada no certifica todas las dependencias ni los binarios de terceros. Preferiría usar una cuenta secundaria antes que mi cuenta principal.

## Verificación

Pasaron **39 pruebas**, incluyendo tres con Edge real en modo sin interfaz, perfiles nuevos, `about:blank` y cookies exclusivamente ficticias:

- La sesión validada se recupera aunque el proceso lanzador haya terminado.
- La cancelación cierra el navegador de prueba y elimina su perfil.
- El navegador permanece disponible después de rechazar una sesión incompleta y conecta cuando la nueva sesión pasa la validación.

También se comprobaron el filtrado de dominios, los tiempos de arranque/desconexión, la separación y eliminación de perfiles y la protección DPAPI de Windows. Clippy pasó sin advertencias. No se completó un acceso real a Google con ninguna cuenta.

## Referencias

- [Código del fork](https://github.com/xXROCHETXx/tubefast).
- [Microsoft: alcance y límites de CryptProtectData](https://learn.microsoft.com/en-us/windows/win32/SecCrypto/example-c-program-using-cryptprotectdata).
- [Chrome: uso de perfiles separados para depuración](https://developer.chrome.com/blog/remote-debugging-port).
- [Chrome DevTools Protocol y descubrimiento del puerto](https://chromedevtools.github.io/devtools-protocol/).
- [Google: claves de acceso y acceso desde otro dispositivo](https://support.google.com/accounts/answer/13548313?hl=es).
