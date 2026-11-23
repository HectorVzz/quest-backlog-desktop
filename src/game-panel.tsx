import type { ComponentChildren } from "preact";
import { useEffect, useRef, useState } from "preact/hooks";

import { Composer } from "./composer";
import { FeedTab } from "./feed";
import { errorMessage, type StatusKey, type Strings } from "./i18n";
import { Icon } from "./icons";
import { native, type Game, type Progress } from "./native";
import { NotificationsTab } from "./notifications";
import { useToast } from "./toast";

/** Status update posts share the API's 500-character post limit. */
const MAX_NOTE_CHARS = 500;

/** In key order: 1 is Playing, 6 is Wishlist. */
const STATUSES: StatusKey[] = ["playing", "completed", "platinumed", "backlog", "dropped", "wishlist"];

/** Finishing a game offers to rate it. */
const RATE_STATUSES: StatusKey[] = ["completed", "platinumed"];
/** Mirrors REVIEW_MAX_LEN in src-tauri/src/games.rs and the API limit (UTF-16 units). */
const MAX_REVIEW_CHARS = 2000;
/** The API's rating scale, one star per point. */
const MAX_RATING = 10;

type View =
  | { mode: "detecting" }
  | { mode: "game"; game: Game; progress: Progress }
  | { mode: "search"; reason: "unknown" | "nothing" | "change"; executable: string | null; query: string };

interface Props {
  t: Strings;
  /** Changes every time the overlay is shown, to detect again. */
  shownCount: number;
  /** Changes when a screenshot was taken, to switch to the Post tab. */
  composeCount: number;
  /** First-run tips, shown under the status buttons until dismissed. */
  tips: ComponentChildren;
}

type Tab = "status" | "post" | "feed" | "mine" | "notifications";

