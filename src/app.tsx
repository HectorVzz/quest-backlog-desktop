import { useEffect, useRef, useState } from "preact/hooks";

import { GamePanel } from "./game-panel";
import { strings, type Strings } from "./i18n";
import { Icon, Mark } from "./icons";
import { native, type AppInfo, type AuthView } from "./native";
import { ToastProvider } from "./toast";

export function App() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [auth, setAuth] = useState<AuthView>({ state: "loading" });
  const [busy, setBusy] = useState(false);
  const [closing, setClosing] = useState(false);
  const [shownCount, setShownCount] = useState(0);
  const [composeCount, setComposeCount] = useState(0);
  // Counts openings; alternating between two identical animations
  // restarts the opening animation each time the panel is shown.
  const [openings, setOpenings] = useState(0);
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
        setClosing(false);
        setOpenings((count) => count + 1);
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

  // The X button: the first times, explain that the app stays in the tray.
  const close = () => (info.closeNoticeSeen ? native.hide() : setClosing(true));
  const confirmClose = (dontShowAgain: boolean) => {
    setClosing(false);
    if (dontShowAgain) {
      setInfo({ ...info, closeNoticeSeen: true });
      native.dismissCloseNotice().catch(() => {});
    }
    native.hide();
  };

  const dismissTips = () => {
    setInfo({ ...info, tipsSeen: true });
    native.dismissTips().catch(() => {});
  };

  return (
    <ToastProvider dismissLabel={t.close}>
      <main class="panel" data-motion={openings % 2 ? "a" : "b"} ref={panel}>
        <header class="topbar">
          <span class="wordmark">
            <Mark size={20} />
            Quest Backlog
          </span>
          <div class="topbar-actions">
            <button
              class="icon-button"
              aria-label={t.preferences}
              title={t.preferences}
              onClick={() => native.openPreferences()}
            >
              <Icon name="settings" />
            </button>
            <button class="icon-button" aria-label={t.close} title={`${t.close} (Esc)`} onClick={close}>
              <Icon name="close" />
            </button>
          </div>
        </header>

        <section class="panel-body">
          <Body
            t={t}
            info={info}
            auth={auth}
            busy={busy}
            run={run}
            shownCount={shownCount}
            composeCount={composeCount}
            onDismissTips={dismissTips}
          />
        </section>

        <footer class="panel-footer">
          {auth.state === "signedIn" ? (
            <span class="account">
              <span class="handle">@{auth.user.username}</span>
              <button class="link-button" disabled={busy} onClick={() => run(native.signOut)}>
                {t.signOut}
              </button>
            </span>
          ) : (
            <span />
          )}
          {info.hotkey.failed ? (
            <span class="footer-warning">{t.shortcutFailed}</span>
          ) : (
            <span class="version">v{info.version}</span>
          )}
        </footer>
        {closing && (
          <CloseNotice t={t} toggle={info.hotkey.toggle} onConfirm={confirmClose} onCancel={() => setClosing(false)} />
        )}
      </main>
    </ToastProvider>
  );
}

interface BodyProps {
  t: Strings;
  info: AppInfo;
  auth: AuthView;
  busy: boolean;
  run: (action: () => Promise<void>) => void;
  shownCount: number;
  composeCount: number;
  onDismissTips: () => void;
}

function Body({ t, info, auth, busy, run, shownCount, composeCount, onDismissTips }: BodyProps) {
  switch (auth.state) {
    case "loading":
      return <p class="muted intro">{t.connecting}</p>;

    case "signedOut":
      return (
        <div class="intro">
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
        </div>
      );

    case "pending":
      return (
        <div class="intro">
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
            <span class="muted small waiting">{t.waiting}</span>
          </div>
        </div>
      );

    case "signedIn":
      return (
        <GamePanel
          t={t}
          shownCount={shownCount}
          composeCount={composeCount}
          tips={info.tipsSeen ? null : <Tips t={t} info={info} onDismiss={onDismissTips} />}
        />
      );

    case "offline":
      return (
        <div class="intro">
          <h1>{t.offlineTitle}</h1>
          <p class="muted">{t.offlineBody}</p>
          <div class="actions">
            <button data-primary class="button primary" disabled={busy} onClick={() => run(native.retryAuth)}>
              {t.retry}
            </button>
          </div>
        </div>
      );
  }
}

/** Shown when the panel is closed with X, until "Don't show this again". */
function CloseNotice({
  t,
  toggle,
  onConfirm,
  onCancel,
}: {
  t: Strings;
  toggle: string | null;
  onConfirm: (dontShowAgain: boolean) => void;
  onCancel: () => void;
}) {
  const [dontShowAgain, setDontShowAgain] = useState(false);
  const confirm = useRef<HTMLButtonElement>(null);
  useEffect(() => confirm.current?.focus(), []);
  // Keys stay in the notice: Esc cancels it instead of closing the panel,
  // and the panel's own keys (1 to 6 set the status) do not fire behind it.
  const onKeyDown = (event: KeyboardEvent) => {
    event.stopPropagation();
    if (event.key === "Escape") onCancel();
  };
  return (
    <div class="modal-backdrop" onClick={(event) => event.target === event.currentTarget && onCancel()}>
      <div
        class="modal"
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="close-notice-title"
        aria-describedby="close-notice-body"
        onKeyDown={onKeyDown}
      >
        <h2 id="close-notice-title">{t.closeNoticeTitle}</h2>
        <p id="close-notice-body" class="muted">
          {toggle ? t.closeNoticeBody(toggle) : t.closeNoticeBodyNoShortcut}
        </p>
        <label class="checkbox">
          <input
            type="checkbox"
            checked={dontShowAgain}
            onChange={(event) => setDontShowAgain(event.currentTarget.checked)}
          />
          {t.dontShowAgain}
        </label>
        <div class="actions">
          <button ref={confirm} class="button primary" onClick={() => onConfirm(dontShowAgain)}>
            {t.closePanel}
          </button>
          <button class="button subtle" onClick={onCancel}>
            {t.cancel}
          </button>
        </div>
      </div>
    </div>
  );
}

/** First-run list of the panel's keys, until dismissed. */
function Tips({ t, info, onDismiss }: { t: Strings; info: AppInfo; onDismiss: () => void }) {
  const rows: [string, string][] = [
    ["1–6", t.tipStatusKeys],
    ...(info.hotkey.toggle ? [[info.hotkey.toggle, t.tipToggle] as [string, string]] : []),
    ...(info.hotkey.screenshot ? [[info.hotkey.screenshot, t.tipScreenshot] as [string, string]] : []),
    ["Esc", t.tipClose],
  ];
  return (
    <aside class="tips" aria-label={t.tipsTitle}>
      <div class="tips-header">
        <h2>{t.tipsTitle}</h2>
        <button class="link-button" onClick={onDismiss}>
          {t.gotIt}
        </button>
      </div>
      <dl class="tips-list">
        {rows.map(([keys, label]) => (
          <div class="tip" key={keys}>
            <dt>
              <kbd>{keys}</kbd>
            </dt>
            <dd>{label}</dd>
          </div>
        ))}
      </dl>
    </aside>
  );
}
