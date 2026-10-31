# Autenticación de Tubefast Stats — versión 6

Revisión del 31 de octubre de 2026. La extensión fue descartada y retirada del código activo; no se instaló en el navegador del usuario. La alternativa implementada es OAuth oficial de escritorio. **Aún requiere registrar el cliente de este fork en Google Cloud y probar una autorización real.** Instrucciones: [OAUTH.es.md](OAUTH.es.md).

## Qué ocurrió con el método original

El original abría un navegador aislado con depuración para leer cookies de YouTube. Se reprodujo y corrigió un error que confundía la salida del lanzador de Edge con el cierre de la ventana. La prueba con cookies ficticias pasó, pero el inicio real fue rechazado por Google con “Es posible que no sean seguros este navegador o la app”. Google documenta que puede rechazar navegadores controlados por software. La captura no demuestra por sí sola una única causa. Reparar la espera del proceso no garantiza que Google permita autenticar en ese navegador.

La versión actual abre la URL OAuth en el navegador predeterminado mediante el sistema operativo. No abre perfiles con depuración ni intenta disimular automatización. Mantiene el título, botón y aspecto original del panel.

## Qué información recibe la app

El permiso solicitado es solamente `youtube.readonly`. La app recibe tokens OAuth para leer datos de YouTube; no recibe tu contraseña, Face ID ni cookies del navegador. La API oficial aporta playlists propias, incluidas privadas, y los vídeos con Me gusta más recientes; no sustituye toda la biblioteca ni el inicio personalizado de YouTube Music. Las modificaciones de favoritos en Tubefast son locales.

El flujo usa código de autorización con PKCE S256 y un estado aleatorio de 256 bits. La devolución ocurre en `127.0.0.1`, puerto aleatorio, dentro de cinco minutos. Se rechazan estados incorrectos, parámetros duplicados y peticiones incompletas; las peticiones están acotadas en tamaño y tiempo. Cerrar el panel cancela el intento y sus resultados antiguos se ignoran. El cliente comprueba la cuenta con la API antes de guardarla.

Los access tokens solo viajan al endpoint fijo `https://www.googleapis.com/youtube/v3/`; los códigos y refresh tokens al endpoint fijo `https://oauth2.googleapis.com/token`. Las peticiones con credenciales no siguen redirecciones. El cliente no incluye los tokens OAuth en cabeceras Cookie ni en llamadas internas de YouTube Music. Los errores mostrados no vuelcan respuestas de credenciales. No hay un servidor del autor que reciba la autorización.

## Almacenamiento y alcance de “local”

En Windows, `session.bin` se cifra con DPAPI del usuario. En macOS/Linux continúa un archivo creado con permisos `0600`, sin cifrado propio. La biblioteca y preferencias se guardan aparte en el perfil de la app. “Local” describe la persistencia: Google necesariamente recibe las peticiones de autorización y consulta, y la app mantiene conexiones de reproducción/arte/letras del original.

DPAPI no es una barrera frente a todo programa que ya ejecuta código como tu usuario. Los tokens siguen siendo credenciales reutilizables. Cerrar sesión borra la copia local; revocar el acceso desde las conexiones de tu cuenta de Google invalida la autorización. Las sesiones antiguas basadas en cookies siguen siendo legibles por compatibilidad, pero ya no hay un importador de cookies del navegador.

No encontré envío de credenciales a un servidor del autor en las rutas revisadas. Eso no permite certificar seguridad absoluta ni sustituye revisar futuros cambios y el ejecutable que se distribuya.

## Estado de pruebas

Las 43 pruebas de Rust y Clippy sin advertencias pasaron. Las pruebas automáticas cubren PKCE con el vector de RFC 7636, estado del callback, cancelación, peticiones TCP locales, renovación y rotación de tokens con datos ficticios, paginación, playlists privadas de prueba, favoritos locales y cifrado DPAPI. El selftest anónimo de la compilación final también pasó: búsqueda, decodificación AAC a 44,1 kHz, saltos de posición y radio. **Ninguna de estas pruebas demuestra todavía un consentimiento real de Google: falta el cliente registrado.**

Para publicación abierta, el propietario debe configurar la audiencia y completar la verificación de Google que corresponda. El modo de pruebas y una compilación sin client ID no garantizan acceso para todos.

Referencias: [Google OAuth para escritorio](https://developers.google.com/identity/protocols/oauth2/native-app), [rechazo de navegadores](https://support.google.com/accounts/answer/7675428?hl=es), [permisos sensibles](https://developers.google.com/identity/protocols/oauth2/production-readiness/sensitive-scope-verification), [alcance de DPAPI](https://learn.microsoft.com/en-us/windows/win32/SecCrypto/example-c-program-using-cryptprotectdata).
