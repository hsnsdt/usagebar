import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Settings } from "../types";

type Props = { onDone: (s: Settings) => void };

/** First-run screen. Saving creates settings.json, which ends first-run mode. */
export default function Welcome({ onDone }: Props) {
  const [s, setS] = useState<Settings | null>(null);

  useEffect(() => {
    invoke<Settings>("get_settings").then(setS).catch(() => {});
  }, []);

  const start = async () => {
    if (!s) return;
    const saved = await invoke<Settings>("save_settings", { settings: s });
    onDone(saved);
  };

  return (
    <>
      <header className="head">
        <div className="head__title">
          <span>UsageTray'e hoş geldin</span>
        </div>
      </header>

      <div className="welcome">
        <p className="welcome__lead">
          Tepsideki halka 5 saatlik pencerenin doluluğunu gösterir; tıklayınca bu panel açılır.
        </p>

        <ul className="welcome__list">
          <li>
            <span className="welcome__k">Tepsi ikonu</span>
            Windows 11 ikonu taşma menüsüne gizleyebilir; görünür olsun istersen görev çubuğuna sürükle.
          </li>
          <li>
            <span className="welcome__k">Gizlilik</span>
            Token yalnızca api.anthropic.com adresine gider. Credential dosyası hiç yazılmaz, telemetri yoktur.
          </li>
          <li>
            <span className="welcome__k">Bildirimler</span>
            %50, %75 ve %90'da birer kez uyarı alırsın; eşikler ayarlardan değişir.
          </li>
        </ul>

        {s && (
          <label className="row row--toggle">
            <div className="row__label">
              <span>Windows ile başlat</span>
              <span className="row__hint">arka planda, görev çubuğu olmadan</span>
            </div>
            <span className={`switch${s.startWithWindows ? " switch--on" : ""}`}>
              <input
                type="checkbox"
                checked={s.startWithWindows}
                onChange={(e) => setS({ ...s, startWithWindows: e.target.checked })}
              />
              <span className="switch__knob" />
            </span>
          </label>
        )}
      </div>

      <footer className="foot foot--welcome">
        <button className="btn btn--primary" disabled={!s} onClick={start}>
          Başla
        </button>
      </footer>
    </>
  );
}