export function GamePanel({ t, shownCount, composeCount, tips }: Props) {
  const [tab, setTab] = useState<Tab>("status");
  const [sharing, setSharing] = useState(false);
  const [reviewing, setReviewing] = useState(false);
  // Bumped by "Post the first one" so the composer attaches the game again.
  const [attachCount, setAttachCount] = useState(0);
  const [view, setView] = useState<View>({ mode: "detecting" });
  const [previous, setPrevious] = useState<View | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [unread, setUnread] = useState(0);

  useEffect(() => {
    if (composeCount > 0) setTab("post");
  }, [composeCount]);

  // The unread count is asked for each time the panel is shown, never in
  // the background. Without it the bell just shows no badge.
  useEffect(() => {
    native
      .unreadNotifications()
      .then(setUnread)
      .catch(() => {});
  }, [shownCount]);

  const detect = () => {
    setView({ mode: "detecting" });
    setPrevious(null);
    setError(null);
    native
      .detectGame()
      .then((detection) => {
        if (detection.kind === "game") {
          setView({ mode: "game", game: detection.game, progress: detection.progress });
        } else if (detection.kind === "unknown") {
          setView({ mode: "search", reason: "unknown", executable: detection.executable, query: detection.query });
        } else {
          setView({ mode: "search", reason: "nothing", executable: null, query: "" });
        }
      })
      .catch((key) => {
        setError(errorMessage(t, key));
        setView({ mode: "search", reason: "nothing", executable: null, query: "" });
      });
  };

  useEffect(detect, [shownCount]);

  // The share and review cards belong to one game.
  const gameId = view.mode === "game" ? view.game.id : null;
  useEffect(() => {
    setSharing(false);
    setReviewing(false);
  }, [gameId]);

  // One card at a time under the status buttons.
  const share = () => {
    setReviewing(false);
    setSharing(true);
  };
  const review = () => {
    setSharing(false);
    setReviewing(true);
  };
  const composeAboutGame = () => {
    setAttachCount((count) => count + 1);
    setTab("post");
  };

  const choose = (game: Game) => {
    setError(null);
    native
      .chooseGame(game)
      .then((chosen) => {
        setPrevious(null);
        setView({ mode: "game", game: chosen.game, progress: chosen.progress });
      })
      .catch((key) => setError(errorMessage(t, key)));
  };

  const change = () => {
    if (view.mode !== "game") return;
    setPrevious(view);
    setTab("status");
    setView({ mode: "search", reason: "change", executable: null, query: view.game.title });
  };

  // While searching for a different game, posts keep the one shown before.
  const currentGame =
    view.mode === "game" ? view.game : previous?.mode === "game" ? previous.game : null;

  const reading = tab === "feed" || tab === "mine" || tab === "notifications";

  const tabs: { id: Tab; label: string }[] = [
    { id: "status", label: t.tabStatus },
    { id: "post", label: t.tabPost },
    { id: "feed", label: t.tabFeed },
    { id: "mine", label: t.tabMyPosts },
  ];

  return (
    <>
      <Backdrop game={view.mode === "game" && !reading ? view.game : null} />
      {!reading && <Hero t={t} view={view} compact={tab === "post" || sharing || reviewing} onChange={change} />}

      {/* While reading posts the game header gives way and the tabs stay on top. */}
      <div class="tabs" role="tablist" data-sticky={reading}>
        {tabs.map(({ id, label }) => (
          <button key={id} role="tab" class="tab" aria-selected={tab === id} onClick={() => setTab(id)}>
            {label}
          </button>
        ))}
        {/* An icon tab at the end: the row has no room for a fifth label. */}
        <button
          role="tab"
          class="tab tab-icon"
          aria-selected={tab === "notifications"}
          aria-label={unread > 0 ? t.notificationsUnread(unread) : t.tabNotifications}
          title={t.tabNotifications}
          onClick={() => setTab("notifications")}
        >
          <Icon name="bell" size={18} />
          {unread > 0 && (
            <span class="badge tabular" aria-hidden="true">
              {unread > 9 ? "9+" : unread}
            </span>
          )}
        </button>
      </div>

      {/* Both stay mounted so a post draft survives switching tabs. */}
      <div class="tab-panel" role="tabpanel" hidden={tab !== "status"}>
        {view.mode === "game" ? (
          <>
            <StatusPicker
              t={t}
              game={view.game}
              status={view.progress.status}
              active={tab === "status" && !sharing && !reviewing}
              onSaved={(progress) => setView({ ...view, progress })}
              onShare={share}
              onRate={review}
            />
            <ProgressRow
              t={t}
              game={view.game}
              progress={view.progress}
              sharing={sharing || reviewing}
              onSaved={(progress) => setView({ ...view, progress })}
              onShare={share}
              onRate={review}
            />
            {view.progress.status === "playing" && (
              <LookingToggle t={t} game={view.game} shownCount={shownCount} />
            )}
            {sharing && view.progress.status && (
              <ShareCard
                key={view.progress.status}
                t={t}
                game={view.game}
                status={view.progress.status}
                hoursPlayed={view.progress.hoursPlayed}
                onClose={() => setSharing(false)}
              />
            )}
            {reviewing && (
              <ReviewCard
                t={t}
                game={view.game}
                progress={view.progress}
                onSaved={(progress) => setView({ ...view, progress })}
                onClose={() => setReviewing(false)}
              />
            )}
          </>
        ) : view.mode === "search" ? (
          <>
            {error && (
              <p class="error" role="alert">
                {error}
              </p>
            )}
            <GameSearch key={`${view.reason}:${view.query}`} t={t} initialQuery={view.query} onChoose={choose} />
            <div class="actions">
              {previous ? (
                <button class="button subtle" onClick={() => setView(previous)}>
                  <Icon name="back" size={16} />
                  {t.backToGame}
                </button>
              ) : (
                <button class="button subtle" onClick={detect}>
                  <Icon name="refresh" size={16} />
                  {t.detectAgain}
                </button>
              )}
            </div>
          </>
        ) : null}
        {tips}
      </div>
      <div class="tab-panel" role="tabpanel" hidden={tab !== "post"}>
        <Composer t={t} game={currentGame} active={tab === "post"} attachCount={attachCount} />
      </div>
      {/* Always mounted so a loaded list survives switching tabs; each loads when opened. */}
      <div class="tab-panel" role="tabpanel" hidden={tab !== "feed"}>
        <FeedTab
          t={t}
          source="feed"
          active={tab === "feed"}
          shownCount={shownCount}
          game={currentGame}
          onCompose={composeAboutGame}
        />
      </div>
      <div class="tab-panel" role="tabpanel" hidden={tab !== "mine"}>
        <FeedTab t={t} source="mine" active={tab === "mine"} shownCount={shownCount} />
      </div>
      <div class="tab-panel" role="tabpanel" hidden={tab !== "notifications"}>
        <NotificationsTab t={t} active={tab === "notifications"} shownCount={shownCount} onUnread={setUnread} />
      </div>
    </>
  );
}

