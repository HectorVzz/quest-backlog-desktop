import { useEffect, useRef, useState } from "preact/hooks";

import { GamePanel } from "./game-panel";
import { strings, type Strings } from "./i18n";
import { native, type AppInfo, type AuthView } from "./native";

export function App() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [auth, setAuth] = useState<AuthView>({ state: "loading" });
  const [busy, setBusy] = useState(false);
  const [shownCount, setShownCount] = useState(0);
  const [composeCount, setComposeCount] = useState(0);
  const panel = useRef<HTMLElement>(null);

  useEffect(() => {
    native.appInfo().then(setInfo);
    native.auth().then(setAuth);

    const focusPrimary = () =>
      panel.current?.querySelector<HTMLButtonElement>("button[data-primary]")?.focus();
    const unlisteners = [
      native.onAuthChanged((view) => {
        setAuth(view);
        setBusy(false);
      }),
      // The hotkey status can change after start-up (the Wayland portal
      // answers asynchronously), so refresh it each time the overlay opens.
      native.onShown(() => {
        native.appInfo().then(setInfo);
        setShownCount((count) => count + 1);
        requestAnimationFrame(focusPrimary);
      }),
      // A screenshot was taken: open the post composer with it.
      native.onCompose(() => setComposeCount((count) => count + 1)),
      // Language or shortcuts changed in Preferences.
      native.onSettingsChanged(() => native.appInfo().then(setInfo)),
    ];

    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") native.hide();
    };
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("keydown", onKey);
      unlisteners.forEach((unlisten) => unlisten.then((stop) => stop()));
    };
  }, []);

  useEffect(() => {
    if (info) document.documentElement.lang = info.language;
  }, [info]);

  if (!info) return null;
  const t = strings(info.language);

  const run = (action: () => Promise<void>) => {
    setBusy(true);
    action().finally(() => setBusy(false));
  };

  return (
    <main class="panel" ref={panel}>
      <header class="panel-header">
        <span class="wordmark">Quest Backlog</span>
        <div class="header-actions">
          <button
            class="icon-button"
            aria-label={t.preferences}
            title={t.preferences}
            onClick={() => native.openPreferences()}
          >
            <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
              <path
                fill="none"
                stroke="currentColor"
                stroke-width="1.8"
                stroke-linecap="round"
                d="M4 7h10M18 7h2M4 17h4M12 17h8M16 5v4M10 15v4"
              />
            </svg>
          </button>
          <button class="icon-button" aria-label={t.close} title={t.close} onClick={() => native.hide()}>
            ×
          </button>
        </div>
      </header>

      <section class="panel-body">
        <Body t={t} auth={auth} busy={busy} run={run} shownCount={shownCount} composeCount={composeCount} />
      </section>

      <footer class="panel-footer">
        <span>
          {info.hotkey.failed
            ? t.shortcutFailed
            : info.hotkey.toggle
              ? t.shortcutHint(info.hotkey.toggle)
              : t.shortcutPending}
        </span>
        <span class="version">v{info.version}</span>
      </footer>
    </main>
  );
}

interface BodyProps {
  t: Strings;
  auth: AuthView;
  busy: boolean;
  run: (action: () => Promise<void>) => void;
  shownCount: number;
  composeCount: number;
}

function Body({ t, auth, busy, run, shownCount, composeCount }: BodyProps) {
  switch (auth.state) {
    case "loading":
      return <p class="muted">{t.connecting}</p>;

    case "signedOut":
      return (
        <>
          <h1>{t.signInTitle}</h1>
          <p class="muted">{t.signInBody}</p>
          {auth.error && (
            <p class="error" role="alert">
              {t.errors[auth.error]}
            </p>
          )}
          <div class="actions">
            <button data-primary class="button primary" disabled={busy} onClick={() => run(native.startSignIn)}>
              {t.signIn}
            </button>
          </div>
        </>
      );

    case "pending":
      return (
        <>
          <h1>{t.pendingTitle}</h1>
          <p class="muted">{t.pendingBody}</p>
          <p class="code" aria-live="polite">
            {auth.userCode}
          </p>
          <div class="actions">
            <button data-primary class="button primary" onClick={() => native.openSignInPage()}>
              {t.openBrowser}
            </button>
            <button class="button subtle" onClick={() => native.cancelSignIn()}>
              {t.cancel}
            </button>
            <span class="muted small">{t.waiting}</span>
          </div>
        </>
      );

    case "signedIn":
      return (
        <>
          <GamePanel t={t} shownCount={shownCount} composeCount={composeCount} />
          <div class="account">
            <span class="muted small">
              {t.signedInAs} <span class="handle">@{auth.user.username}</span>
            </span>
            <button class="button subtle small" disabled={busy} onClick={() => run(native.signOut)}>
              {t.signOut}
            </button>
          </div>
        </>
      );

    case "offline":
      return (
        <>
          <h1>{t.offlineTitle}</h1>
          <p class="muted">{t.offlineBody}</p>
          <div class="actions">
            <button data-primary class="button primary" disabled={busy} onClick={() => run(native.retryAuth)}>
              {t.retry}
            </button>
          </div>
        </>
      );
  }
}
