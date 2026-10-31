# Autenticación de Tubefast Stats — versión 3

Fecha: 30 de octubre de 2026. Esta actualización reemplaza la descripción del inicio de sesión del fork anterior. La revisión original tomó como base `c6c55dd6b1ef60f0339737e7279cda2ff3b433e2`.

## Qué fallaba

El original abría Edge/Chrome con un perfil especial y vigilaba el proceso que lo lanzaba. En esta máquina, ese proceso termina aunque el navegador real siga funcionando. La prueba original de integración con Edge, usando exclusivamente cookies ficticias, reprodujo el mismo error: “The browser window was closed before you signed in.”

La captura también muestra UrbanVPN y un aviso de iCloud. No encontré URLs de UrbanVPN ni código que instale esas extensiones en el cliente. La captura por sí sola no permite determinar qué componente del navegador abrió esa página.

## Cambio en esta versión

A petición del usuario, **Open my browser** ahora solicita al sistema abrir `https://music.youtube.com` en el navegador predeterminado. Usa la integración nativa de egui/eframe, que delega en `webbrowser::open`. No fija Edge ni Chrome, no crea perfiles temporales, no lanza ventanas de aplicación especiales y no abre puertos de depuración.

El usuario inicia sesión en su navegador y pega explícitamente la cookie de YouTube Music en Tubefast. **Abrir la página no conecta la cuenta automáticamente.** La app comprueba la cookie con YouTube antes de guardar la sesión.

La versión nueva no cierra ventanas de navegador ni elimina perfiles anteriores. Las ventanas antiguas del flujo original deben cerrarse manualmente; esta actualización no accede a ellas.

## QR, iPhone y Face ID

Google puede ofrecer una clave de acceso de otro dispositivo: el ordenador muestra un QR y el iPhone permite confirmar con Face ID. Depende de tener una clave de acceso configurada y dispositivos/navegador compatibles. Google y el sistema gestionan esa autenticación; Tubefast no recibe información biométrica.

Ese mecanismo autentica el navegador. No entrega por sí solo la sesión a Tubefast. Una conexión automática mediante OAuth requiere registrar un cliente de Google y adaptar el cliente para usar las APIs y permisos admitidos; este fork todavía usa cookies y la API interna de YouTube Music. No se añadió un QR propio ni se presentó OAuth como implementado.

## Almacenamiento y destinos

- En Windows, `src/auth.rs::store` cifra la cookie mediante DPAPI y la guarda en `session.bin` bajo el directorio de datos del fork. La cookie descifrada se conserva en memoria para autenticar solicitudes.
- En macOS/Linux este código no cifra el archivo: lo crea con modo `0600`. Ese modo no corrige los permisos de un archivo que ya existiera con permisos más amplios.
- `src/ytm.rs::music` adjunta Cookie y Authorization a solicitudes HTTPS al dominio fijo `music.youtube.com`. No encontré envío de estas credenciales al autor, a GitHub, Discord o lrclib en el recorrido revisado.
- La dependencia fijada `ureq 2.12.1` elimina Cookie al redirigir y limita la propagación de Authorization.
- Cerrar sesión borra la cookie en memoria y pide eliminar el archivo local. No revoca la sesión en Google ni invalida una copia robada.

## Límites de seguridad

El almacenamiento local cifrado en Windows es real, pero DPAPI no aísla las credenciales frente a todo proceso ejecutado con tu mismo usuario. La cookie sigue siendo una credencial reutilizable. El método manual también implica portapapeles y un campo visible: no se debe compartir la cookie por chat, publicarla ni incluirla en capturas o registros.

Yo usaría la app sin cuenta o con una secundaria antes que con mi cuenta principal. Esta revisión no certifica binarios ajenos, dependencias completas ni versiones futuras. No se inició sesión con ninguna cuenta real ni se leyeron cookies del usuario.

El endpoint de reproducción sigue consultando sin cookies: el indicador de calidad solo puede comparar AAC anunciados en esa respuesta, no certificar una máxima calidad Premium.

## Verificación y fuentes

La versión final conserva las pruebas de firmas de solicitud, limpieza de cookies ficticias y cifrado de Windows. La prueba del antiguo lector de navegador se retiró junto con ese lector. Pasaron 33 pruebas y la comprobación de Clippy; se verificó en el código de la integración nativa que la apertura utiliza el navegador predeterminado. No se completó un inicio de sesión real.

- [Base original revisada](https://github.com/yigitbozyaka/tubefast/tree/c6c55dd6b1ef60f0339737e7279cda2ff3b433e2).
- [Microsoft: alcance y límites de CryptProtectData](https://learn.microsoft.com/en-us/windows/win32/SecCrypto/example-c-program-using-cryptprotectdata).
- [Google: acceso con claves de acceso](https://support.google.com/accounts/answer/13548313?hl=es).
- [Apple: claves de acceso desde el iPhone](https://support.apple.com/en-euro/guide/iphone/iphf538ea8d0/ios).
- [Google: OAuth para dispositivos y sus requisitos](https://developers.google.com/identity/protocols/oauth2/limited-input-device).