/** The game's cover, blurred and faded into the page behind the header. */
function Backdrop({ game }: { game: Game | null }) {
  return (
    <div class="backdrop" aria-hidden="true">
      {game?.coverImageUrl && <img key={game.id} class="backdrop-image" src={game.coverImageUrl} alt="" />}
    </div>
  );
}

function releaseYear(game: Game) {
  return game.releaseDate ? new Date(game.releaseDate).getUTCFullYear() : null;
}

function Cover({ game, size }: { game: Game; size: "small" | "large" }) {
  return game.coverImageUrl ? (
    <img class={`cover cover-${size}`} src={game.coverImageUrl} alt="" />
  ) : (
    <span class={`cover cover-${size} cover-empty`} aria-hidden="true" />
  );
}

interface HeroProps {
  t: Strings;
  view: View;
  /** The Post tab or the share card is shown: a smaller header leaves room. */
  compact: boolean;
  onChange: () => void;
}

function Hero({ t, view, compact, onChange }: HeroProps) {
  if (view.mode === "detecting") {
    return (
      <div class="hero" aria-busy="true">
        <span class="cover cover-large cover-loading" aria-hidden="true" />
        <div class="hero-text">
          <p class="muted">{t.detecting}</p>
        </div>
      </div>
    );
  }

  if (view.mode === "search") {
    const heading =
      view.reason === "unknown" && view.executable
        ? t.whichGame(view.executable)
        : view.reason === "nothing"
          ? t.noGame
          : t.chooseGame;
    const hint = view.reason === "unknown" ? t.whichGameHint : view.reason === "nothing" ? t.noGameHint : null;
    return (
      <div class="hero hero-plain">
        <div class="hero-text">
          <h1>{heading}</h1>
          {hint && <p class="muted">{hint}</p>}
        </div>
      </div>
    );
  }

  const year = releaseYear(view.game);
  const { status, hoursPlayed } = view.progress;
  return (
    <div class="hero" key={view.game.id} data-compact={compact}>
      <Cover game={view.game} size="large" />
      <div class="hero-text">
        <h1 class="game-title">{view.game.title}</h1>
        <p class="meta">
          {year && <span class="tabular">{year}</span>}
          {status ? (
            <span class="status-label" style={`--status: var(--qb-status-${status})`}>
              <span class="status-dot" aria-hidden="true" />
              {t.statuses[status]}
            </span>
          ) : (
            <span>{t.notTracked}</span>
          )}
          {status && hoursPlayed !== null && <span class="tabular">{t.hours(formatHours(t, hoursPlayed))}</span>}
        </p>
        <button class="link-button hero-change" onClick={onChange}>
          {t.changeGame}
        </button>
      </div>
    </div>
  );
}

interface PickerProps {
  t: Strings;
  game: Game;
  status: StatusKey | null;
  /** The Status tab is shown, so the number keys pick a status. */
  active: boolean;
  onSaved: (progress: Progress) => void;
  /** Opens the share card, offered in the "Saved as" toast. */
  onShare: () => void;
  /** Opens the review card, offered too when the game is finished. */
  onRate: () => void;
}

