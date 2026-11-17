import { useEffect, useRef, useState } from "preact/hooks";

import { errorMessage, type Strings } from "./i18n";
import { Icon } from "./icons";
import { native, type Game, type ScreenshotView } from "./native";
import { useToast } from "./toast";

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
  const textarea = useRef<HTMLTextAreaElement>(null);
  const toast = useToast();

  useEffect(() => {
    native.screenshot().then(setScreenshot);
    const unlisten = native.onScreenshot(setScreenshot);
    return () => {
      unlisten.then((stop) => stop());
    };
  }, []);

  // A different game becomes attached again by default.
  useEffect(() => setWithGame(true), [game?.id]);

  useEffect(() => {
    if (active) requestAnimationFrame(() => textarea.current?.focus());
  }, [active, screenshot.state]);

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
        setContent("");
        setSpoiler(false);
        toast({ text: t.posted, action: { label: t.viewPost, run: () => native.openPost(id) } });
      })
      .catch((key) => setError(errorMessage(t, key)))
      .finally(() => setPosting(false));
  };

  return (
    <div class="composer">
      {attachedGame && (
        <div class="game-chip">
          <span>{attachedGame.title}</span>
          <button class="chip-remove" aria-label={t.removeGame} title={t.removeGame} onClick={() => setWithGame(false)}>
            <Icon name="close" size={12} />
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
        <button
          data-primary
          class="button primary"
          disabled={!canPost}
          onClick={submit}
          aria-keyshortcuts="Control+Enter"
          title="Ctrl+Enter"
        >
          {posting ? t.posting : t.post}
        </button>
      </div>
    </div>
  );
}

function ScreenshotSlot({ t, screenshot }: { t: Strings; screenshot: ScreenshotView }) {
  switch (screenshot.state) {
    case "capturing":
      return (
        <div class="screenshot">
          <div class="screenshot-placeholder" aria-busy="true">
            <span class="muted small">{t.capturing}</span>
          </div>
        </div>
      );

    case "ready":
      return (
        <div class="screenshot">
          <img class="screenshot-preview" src={screenshot.preview} alt={t.screenshotAlt} />
          <div class="screenshot-actions">
            <button class="button subtle small" onClick={() => native.takeScreenshot()}>
              <Icon name="camera" size={16} />
              {t.retakeScreenshot}
            </button>
            <button class="button subtle small" onClick={() => native.takeScreenshot(true)}>
              <Icon name="window" size={16} />
              {t.chooseWindow}
            </button>
            <button class="button subtle small" onClick={() => native.discardScreenshot()}>
              <Icon name="trash" size={16} />
              {t.removeScreenshot}
            </button>
            {screenshot.savedTo && <p class="muted small saved-copy">{t.savedCopy(screenshot.savedTo)}</p>}
            {screenshot.saveFailed && <p class="error small">{t.saveCopyFailed}</p>}
          </div>
        </div>
      );

    case "failed":
    case "none":
      return (
        <div class="screenshot-empty">
          {screenshot.state === "failed" && <p class="error">{t.screenshotFailed}</p>}
          <div class="actions">
            <button class="button default small" onClick={() => native.takeScreenshot()}>
              <Icon name="camera" size={16} />
              {t.addScreenshot}
            </button>
            <button class="button subtle small" onClick={() => native.takeScreenshot(true)}>
              <Icon name="window" size={16} />
              {t.chooseWindow}
            </button>
          </div>
        </div>
      );
  }
}
