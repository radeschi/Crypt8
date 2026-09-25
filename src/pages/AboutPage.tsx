import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useEffect, useState } from "react";
import {
  APP_COPYRIGHT,
  APP_LICENSE_URL,
  APP_NAME,
  APP_REPOSITORY,
  APP_WEBSITE,
  APP_WEBSITE_LABEL,
} from "../branding";
import { UpdateNotice } from "../components/UpdateNotice";
import { messages } from "../i18n/index.ts";
import appIcon from "../../src-tauri/icons/icon.png";

export function AboutPage() {
  const [version, setVersion] = useState("");

  useEffect(() => {
    void getVersion().then(setVersion).catch(() => undefined);
  }, []);

  return (
    <main className="app about">
      <header className="chrome" data-tauri-drag-region />
      <section className="about-body">
        <img className="about-icon" src={appIcon} alt="" />
        <p className="about-name">{APP_NAME}</p>
        {version && <p className="about-version">{messages.versionLabel(version)}</p>}
        <p className="about-tagline">{messages.tagline}</p>
        <p className="about-summary">{messages.aboutSummary}</p>
        <hr className="about-rule" />
        <p className="about-tech">OpenPGP</p>
        <p className="about-tech about-tech-sub">Sequoia PGP</p>
        <hr className="about-rule" />
        <p className="about-copy">{APP_COPYRIGHT}</p>
        <button className="about-site" type="button" onClick={() => void openUrl(APP_WEBSITE)}>
          {APP_WEBSITE_LABEL}
        </button>
        <p className="about-license">
          {messages.openSource}
          <button className="about-inline" type="button" onClick={() => void openUrl(APP_LICENSE_URL)}>
            GPLv3
          </button>
        </p>
        <UpdateNotice autoCheck={false} busy={false} showButton />
        <nav className="about-links">
          <button type="button" onClick={() => void openUrl(APP_WEBSITE)}>{messages.websiteLink}</button>
          {APP_REPOSITORY && (
            <button type="button" onClick={() => void openUrl(APP_REPOSITORY)}>{messages.githubLink}</button>
          )}
          <button type="button" onClick={() => void openUrl(APP_LICENSE_URL)}>{messages.licenseLink}</button>
        </nav>
      </section>
    </main>
  );
}