function StatusPicker({ t, game, status, active, onSaved, onShare, onRate }: PickerProps) {
  const [saving, setSaving] = useState<StatusKey | null>(null);
  const toast = useToast();
  // The key handler is registered once per activation; read state through a ref.
  const latest = useRef({ status, saving });
  latest.current = { status, saving };

  const save = (next: StatusKey) => {
    if (latest.current.saving || next === latest.current.status) return;
    setSaving(next);
    native
      .setGameStatus(game.id, next)
      .then((saved) => {
        onSaved(saved);
        if (saved.status) {
          toast({
            text: t.savedAs(t.statuses[saved.status]),
            action: { label: t.shareStatus, run: onShare },
            secondary: RATE_STATUSES.includes(saved.status) ? { label: t.rateIt, run: onRate } : undefined,
          });
        }
      })
      .catch((key) => toast({ text: errorMessage(t, key), tone: "error" }))
      .finally(() => setSaving(null));
  };

  useEffect(() => {
    if (!active) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.ctrlKey || event.altKey || event.metaKey || event.repeat) return;
      const target = event.target;
      if (target instanceof Element && target.closest("input, textarea, select, [contenteditable]")) return;
      const index = Number(event.key) - 1;
      if (Number.isInteger(index) && index >= 0 && index < STATUSES.length) {
        event.preventDefault();
        save(STATUSES[index]);
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [active, game.id]);

  // Shown as chosen while it saves; reverts if saving fails.
  const shown = saving ?? status;

  return (
    <div class="status-grid" role="group" aria-label={t.tabStatus} aria-busy={saving !== null}>
      {STATUSES.map((value, index) => (
        <button
          key={value}
          class="status"
          style={`--status: var(--qb-status-${value})`}
          aria-pressed={value === shown}
          aria-keyshortcuts={String(index + 1)}
          data-saving={saving === value ? true : undefined}
          data-primary={value === "playing" ? true : undefined}
          disabled={saving !== null && saving !== value}
          onClick={() => save(value)}
        >
          <span class="status-icon">
            <Icon name={value} size={18} />
          </span>
          <span class="status-name">{t.statuses[value]}</span>
          <kbd class="status-key" aria-hidden="true">
            {index + 1}
          </kbd>
        </button>
      ))}
    </div>
  );
}

export function formatHours(t: Strings, hours: number) {
  return new Intl.NumberFormat(t.locale, { maximumFractionDigits: 1 }).format(hours);
}

interface ProgressProps {
  t: Strings;
  game: Game;
  progress: Progress;
  /** A share or review card is open. */
  sharing: boolean;
  onSaved: (progress: Progress) => void;
  onShare: () => void;
  onRate: () => void;
}

/** Hours played, saved on Enter or when the field loses focus, "Post update" and "Rate it". */
function ProgressRow({ t, game, progress, sharing, onSaved, onShare, onRate }: ProgressProps) {
  const saved = progress.hoursPlayed === null ? "" : String(progress.hoursPlayed);
  const [draft, setDraft] = useState(saved);
  const [busy, setBusy] = useState(false);
  const toast = useToast();

  useEffect(() => setDraft(saved), [game.id, saved]);

  const editable = progress.status !== null && !progress.steamHours;
  const hasReview = progress.rating !== null || progress.review !== null;
  const hint = progress.steamHours ? t.hoursFromSteam : progress.status === null ? t.hoursNeedStatus : null;

  const save = () => {
    const text = draft.trim().replace(",", ".");
    // Leaving the field empty keeps the saved hours.
    if (!editable || busy || text === "" || text === saved) {
      setDraft(saved);
      return;
    }
    const hours = Number(text);
    if (!Number.isFinite(hours)) {
      toast({ text: t.errors.hoursInvalid, tone: "error" });
      setDraft(saved);
      return;
    }
    setBusy(true);
    native
      .setGameStatus(game.id, progress.status!, hours)
      .then((next) => {
        onSaved(next);
        if (next.hoursPlayed !== null) toast({ text: t.hoursSaved(formatHours(t, next.hoursPlayed)) });
      })
      .catch((key) => {
        setDraft(saved);
        toast({ text: errorMessage(t, key), tone: "error" });
      })
      .finally(() => setBusy(false));
  };

  return (
    <div class="progress-row">
      <label class="hours-field">
        <span class="hours-label">{t.hoursPlayed}</span>
        <input
          class="number-input tabular"
          type="text"
          inputMode="decimal"
          value={draft}
          placeholder="0"
          disabled={!editable || busy}
          aria-describedby={hint ? "hours-hint" : undefined}
          onInput={(event) => setDraft(event.currentTarget.value)}
          onBlur={save}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.preventDefault();
              event.currentTarget.blur();
            }
          }}
        />
        {hint && (
          <span id="hours-hint" class="muted small">
            {hint}
          </span>
        )}
      </label>
      {!sharing && (
        <div class="progress-actions">
          {(hasReview || (progress.status && RATE_STATUSES.includes(progress.status))) && (
            <button
              class="button subtle small"
              aria-label={
                progress.rating !== null ? `${t.editReview}: ${t.ratingValue(formatHours(t, progress.rating))}` : undefined
              }
              title={hasReview ? t.editReview : undefined}
              onClick={onRate}
            >
              <Icon name="star" size={16} filled={progress.rating !== null} />
              {progress.rating !== null
                ? t.ratingValue(formatHours(t, progress.rating))
                : hasReview
                  ? t.editReview
                  : t.rateIt}
            </button>
          )}
          <button class="button subtle small" disabled={progress.status === null} onClick={onShare}>
            <Icon name="arrow" size={16} />
            {t.shareStatus}
          </button>
        </div>
      )}
    </div>
  );
}

