import { useEffect, useRef, useState } from "preact/hooks";

import { errorMessage, type Strings } from "./i18n";
import { native, type Game, type ScreenshotView } from "./native";

// Mirrors POST_MAX_CHARS in src-tauri/src/compose.rs and the API limit.
const MAX_CHARS = 500;

interface Props {
  t: Strings;
  /** The detected or chosen game; the user may detach it for this post. */
  game: Game | null;
  /** True while this tab is shown, to focus the text box. */
  active: boolean;
}

export function Composer({ t, game, active }: Props) {
  const [content, setContent] = useState("");
  const [spoiler, setSpoiler] = useState(false);
  const [withGame, setWithGame] = useState(true);
  const [screenshot, setScreenshot] = useState<ScreenshotView>({ state: "none" });
  const [posting, setPosting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [postedId, setPostedId] = useState<string | null>(null);
  const textarea = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    native.screenshot().then(setScreenshot);
    const unlisten = native.onScreenshot((view) => {
      setScreenshot(view);
      if (view.state === "ready") setPostedId(null);
    });
    return () => {
      unlisten.then((stop) => stop());
    };
  }, []);

  // A different game becomes attached again by default.
  useEffect(() => setWithGame(true), [game?.id]);

  useEffect(() => {
    if (active && !postedId) requestAnimationFrame(() => textarea.current?.focus());
  }, [active, postedId, screenshot.state]);

  const length = [...content.trim()].length;
  const hasScreenshot = screenshot.state === "ready";
  const attachedGame = withGame ? game : null;
  const canPost = !posting && length <= MAX_CHARS && (length > 0 || hasScreenshot);

  const submit = () => {
    if (!canPost) return;
    setPosting(true);
    setError(null);
    native
      .createPost({
        content,
        gameId: attachedGame?.id ?? null,
        spoiler,
        withScreenshot: hasScreenshot,
      })
      .then((id) => {
        setPostedId(id);
        setContent("");
        setSpoiler(false);
      })
      .catch((key) => setError(errorMessage(t, key)))
      .finally(() => setPosting(false));
  };

  if (postedId) {
    return (
      <div class="composer">
        <h1>{t.posted}</h1>
        <div class="actions">
          <button data-primary class="button primary" onClick={() => native.openPost(postedId)}>
            {t.viewPost}
          </button>
          <button class="button subtle" onClick={() => setPostedId(null)}>
            {t.newPost}
          </button>
        </div>
      </div>
    );
  }

  return (
    <div class="composer">
      {attachedGame && (
        <div class="game-chip">
          <span>{attachedGame.title}</span>
          <button class="chip-remove" aria-label={t.removeGame} title={t.removeGame} onClick={() => setWithGame(false)}>
            ×
          </button>
        </div>
      )}

      <textarea
        ref={textarea}
        class="composer-input"
        rows={3}
        value={content}
        placeholder={t.composerPlaceholder}
        aria-label={t.composerPlaceholder}
        onInput={(event) => setContent(event.currentTarget.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
            event.preventDefault();
            submit();
          }
        }}
      />

      <ScreenshotSlot t={t} screenshot={screenshot} />

      {error && (
        <p class="error" role="alert">
          {error}
        </p>
      )}

      <div class="composer-footer">
        <label class="checkbox">
          <input type="checkbox" checked={spoiler} onChange={(event) => setSpoiler(event.currentTarget.checked)} />
          {t.spoiler}
        </label>
        <span class={`counter ${length > MAX_CHARS ? "counter-over" : ""}`}>
          {length}/{MAX_CHARS}
        </span>
        <button data-primary class="button primary" disabled={!canPost} onClick={submit}>
          {posting ? t.posting : t.post}
        </button>
      </div>
    </div>
  );
}

function ScreenshotSlot({ t, screenshot }: { t: Strings; screenshot: ScreenshotView }) {
  switch (screenshot.state) {
    case "capturing":
      return <p class="muted small">{t.capturing}</p>;

    case "ready":
      return (
        <div class="screenshot">
          <img class="screenshot-preview" src={screenshot.preview} alt={t.screenshotAlt} />
          <div class="actions">
            <button class="button subtle small" onClick={() => native.takeScreenshot()}>
              {t.retakeScreenshot}
            </button>
            <button class="button subtle small" onClick={() => native.takeScreenshot(true)}>
              {t.chooseWindow}
            </button>
            <button class="button subtle small" onClick={() => native.discardScreenshot()}>
              {t.removeScreenshot}
            </button>
          </div>
          {screenshot.savedTo && <p class="muted small">{t.savedCopy(screenshot.savedTo)}</p>}
          {screenshot.saveFailed && <p class="error">{t.saveCopyFailed}</p>}
        </div>
      );

    case "failed":
    case "none":
      return (
        <div>
          {screenshot.state === "failed" && <p class="error">{t.screenshotFailed}</p>}
          <div class="actions">
            <button class="button default small" onClick={() => native.takeScreenshot()}>
              {t.addScreenshot}
            </button>
            <button class="button subtle small" onClick={() => native.takeScreenshot(true)}>
              {t.chooseWindow}
            </button>
          </div>
        </div>
      );
  }
}
