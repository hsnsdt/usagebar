import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { BackIcon } from "../components/Icons";
import { MIN_POLL_INTERVAL_SEC, type Settings, type Theme } from "../types";

const FIVE_HOUR_OPTIONS = [50, 75, 90];
const SEVEN_DAY_OPTIONS = [80, 95];

type Props = {
  onBack: () => void;
  onSaved: (s: Settings) => void;
};

export default function SettingsView({ onBack, onSaved }: Props) {
  const [s, setS] = useState<Settings | null>(null);
  const [version, setVersion] = useState("");
  const [savedFlash, setSavedFlash] = useState(false);

  useEffect(() => {
    invoke<Settings>("get_settings").then(setS).catch(() => {});
    invoke<string>("app_version").then(setVersion).catch(() => {});
  }, []);

  if (!s) return <div className="settings__loading">Yükleniyor…</div>;

  const patch = (p: Partial<Settings>) => setS({ ...s, ...p });
  const patchN = (p: Partial<Settings["notifications"]>) =>
    setS({ ...s, notifications: { ...s.notifications, ...p } });

  const toggleIn = (list: number[], v: number) =>
    list.includes(v) ? list.filter((x) => x !== v) : [...list, v].sort((a, b) => a - b);

  const save = async () => {
    const clean: Settings = {
      ...s,
      pollIntervalSec: Math.max(MIN_POLL_INTERVAL_SEC, Math.round(s.pollIntervalSec || 0)),
      usableContextTokens: Math.max(10_000, Math.round(s.usableContextTokens || 0)),
    };
    const saved = await invoke<Settings>("save_settings", { settings: clean });
    setS(saved);
    onSaved(saved);
    setSavedFlash(true);
    window.setTimeout(() => setSavedFlash(false), 1200);
  };

  return (
    <>
      <header className="head">
        <div className="head__title">
          <button className="iconbtn" title="Geri" aria-label="Geri" onClick={onBack}>
            <BackIcon />
          </button>
          <span>Ayarlar</span>
        </div>
        <div className="head__actions">
          <button className={`btn btn--primary${savedFlash ? " btn--ok" : ""}`} onClick={save}>
            {savedFlash ? "Kaydedildi" : "Kaydet"}
          </button>
        </div>
      </header>

      <div className="settings">
        <Row label="Yenileme aralığı" hint={`en az ${MIN_POLL_INTERVAL_SEC} sn`}>
          <input
            className="input input--num"
            type="number"
            min={MIN_POLL_INTERVAL_SEC}
            step={30}
            value={s.pollIntervalSec}
            onChange={(e) => patch({ pollIntervalSec: Number(e.target.value) })}
            onBlur={() =>
              patch({ pollIntervalSec: Math.max(MIN_POLL_INTERVAL_SEC, Math.round(s.pollIntervalSec || 0)) })
            }
          />
          <span className="unit">sn</span>
        </Row>

        <Row label="Kullanılabilir context" hint="autocompact payı düşülmüş">
          <input
            className="input input--num input--wide"
            type="number"
            min={10000}
            step={5000}
            value={s.usableContextTokens}
            onChange={(e) => patch({ usableContextTokens: Number(e.target.value) })}
          />
          <span className="unit">token</span>
        </Row>

        <Row label="Tema">
          <select className="input" value={s.theme} onChange={(e) => patch({ theme: e.target.value as Theme })}>
            <option value="system">Sistem</option>
            <option value="dark">Koyu</option>
            <option value="light">Açık</option>
          </select>
        </Row>

        <Toggle
          label="Tepsi ikonunda yüzde yazısı"
          hint="16px'te zor okunur"
          checked={s.showPercentText}
          onChange={(v) => patch({ showPercentText: v })}
        />
        <Toggle
          label="Windows ile başlat"
          checked={s.startWithWindows}
          onChange={(v) => patch({ startWithWindows: v })}
        />

        <div className="settings__group">Bildirimler</div>
        <Toggle
          label="Bildirimler açık"
          checked={s.notifications.enabled}
          onChange={(v) => patchN({ enabled: v })}
        />
        <Row label="5 saatlik eşikler">
          <div className="chips">
            {FIVE_HOUR_OPTIONS.map((p) => (
              <button
                key={p}
                className={`chip${s.notifications.fiveHour.includes(p) ? " chip--on" : ""}`}
                disabled={!s.notifications.enabled}
                onClick={() => patchN({ fiveHour: toggleIn(s.notifications.fiveHour, p) })}
              >
                %{p}
              </button>
            ))}
          </div>
        </Row>
        <Row label="Haftalık eşikler">
          <div className="chips">
            {SEVEN_DAY_OPTIONS.map((p) => (
              <button
                key={p}
                className={`chip${s.notifications.sevenDay.includes(p) ? " chip--on" : ""}`}
                disabled={!s.notifications.enabled}
                onClick={() => patchN({ sevenDay: toggleIn(s.notifications.sevenDay, p) })}
              >
                %{p}
              </button>
            ))}
          </div>
        </Row>
        <Row label="Context uyarısı" hint="kalan token bunun altına inince">
          <input
            className="input input--num input--wide"
            type="number"
            min={0}
            step={5000}
            disabled={!s.notifications.enabled}
            value={s.notifications.contextLowTokens}
            onChange={(e) => patchN({ contextLowTokens: Number(e.target.value) })}
          />
          <span className="unit">token</span>
        </Row>

        <div className="settings__foot">
          <button className="link" onClick={() => invoke("open_logs_dir")}>
            Log klasörünü aç
          </button>
          <span className="settings__version">{version && `v${version}`}</span>
          <button className="link link--danger" onClick={() => invoke("quit_app")}>
            Çıkış
          </button>
        </div>
      </div>
    </>
  );
}

function Row({ label, hint, children }: { label: string; hint?: string; children: React.ReactNode }) {
  return (
    <div className="row">
      <div className="row__label">
        <span>{label}</span>
        {hint && <span className="row__hint">{hint}</span>}
      </div>
      <div className="row__ctl">{children}</div>
    </div>
  );
}

function Toggle({
  label,
  hint,
  checked,
  onChange,
}: {
  label: string;
  hint?: string;
  checked: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <label className="row row--toggle">
      <div className="row__label">
        <span>{label}</span>
        {hint && <span className="row__hint">{hint}</span>}
      </div>
      <span className={`switch${checked ? " switch--on" : ""}`}>
        <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} />
        <span className="switch__knob" />
      </span>
    </label>
  );
}