interface ShareProps {
  t: Strings;
  game: Game;
  status: StatusKey;
  hoursPlayed: number | null;
  onClose: () => void;
}

/** Posts the current status to the feed, with an editable note. */
function ShareCard({ t, game, status, hoursPlayed, onClose }: ShareProps) {
  const [note, setNote] = useState(t.shareDefaults[status]);
  const [posting, setPosting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const textarea = useRef<HTMLTextAreaElement>(null);
  const toast = useToast();

  useEffect(() => {
    textarea.current?.focus();
    textarea.current?.select();
  }, []);

  const length = [...note.trim()].length;
  const canPost = !posting && length > 0 && length <= MAX_NOTE_CHARS;

  const submit = () => {
    if (!canPost) return;
    setPosting(true);
    setError(null);
    native
      .shareStatus(game.id, status, note, hoursPlayed)
      .then((id) => {
        onClose();
        toast({ text: t.shared, action: { label: t.viewPost, run: () => native.openPost(id) } });
      })
      .catch((key) => setError(errorMessage(t, key)))
      .finally(() => setPosting(false));
  };

  return (
    <div class="share" style={`--status: var(--qb-status-${status})`}>
      <h2 class="share-title">
        <span class="status-dot" aria-hidden="true" />
        {t.shareTitle(t.statuses[status])}
      </h2>
      <textarea
        ref={textarea}
        class="composer-input"
        rows={2}
        value={note}
        aria-label={t.shareTitle(t.statuses[status])}
        onInput={(event) => setNote(event.currentTarget.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
            event.preventDefault();
            submit();
          }
        }}
      />
      {error && (
        <p class="error" role="alert">
          {error}
        </p>
      )}
      <div class="actions">
        <button class="button primary" disabled={!canPost} onClick={submit} title="Ctrl+Enter">
          {posting ? t.posting : t.post}
        </button>
        <button class="button subtle" disabled={posting} onClick={onClose}>
          {t.cancel}
        </button>
        <span class={`counter share-counter ${length > MAX_NOTE_CHARS ? "counter-over" : ""}`}>
          {length}/{MAX_NOTE_CHARS}
        </span>
      </div>
    </div>
  );
}

/**
 * "Looking for players" for a game the viewer is playing, like the toggle
 * on the website's backlog. Read when shown; the server ends it when the
 * game leaves Playing.
 */
