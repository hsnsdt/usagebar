import { useEffect, useState, type CSSProperties, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { BackIcon } from "../components/Icons";
import Ring from "../components/Ring";
import { t, useLang } from "../i18n";

export const APP = {
  name: "UsageTray",
  author: "Sedat Okutan",
  email: "sedat@okutan.org",
  website: "https://www.okutan.org",
  github: "https://github.com/hsnsdt/usagebar",
  issues: "https://github.com/hsnsdt/usagebar/issues",
  releases: "https://github.com/hsnsdt/usagebar/releases",
  // Also in README.md, README.tr.md and .github/FUNDING.yml.
  donate: "https://checkout.dodopayments.com/buy/pdt_0NoEIiQ04pWMGCSY9fWXV?quantity=1",
  license: "MIT",
};

function open(url: string) {
  invoke("open_url", { url }).catch(() => {});
}

function GithubGlyph() {
  return (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
      <path d="M12 2a10 10 0 0 0-3.16 19.49c.5.09.68-.22.68-.48v-1.7c-2.78.6-3.37-1.34-3.37-1.34-.45-1.16-1.11-1.47-1.11-1.47-.91-.62.07-.61.07-.61 1 .07 1.53 1.03 1.53 1.03.9 1.53 2.35 1.09 2.92.83.09-.65.35-1.09.63-1.34-2.22-.25-4.56-1.11-4.56-4.94 0-1.09.39-1.98 1.03-2.68-.1-.25-.45-1.27.1-2.64 0 0 .84-.27 2.75 1.02a9.56 9.56 0 0 1 5 0c1.91-1.29 2.75-1.02 2.75-1.02.55 1.37.2 2.39.1 2.64.64.7 1.03 1.59 1.03 2.68 0 3.84-2.34 4.68-4.57 4.93.36.31.68.92.68 1.85v2.74c0 .27.18.58.69.48A10 10 0 0 0 12 2z" />
    </svg>
  );
}
function MailGlyph() {
  return (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <rect x="3" y="5" width="18" height="14" rx="2" />
      <path d="M3 7l9 6 9-6" />
    </svg>
  );
}
function GlobeGlyph() {
  return (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <circle cx="12" cy="12" r="9" />
      <path d="M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18" />
    </svg>
  );
}
function BugGlyph() {
  return (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M8 9a4 4 0 0 1 8 0v5a4 4 0 0 1-8 0z" />
      <path d="M3 13h5M16 13h5M5 7l3 2M19 7l-3 2M5 19l3-2M19 19l-3-2M10 5l1-2M14 5l-1-2" />
    </svg>
  );
}
function TagGlyph() {
  return (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M20 12l-8 8-9-9V3h8z" />
      <circle cx="7.5" cy="7.5" r="1.5" />
    </svg>
  );
}
function HeartGlyph() {
  return (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M20.8 4.6a5.5 5.5 0 0 0-7.8 0L12 5.7l-1-1.1a5.5 5.5 0 0 0-7.8 7.8l1 1.1L12 21l7.8-7.5 1-1.1a5.5 5.5 0 0 0 0-7.8z" />
    </svg>
  );
}
function ScaleGlyph() {
  return (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M12 3v18M5 21h14M7 7h10M4 14l3-7 3 7a3 3 0 0 1-6 0zM14 14l3-7 3 7a3 3 0 0 1-6 0z" />
    </svg>
  );
}

export default function About({ onBack }: { onBack: () => void }) {
  useLang();
  const [version, setVersion] = useState("");
  useEffect(() => {
    invoke<string>("app_version").then(setVersion).catch(() => {});
  }, []);

  return (
    <>
      <header className="head head--sub">
        <div className="head__text">
          <div className="head__title">
            <button className="iconbtn iconbtn--back" title={t("back")} aria-label={t("back")} onClick={onBack}>
              <BackIcon />
            </button>
            <span>{t("about")}</span>
          </div>
        </div>
      </header>

      <div className="stack">
        <section className="hero hero--welcome" style={{ "--tone": "var(--green)" } as CSSProperties}>
          <Ring size={84} stroke={9} value={62} tone="green" marker={45}>
            <span className="ring__pct ring__pct--sm tone-text-green">62</span>
            <span className="ring__unit">%</span>
          </Ring>
          <div className="hero__text">
            <div className="hero__big hero__big--sm">{APP.name}</div>
            <div className="hero__sub">{version ? `v${version}` : ""} · {t("aboutTagline")}</div>
          </div>
        </section>

        <div className="eyebrow eyebrow--group">{t("aboutDeveloper")}</div>
        <section className="row-card row-card--list">
          <LinkRow icon={<MailGlyph />} title={APP.author} sub={APP.email} onClick={() => open(`mailto:${APP.email}`)} />
          <LinkRow icon={<GlobeGlyph />} title={t("aboutWebsite")} sub="www.okutan.org" onClick={() => open(APP.website)} />
        </section>

        <section className="row-card row-card--list">
          <LinkRow icon={<HeartGlyph />} title={t("aboutDonate")} sub={t("aboutDonateSub")} onClick={() => open(APP.donate)} />
        </section>

        <div className="eyebrow eyebrow--group">{t("aboutProject")}</div>
        <section className="row-card row-card--list">
          <LinkRow icon={<GithubGlyph />} title={t("aboutSource")} sub="github.com/hsnsdt/usagebar" onClick={() => open(APP.github)} />
          <LinkRow icon={<BugGlyph />} title={t("aboutIssue")} sub={t("aboutIssueSub")} onClick={() => open(APP.issues)} />
          <LinkRow icon={<TagGlyph />} title={t("aboutReleases")} sub={t("aboutReleasesSub")} onClick={() => open(APP.releases)} />
          <LinkRow icon={<ScaleGlyph />} title={t("aboutLicense")} sub={`${APP.license} · Inter: SIL OFL`} />
        </section>

        <p className="about__note">{t("aboutPrivacy")}</p>
      </div>
    </>
  );
}

function LinkRow({ icon, title, sub, onClick }: { icon: ReactNode; title: string; sub: string; onClick?: () => void }) {
  const Tag = onClick ? "button" : "div";
  return (
    <Tag className={`feature feature--row${onClick ? " feature--link" : ""}`} onClick={onClick} type={onClick ? "button" : undefined}>
      <span className="feature__icon">{icon}</span>
      <div className="feature__text">
        <div className="feature__title">{title}</div>
        <div className="feature__body">{sub}</div>
      </div>
      {onClick && (
        <svg className="feature__chev" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
          <path d="M9 5l7 7-7 7" />
        </svg>
      )}
    </Tag>
  );
}
