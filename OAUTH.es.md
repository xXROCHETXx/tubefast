# Activar Google OAuth en Tubefast Stats

El código está implementado; esta compilación todavía no incluye un cliente OAuth registrado. Sin ese registro no se puede probar ni completar un inicio de sesión real. La extensión y el acceso mediante navegador controlado fueron retirados.

## Configuración del propietario del fork

Se hace una vez para el proyecto, no una vez por usuario:

1. En [Google Cloud Console](https://console.cloud.google.com/), crea o selecciona un proyecto del fork y habilita **YouTube Data API v3**.
2. En **Google Auth Platform**, configura el nombre **Tubefast Stats**, contacto y audiencia. Para la primera prueba usa el modo de pruebas y añade tu cuenta como usuario de prueba.
3. Solicita solamente `https://www.googleapis.com/auth/youtube.readonly` en los permisos de datos.
4. Crea un cliente OAuth de tipo **Desktop app / Aplicación de escritorio** y descarga su JSON. Un cliente web, Android o de TV no sirve para este flujo.
5. Guarda el JSON descargado como **oauth-client.json**, junto a **tubefast.exe**. Conserva el objeto `installed` tal como viene de Google. Reinicia la app y pulsa **Sign in → Continue in browser**.

La app abrirá el navegador predeterminado, Google pedirá permiso y la conexión terminará automáticamente al volver a Tubefast. No se necesitan cookies ni contraseñas dentro del programa. Si Google ofrece una clave de acceso con iPhone/Face ID, puedes usarla en su página.

Para distribuir un único ejecutable ya configurado, coloca ese JSON en la raíz del código al compilar o indica su ruta mediante `TUBEFAST_OAUTH_CLIENT_FILE`. `build.rs` lo incorpora; el archivo está excluido de Git. El JSON de ejemplo contiene marcadores y no activa OAuth. Un cliente de escritorio es un cliente público: el `client_secret` incluido no debe considerarse un secreto que el ejecutable pueda ocultar. Los tokens personales nunca forman parte del paquete.

## Publicar para otras personas

El modo de pruebas sirve para las cuentas de prueba autorizadas, no para una distribución abierta. Los refresh tokens de una app externa en pruebas normalmente caducan a los siete días para estos permisos. Para distribuir el fork sin esas limitaciones, configura la publicación y completa la verificación que Google solicite para el permiso sensible de YouTube. Esto puede requerir información del producto, dominio, política de privacidad y revisión. Crear un client ID por sí solo no garantiza acceso para todos los usuarios. La cuota de la Data API es compartida por el proyecto; revisa su consumo antes de distribuir.

## Funciones y límites

- Importa playlists **propias** (incluidas privadas) y sus pistas. No enumera todas las playlists de otros autores que hayas guardado en YouTube Music.
- Importa vídeos con Me gusta. Google limita `videos.list(myRating=like)` a los **1.000 más recientes**. Puede incluir vídeos que no sean canciones.
- La búsqueda, reproducción, letras, radio y estadísticas siguen funcionando con el reproductor original. OAuth no agrega acceso a streams Premium ni selección de calidad.
- No importa todos los álbumes guardados ni el inicio personalizado de YouTube Music. El inicio se construye con la escucha local.
- Los nuevos favoritos y las eliminaciones de favoritos dentro de Tubefast se conservan localmente por cuenta; el permiso de lectura no cambia tu cuenta de Google.
- La app limita la importación de una lista a 10.000 elementos e informa si se supera. Un vídeo privado o eliminado que no devuelve metadatos utilizables se omite.

## Seguridad y validación pendiente

OAuth usa PKCE S256, un estado aleatorio por intento y un servidor temporal en `127.0.0.1` con puerto aleatorio y límite de cinco minutos. Los tokens solo se envían a endpoints HTTPS fijos de Google y no se mezclan con las cookies del cliente antiguo. En Windows se guardan con DPAPI; cerrar sesión elimina la copia local. Puedes revocar el acceso en [las conexiones de tu cuenta de Google](https://myaccount.google.com/connections).

Las pruebas automáticas verifican el protocolo local con datos ficticios, los permisos, renovación y parsers de la API. **Todavía falta comprobar el consentimiento y la importación con una cuenta real, después de configurar el cliente.**

Referencias: [OAuth para escritorio](https://developers.google.com/identity/protocols/oauth2/native-app), [verificación de permisos sensibles](https://developers.google.com/identity/protocols/oauth2/production-readiness/sensitive-scope-verification), [caducidad de tokens](https://developers.google.com/identity/protocols/oauth2#expiration), [playlists propias](https://developers.google.com/youtube/v3/docs/playlists/list), [límite de Me gusta](https://developers.google.com/youtube/v3/docs/videos/list).
