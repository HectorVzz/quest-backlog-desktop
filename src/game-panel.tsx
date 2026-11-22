import type { ComponentChildren } from "preact";
import { useEffect, useRef, useState } from "preact/hooks";

import { Composer } from "./composer";
import { FeedTab } from "./feed";
import { errorMessage, type StatusKey, type Strings } from "./i18n";
import { Icon } from "./icons";
import { native, type Game, type Progress } from "./native";
import { useToast } from "./toast";

/** Status update posts share the API's 500-character post limit. */
const MAX_NOTE_CHARS = 500;

/** In key order: 1 is Playing, 6 is Wishlist. */
const STATUSES: StatusKey[] = ["playing", "completed", "platinumed", "backlog", "dropped", "wishlist"];

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

type Tab = "status" | "post" | "feed" | "mine";

export function GamePanel({ t, shownCount, composeCount, tips }: Props) {
  const [tab, setTab] = useState<Tab>("status");
  const [sharing, setSharing] = useState(false);
  const [view, setView] = useState<View>({ mode: "detecting" });
  const [previous, setPrevious] = useState<View | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (composeCount > 0) setTab("post");
  }, [composeCount]);

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

  // The share card belongs to one game.
  const gameId = view.mode === "game" ? view.game.id : null;
  useEffect(() => setSharing(false), [gameId]);

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

  const reading = tab === "feed" || tab === "mine";

  const tabs: { id: Tab; label: string }[] = [
    { id: "status", label: t.tabStatus },
    { id: "post", label: t.tabPost },
    { id: "feed", label: t.tabFeed },
    { id: "mine", label: t.tabMyPosts },
  ];

  return (
    <>
      <Backdrop game={view.mode === "game" && !reading ? view.game : null} />
      {!reading && <Hero t={t} view={view} compact={tab === "post" || sharing} onChange={change} />}

      {/* While reading posts the game header gives way and the tabs stay on top. */}
      <div class="tabs" role="tablist" data-sticky={reading}>
        {tabs.map(({ id, label }) => (
          <button key={id} role="tab" class="tab" aria-selected={tab === id} onClick={() => setTab(id)}>
            {label}
          </button>
        ))}
      </div>

      {/* Both stay mounted so a post draft survives switching tabs. */}
      <div class="tab-panel" role="tabpanel" hidden={tab !== "status"}>
        {view.mode === "game" ? (
          <>
            <StatusPicker
              t={t}
              game={view.game}
              status={view.progress.status}
              active={tab === "status" && !sharing}
              onSaved={(progress) => setView({ ...view, progress })}
              onShare={() => setSharing(true)}
            />
            <ProgressRow
              t={t}
              game={view.game}
              progress={view.progress}
              sharing={sharing}
              onSaved={(progress) => setView({ ...view, progress })}
              onShare={() => setSharing(true)}
            />
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
        <Composer t={t} game={currentGame} active={tab === "post"} />
      </div>
      {/* Always mounted so a loaded list survives switching tabs; each loads when opened. */}
      <div class="tab-panel" role="tabpanel" hidden={tab !== "feed"}>
        <FeedTab t={t} source="feed" active={tab === "feed"} shownCount={shownCount} />
      </div>
      <div class="tab-panel" role="tabpanel" hidden={tab !== "mine"}>
        <FeedTab t={t} source="mine" active={tab === "mine"} shownCount={shownCount} />
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
}

function StatusPicker({ t, game, status, active, onSaved, onShare }: PickerProps) {
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
          toast({ text: t.savedAs(t.statuses[saved.status]), action: { label: t.shareStatus, run: onShare } });
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
  sharing: boolean;
  onSaved: (progress: Progress) => void;
  onShare: () => void;
}

/** Hours played, saved on Enter or when the field loses focus, and "Post update". */
function ProgressRow({ t, game, progress, sharing, onSaved, onShare }: ProgressProps) {
  const saved = progress.hoursPlayed === null ? "" : String(progress.hoursPlayed);
  const [draft, setDraft] = useState(saved);
  const [busy, setBusy] = useState(false);
  const toast = useToast();

  useEffect(() => setDraft(saved), [game.id, saved]);

  const editable = progress.status !== null && !progress.steamHours;
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
        <button class="button subtle small" disabled={progress.status === null} onClick={onShare}>
          <Icon name="arrow" size={16} />
          {t.shareStatus}
        </button>
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