function LookingToggle({ t, game, shownCount }: { t: Strings; game: Game; shownCount: number }) {
  const [looking, setLooking] = useState<boolean | null>(null);
  const [busy, setBusy] = useState(false);
  const toast = useToast();

  useEffect(() => {
    let current = true;
    setLooking(null);
    native
      .lookingForPlayers(game.id)
      .then((value) => current && setLooking(value))
      // Unknown state: leave the toggle out rather than show a wrong one.
      .catch(() => current && setLooking(null));
    return () => {
      current = false;
    };
  }, [game.id, shownCount]);

  if (looking === null) return null;

  const toggle = (next: boolean) => {
    if (busy) return;
    setBusy(true);
    setLooking(next);
    native
      .setLookingForPlayers(game.id, next)
      .then((value) => {
        setLooking(value);
        toast({ text: value ? t.lookingOn(game.title) : t.lookingOff(game.title) });
      })
      .catch((key) => {
        setLooking(!next);
        toast({ text: errorMessage(t, key), tone: "error" });
      })
      .finally(() => setBusy(false));
  };

  return (
    <div class="looking">
      <label class="checkbox">
        <input
          type="checkbox"
          role="switch"
          checked={looking}
          disabled={busy}
          aria-describedby="looking-hint"
          onChange={(event) => toggle(event.currentTarget.checked)}
        />
        {t.lookingForPlayers}
      </label>
      <span id="looking-hint" class="muted small">
        {t.lookingHint}
      </span>
    </div>
  );
}

interface ReviewProps {
  t: Strings;
  game: Game;
  progress: Progress;
  onSaved: (progress: Progress) => void;
  onClose: () => void;
}

/** The viewer's rating (0 to 10) and optional review, saved on the game entry or posted. */
function ReviewCard({ t, game, progress, onSaved, onClose }: ReviewProps) {
  const [rating, setRating] = useState(progress.rating);
  const [text, setText] = useState(progress.review ?? "");
  const [busy, setBusy] = useState<"save" | "post" | null>(null);
  const [error, setError] = useState<string | null>(null);
  const stars = useRef<HTMLDivElement>(null);
  const toast = useToast();

  useEffect(() => {
    stars.current?.querySelector<HTMLButtonElement>('[tabindex="0"]')?.focus();
  }, []);

  // The server counts UTF-16 units, as `length` does.
  const length = text.trim().length;
  const changed = rating !== progress.rating || text.trim() !== (progress.review ?? "").trim();
  const canSave = busy === null && length <= MAX_REVIEW_CHARS && changed;
  const canPost = busy === null && length > 0 && length <= MAX_REVIEW_CHARS;

  const submit = (post: boolean) => {
    if (post ? !canPost : !canSave) return;
    setBusy(post ? "post" : "save");
    setError(null);
    native
      .saveReview(game.id, rating, text, post, progress.hoursPlayed)
      .then((saved) => {
        onSaved({ ...progress, rating: saved.rating, review: saved.review });
        onClose();
        const postId = saved.postId;
        if (post && postId) {
          toast({ text: t.reviewPosted, action: { label: t.viewPost, run: () => native.openPost(postId) } });
        } else {
          toast({ text: saved.rating === null && !saved.review ? t.reviewRemoved : t.reviewSaved });
        }
      })
      .catch((key) => setError(errorMessage(t, key)))
      .finally(() => setBusy(null));
  };

  return (
    <div class="share review">
      <h2 class="share-title">{t.reviewTitle(game.title)}</h2>
      <div class="rating-row">
        <StarRating t={t} value={rating} onChange={setRating} containerRef={stars} />
        <span class="muted small tabular" aria-live="polite">
          {rating === null ? t.ratingNone : t.ratingValue(formatHours(t, rating))}
        </span>
        {rating !== null && (
          <button class="link-button small" onClick={() => setRating(null)}>
            {t.clearRating}
          </button>
        )}
      </div>
      <textarea
        class="composer-input"
        rows={2}
        value={text}
        placeholder={t.reviewPlaceholder}
        aria-label={t.reviewLabel}
        aria-describedby="review-hint"
        onInput={(event) => setText(event.currentTarget.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
            event.preventDefault();
            submit(false);
          }
        }}
      />
      <p id="review-hint" class="muted small">
        {t.reviewHint}
      </p>
      {error && (
        <p class="error" role="alert">
          {error}
        </p>
      )}
      <div class="actions">
        <button class="button primary" disabled={!canSave} onClick={() => submit(false)} title="Ctrl+Enter">
          {busy === "save" ? t.saving : t.save}
        </button>
        <button class="button default" disabled={!canPost} onClick={() => submit(true)}>
          {busy === "post" ? t.posting : t.saveAndPost}
        </button>
        <button class="button subtle" disabled={busy !== null} onClick={onClose}>
          {t.cancel}
        </button>
        <span class={`counter share-counter ${length > MAX_REVIEW_CHARS ? "counter-over" : ""}`}>
          {length}/{MAX_REVIEW_CHARS}
        </span>
      </div>
    </div>
  );
}

