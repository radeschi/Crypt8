// Developed by: Maicon Radeschi
// Email: radeschi@me.com
// May the Force be with you!
// 2026

import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles/app.css";

if (import.meta.env.PROD) {
  document.addEventListener("contextmenu", (event) => {
    event.preventDefault();
  });
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
