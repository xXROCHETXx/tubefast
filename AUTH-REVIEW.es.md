# Revisión de autenticación de Tubefast

Fecha: 30 de octubre de 2026. Revisión manual del original en commit `c6c55dd6b1ef60f0339737e7279cda2ff3b433e2` y del fork Tubefast Stats. No se inició sesión ni se leyeron credenciales reales del usuario.

## Conclusión

En Windows, la afirmación de almacenamiento local cifrado es correcta: guarda las cookies en `session.bin` mediante DPAPI, asociado al usuario de Windows. No encontré un envío de esas cookies al autor, a GitHub, a Discord o a lrclib en los recorridos revisados. Sí se envían a YouTube por HTTPS para autenticar solicitudes, como es necesario para usar la cuenta.

Eso no equivale a una garantía de seguridad. La app recibe cookies de sesión reutilizables, no un token OAuth con permisos limitados. Yo preferiría usarla sin cuenta o con una cuenta secundaria antes que con mi cuenta principal de Google. Esta conclusión es una revisión acotada del código; no certifica los binarios publicados ni descarta problemas en dependencias o cambios futuros.

## Recorrido verificado

1. `src/auth.rs`, `browser_sign_in`, `launch`, `watch`: abre Edge/Chrome con un perfil separado (`signin-profile`) en la página oficial de Google. La contraseña se introduce en el navegador. La app usa el protocolo de depuración local para solicitar cookies; `youtube_cookie` conserva solo dominios `youtube.com` y subdominios reales.
2. `clean_cookie`: la alternativa manual acepta el encabezado completo que pegues. Ese método no puede verificar el dominio de origen de cada cookie. El campo se muestra sin ocultar y el código no limpia el portapapeles.
3. `store`, `seal`, `load`: en Windows, `CryptProtectData`/`CryptUnprotectData` cifran y recuperan el archivo. La ruta se calcula mediante `eframe::storage_dir(app_id())`; el fork usa un identificador distinto al original. La cookie descifrada se mantiene en memoria mientras se usa.
4. `src/ytm.rs`, `music`: solo aquí añade explícitamente `Cookie` y `Authorization: SAPISIDHASH` a solicitudes hacia `https://music.youtube.com/youtubei/v1/`. El hash no evita que la cookie completa también se envíe. La dependencia fijada `ureq 2.12.1` elimina el encabezado Cookie al redirigir y limita la propagación de Authorization.
5. `src/app.rs`, `sign_out`: borra la sesión en memoria y pide borrar `session.bin`. No llama a un endpoint de Google para revocar la sesión remotamente. El borrado local ignora errores y no garantiza eliminar copias o respaldos.

## Límites y riesgos concretos

- **DPAPI protege el archivo, no contra todo programa del mismo usuario.** Otro proceso que ejecute con tu usuario y acceda al archivo puede invocar DPAPI. No usa una protección ligada exclusivamente al ejecutable. En memoria, las cookies están disponibles para que la app pueda enviarlas.
- **Depuración durante el inicio de sesión.** Se usa un puerto TCP local y `Storage.getCookies`; el canal permite acceso sensible al perfil temporal. La app intenta cerrar el navegador y borrar el perfil al terminar, cancelar o agotar el tiempo. Un cierre forzado de la app puede impedir esa limpieza y los errores de borrado se ignoran. No vi pruebas de exposición deliberada a la red externa; el cliente conecta a `127.0.0.1`. Otro proceso local sigue siendo un riesgo durante ese intervalo.
- **macOS/Linux no cifran el archivo en este código.** `seal` devuelve los bytes originales. Se crea con modo `0600`, pero ese modo no corrige permisos de un archivo que ya existiera con permisos más amplios. El cifrado de disco o llavero del sistema no se implementa aquí.
- **Las cookies son credenciales de sesión.** No deben compartirse por chat ni publicarse. El inicio de sesión por navegador evita el paso de copiarlas al portapapeles, aunque sigue entregándoselas a la app mediante depuración.
- **Cerrar sesión en la app es una limpieza local.** Ante sospecha de robo, revisa y cierra las sesiones correspondientes en la seguridad de tu cuenta Google; borrar el archivo local por sí solo no invalida una copia robada.
- **Cadena de actualizaciones.** El original puede instalar una actualización desde sus releases de GitHub y comprueba SHA-256 contra el manifiesto del mismo release. Eso comprueba integridad respecto de ese release, no una auditoría independiente. El fork apunta su actualizador a su propio repositorio para no reemplazar estos cambios con el original. `clients.json` todavía se actualiza desde el original; cambia parámetros de cliente, no el dominio fijo al que se envían las cookies.
- **Otros servicios reciben datos limitados distintos de las cookies.** GitHub sirve parámetros/actualizaciones, lrclib recibe información de la canción al buscar letras y Discord recibe actividad si lo habilitas. No observé que se les adjunten cookies de YouTube.

## Calidad y cuenta conectada

`src/ytm.rs::player` no adjunta la sesión: conectarte personaliza las llamadas de biblioteca, pero no vuelve autenticada esa consulta de reproducción. Por eso el indicador del fork solo puede afirmar “máxima AAC anunciada en esta respuesta”; no puede afirmar “máxima calidad Premium de mi cuenta”. No cambia la autenticación del original.

## Fuentes y alcance

- [Código original revisado](https://github.com/yigitbozyaka/tubefast/tree/c6c55dd6b1ef60f0339737e7279cda2ff3b433e2).
- [Microsoft: alcance y límites de CryptProtectData](https://learn.microsoft.com/en-us/windows/win32/SecCrypto/example-c-program-using-cryptprotectdata).
- [Chrome: depuración remota y acceso a cookies](https://developer.chrome.com/blog/remote-debugging-port).

No se realizó una auditoría criptográfica, análisis completo de dependencias, captura de tráfico autenticado ni comparación de los binarios originales con el código. No se automatizó el acceso a ninguna cuenta.
