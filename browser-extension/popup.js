"use strict";

const BRIDGE = "http://127.0.0.1:17889";
const connectButton = document.getElementById("connect");
const status = document.getElementById("status");

async function callBridge(path, init = {}) {
  const response = await fetch(`${BRIDGE}${path}`, {
    ...init,
    credentials: "omit",
    cache: "no-store",
    redirect: "error",
    signal: AbortSignal.timeout(15000),
    headers: { "X-Tubefast-Extension": chrome.runtime.id, ...init.headers },
  });
  const body = await response.json();
  if (!response.ok) throw new Error(body.error || "No se pudo conectar con Tubefast.");
  return body;
}

connectButton.addEventListener("click", async () => {
  connectButton.disabled = true;
  status.className = "";
  status.textContent = "Conectando tu cuenta…";
  try {
    const challenge = await callBridge("/v1/challenge");
    if (challenge.app !== "Tubefast Stats" || !/^[a-f0-9]{64}$/.test(challenge.nonce)) {
      throw new Error("La conexión local no corresponde a Tubefast.");
    }
    const all = await chrome.cookies.getAll({ url: "https://music.youtube.com/" });
    const cookies = all
      .filter(cookie => cookie.domain === "youtube.com" || cookie.domain.endsWith(".youtube.com"))
      .map(({ name, value, domain }) => ({ name, value, domain }));
    if (!cookies.some(cookie => (cookie.name === "SAPISID" || cookie.name === "__Secure-3PAPISID") && cookie.value)) {
      throw new Error("Inicia sesión en YouTube Music en este navegador y vuelve a pulsar Conectar.");
    }
    await callBridge("/v1/session", {
      method: "POST",
      headers: { "Content-Type": "application/json", Authorization: `Bearer ${challenge.nonce}` },
      body: JSON.stringify({ cookies }),
    });
    status.className = "success";
    status.textContent = "Cuenta conectada. Ya puedes volver a Tubefast.";
    connectButton.textContent = "Conectado ✓";
  } catch (error) {
    status.textContent = error instanceof TypeError || error.name === "TimeoutError"
      ? "Abre Tubefast, pulsa Sign in → Continue in browser y vuelve a Conectar."
      : error.message;
    connectButton.disabled = false;
  }
});

document.getElementById("youtube").addEventListener("click", () => {
  chrome.tabs.create({ url: "https://music.youtube.com/" });
});
