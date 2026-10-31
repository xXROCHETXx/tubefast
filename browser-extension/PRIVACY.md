# Privacidad de Tubefast Connect

La extensión conecta tu sesión actual de YouTube con la aplicación Tubefast Stats en tu propio equipo.

- Solo lee cookies después de pulsar **Conectar con Tubefast** y comprobar que hay una conexión local pendiente.
- Solicita acceso a YouTube y al equipo local. No solicita acceso a cookies de Google, correo, contraseñas guardadas, historial ni otros sitios.
- Envía las cookies de YouTube únicamente a `http://127.0.0.1:17889`, para la solicitud temporal iniciada desde Tubefast. No hay un servidor del autor.
- No guarda cookies en el almacenamiento de la extensión ni las copia al portapapeles. No usa analítica ni publicidad.
- La aplicación valida la sesión con YouTube y la guarda según el sistema operativo: DPAPI en Windows; archivo privado sin cifrado propio en macOS/Linux. Véase la revisión de autenticación del repositorio.
- Las cookies son credenciales de sesión. Confía únicamente en una copia de la extensión y de la aplicación cuyo origen hayas comprobado. Este enlace local no pretende proteger frente a malware ejecutándose con tu mismo usuario.

Para retirar el acceso, elimina o desactiva la extensión desde tu navegador. Cerrar sesión en Tubefast borra su copia local; no revoca una sesión robada en Google.

Código y soporte: https://github.com/xXROCHETXx/tubefast
