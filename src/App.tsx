import { useEffect } from "react";
import { installAppMenu } from "./appMenu";
import { AboutPage } from "./pages/AboutPage";
import { EncryptPage } from "./pages/EncryptPage";

const about = new URLSearchParams(window.location.search).has("about");

function App() {
  useEffect(() => {
    if (about) return;
    void installAppMenu().catch(() => undefined);
  }, []);

  return about ? <AboutPage /> : <EncryptPage />;
}

export default App;
