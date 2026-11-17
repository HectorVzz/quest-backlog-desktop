import { useEffect, useState } from "preact/hooks";

import { errorMessage, strings, type Strings } from "./i18n";
import { native, type Preferences, type PreferencesInput } from "./native";

/** The Preferences window (a normal window, opened from the tray). */
export function PreferencesApp() {
  const [language, setLanguage] = useState<"en" | "es" | null>(null);

  useEffect(() => {
    const refresh = () => native.appInfo().then((info) => setLanguage(info.language));
    refresh();
    const unlisten = native.onSettingsChanged(refresh);
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") native.closePreferences();
    };
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("keydown", onKey);
      unlisten.then((stop) => stop());
    };
  }, []);

  useEffect(() => {
    if (language) document.documentElement.lang = language;
  }, [language]);

  if (!language) return null;
  return (
    <main class="window">
      <PreferencesForm t={strings(language)} />
    </main>
  );
}

function PreferencesForm({ t }: { t: Strings }) {
  const [loaded, setLoaded] = useState<Preferences | null>(null);
  const [draft, setDraft] = useState<PreferencesInput | null>(null);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<{ text: string; error: boolean } | null>(null);

  const load = (preferences: Preferences) => {
    setLoaded(preferences);
    const { hotkeys: _hotkeys, ...input } = preferences;
    setDraft(input);
  };

  useEffect(() => {
    native.preferences().then(load);
    // Reopening the window (or the desktop changing Wayland shortcuts)
    // reloads what is on disk and bound.
    const unlisten = native.onSettingsChanged(() => native.preferences().then(load));
    return () => {
      unlisten.then((stop) => stop());
    };
  }, []);

  if (!loaded || !draft) return null;

  const update = (patch: Partial<PreferencesInput>) => {
    setDraft({ ...draft, ...patch });
    setMessage(null);
  };

  const save = () => {
    setSaving(true);
    setMessage(null);
    native
      .updatePreferences(draft)
      .then((saved) => {
        load(saved);
        setMessage({ text: t.saved, error: false });
      })
      .catch((key) => setMessage({ text: errorMessage(t, key), error: true }))
      .finally(() => setSaving(false));
  };

  const chooseFolder = () => {
    native.chooseScreenshotFolder().then((folder) => {
      if (folder) update({ screenshotFolder: folder });
    });
  };

  const system = loaded.hotkeys.systemManaged;

  return (
    <div class="preferences">
      <h1>{t.preferences}</h1>

      <section class="pref-section">
        <h2>{t.shortcutsTitle}</h2>
        {system ? (
          <>
            <div class="pref-row">
              <span>{t.overlayShortcut}</span>
              <kbd>{loaded.hotkeys.toggle ?? t.notSet}</kbd>
            </div>
            <div class="pref-row">
              <span>{t.screenshotShortcut}</span>
              <kbd>{loaded.hotkeys.screenshot ?? t.notSet}</kbd>
            </div>
            <p class="muted small">{t.systemShortcutsHint}</p>
            <div class="actions">
              <button
                class="button default small"
                onClick={() =>
                  native.configureShortcuts().catch((key) => setMessage({ text: errorMessage(t, key), error: true }))
                }
              >
                {t.changeShortcuts}
              </button>
            </div>
          </>
        ) : (
          <>
            <div class="pref-row">
              <span>{t.overlayShortcut}</span>
              <ShortcutRecorder t={t} value={draft.hotkey} onChange={(hotkey) => update({ hotkey })} />
            </div>
            <div class="pref-row">
              <span>{t.screenshotShortcut}</span>
              <ShortcutRecorder
                t={t}
                value={draft.screenshotHotkey}
                onChange={(screenshotHotkey) => update({ screenshotHotkey })}
              />
            </div>
          </>
        )}
      </section>

      <section class="pref-section">
        <h2>{t.screenshotsTitle}</h2>
        <label class="checkbox">
          <input
            type="checkbox"
            checked={draft.saveScreenshots}
            onChange={(event) => update({ saveScreenshots: event.currentTarget.checked })}
          />
          {t.saveCopies}
        </label>
        {draft.saveScreenshots && (
          <div class="pref-row">
            <span class="muted small folder-path" title={draft.screenshotFolder ?? ""}>
              {t.folder}: {draft.screenshotFolder}
            </span>
            <button class="button default small" onClick={chooseFolder}>
              {t.chooseFolder}
            </button>
          </div>
        )}
      </section>

      <section class="pref-section">
        <h2>{t.startupTitle}</h2>
        <label class="checkbox">
          <input
            type="checkbox"
            checked={draft.startWithSystem}
            onChange={(event) => update({ startWithSystem: event.currentTarget.checked })}
          />
          {t.startWithSystem}
        </label>
      </section>

      <section class="pref-section">
        <h2>{t.languageTitle}</h2>
        <select
          class="select"
          value={draft.language ?? ""}
          aria-label={t.languageTitle}
          onChange={(event) => {
            const value = event.currentTarget.value;
            update({ language: value === "en" || value === "es" ? value : null });
          }}
        >
          <option value="">{t.languageSystem}</option>
          <option value="en">English</option>
          <option value="es">Español</option>
        </select>
      </section>

      {message && (
        <p class={message.error ? "error" : "success"} role="status">
          {message.text}
        </p>
      )}

      <div class="actions">
        <button data-primary class="button primary" disabled={saving} onClick={save}>
          {t.save}
        </button>
        <button class="button subtle" onClick={() => native.closePreferences()}>
          {t.close}
        </button>
      </div>
    </div>
  );
}

const MODIFIER_CODES = new Set([
  "ControlLeft", "ControlRight", "ShiftLeft", "ShiftRight", "AltLeft", "AltRight", "MetaLeft", "MetaRight",
]);

/** KeyboardEvent -> accelerator the Rust side parses, e.g. "Ctrl+Shift+G". */
function accelerator(event: KeyboardEvent): string | null {
  if (MODIFIER_CODES.has(event.code)) return null;
  const key = event.code.startsWith("Key")
    ? event.code.slice(3)
    : event.code.startsWith("Digit")
      ? event.code.slice(5)
      : event.code;
  const parts = [
    event.ctrlKey && "Ctrl",
    event.altKey && "Alt",
    event.shiftKey && "Shift",
    event.metaKey && "Super",
    key,
  ].filter(Boolean);
  return parts.join("+");
}

interface RecorderProps {
  t: Strings;
  value: string;
  onChange: (accelerator: string) => void;
}

function ShortcutRecorder({ t, value, onChange }: RecorderProps) {
  const [recording, setRecording] = useState(false);

  return (
    <button
      class={`shortcut ${recording ? "shortcut-recording" : ""}`}
      onClick={() => setRecording(true)}
      onBlur={() => setRecording(false)}
      onKeyDown={(event) => {
        if (!recording) return;
        // Keep Escape and the keys themselves from reaching the overlay.
        event.preventDefault();
        event.stopPropagation();
        if (event.key === "Escape") {
          setRecording(false);
          return;
        }
        const next = accelerator(event);
        if (next) {
          onChange(next);
          setRecording(false);
        }
      }}
    >
      {recording ? t.pressKeys : value || t.notSet}
    </button>
  );
}
