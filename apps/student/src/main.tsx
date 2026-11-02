import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles.css";

// Tauri dışında, düz bir tarayıcıda açıldıysa sahte arka ucu kur.
// Üretim derlemesinde bu dal tamamen elenir (import.meta.env.DEV sabittir).
if (import.meta.env.DEV && !("__TAURI_INTERNALS__" in window)) {
  const { installDevMock } = await import("./dev-mock");
  installDevMock();
}

const root = document.getElementById("root");
if (!root) {
  throw new Error("#root bulunamadı");
}

ReactDOM.createRoot(root).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