interface StarProps {
  t: Strings;
  value: number | null;
  onChange: (value: number) => void;
  containerRef: { current: HTMLDivElement | null };
}

/** Ten stars as a radio group: arrow keys, Home and End move and choose. */
function StarRating({ t, value, onChange, containerRef }: StarProps) {
  const [hover, setHover] = useState<number | null>(null);
  // A rating saved on the website may be a half point; show the stars up to it.
  const chosen = value === null ? 0 : Math.floor(value);
  const shown = hover ?? chosen;
  const focusable = chosen || 1;

  const choose = (next: number) => {
    const clamped = Math.min(MAX_RATING, Math.max(1, next));
    onChange(clamped);
    requestAnimationFrame(() =>
      containerRef.current?.querySelector<HTMLButtonElement>(`[data-value="${clamped}"]`)?.focus(),
    );
  };

  const onKeyDown = (event: KeyboardEvent) => {
    const moves: Record<string, number> = {
      ArrowRight: chosen + 1,
      ArrowUp: chosen + 1,
      ArrowLeft: chosen - 1,
      ArrowDown: chosen - 1,
      Home: 1,
      End: MAX_RATING,
    };
    if (event.key in moves) {
      event.preventDefault();
      choose(moves[event.key]);
    }
  };

  return (
    <div
      ref={containerRef}
      class="stars"
      role="radiogroup"
      aria-label={t.rating}
      onKeyDown={onKeyDown}
      onMouseLeave={() => setHover(null)}
    >
      {Array.from({ length: MAX_RATING }, (_, index) => index + 1).map((star) => (
        <button
          key={star}
          type="button"
          class="star"
          role="radio"
          data-value={star}
          data-on={star <= shown}
          aria-checked={star === chosen}
          aria-label={t.ratingStar(star)}
          tabIndex={star === focusable ? 0 : -1}
          onMouseEnter={() => setHover(star)}
          onClick={() => choose(star)}
        >
          <Icon name="star" size={18} filled={star <= shown} />
        </button>
      ))}
    </div>
  );
}

interface SearchProps {
  t: Strings;
  initialQuery: string;
  onChoose: (game: Game) => void;
}

function GameSearch({ t, initialQuery, onChoose }: SearchProps) {
  const [query, setQuery] = useState(initialQuery);
  const [results, setResults] = useState<Game[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const input = useRef<HTMLInputElement>(null);
  const latest = useRef(0);

  useEffect(() => {
    input.current?.focus();
    input.current?.select();
  }, []);

  useEffect(() => {
    const trimmed = query.trim();
    if (trimmed.length < 2) {
      setResults(null);
      return;
    }
    const request = ++latest.current;
    const timer = setTimeout(() => {
      setLoading(true);
      setError(null);
      native
        .searchGames(trimmed)
        .then((games) => request === latest.current && setResults(games))
        .catch((key) => request === latest.current && setError(errorMessage(t, key)))
        .finally(() => request === latest.current && setLoading(false));
    }, 350);
    return () => clearTimeout(timer);
  }, [query]);

  return (
    <div class="search">
      <label class="search-field">
        <Icon name="search" size={16} />
        <input
          ref={input}
          class="search-input"
          type="search"
          value={query}
          placeholder={t.searchPlaceholder}
          aria-label={t.searchPlaceholder}
          onInput={(event) => setQuery(event.currentTarget.value)}
        />
      </label>
      {loading && <p class="muted small">{t.searching}</p>}
      {error && (
        <p class="error" role="alert">
          {error}
        </p>
      )}
      {!loading && results?.length === 0 && <p class="muted small">{t.noResults}</p>}
      {results && results.length > 0 && (
        <ul class="results">
          {results.map((game) => (
            <li key={game.id}>
              <button class="result" onClick={() => onChoose(game)}>
                <Cover game={game} size="small" />
                <span class="result-title">{game.title}</span>
                <span class="muted small tabular">{releaseYear(game)}</span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
