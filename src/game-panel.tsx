import { useEffect, useRef, useState } from "preact/hooks";

import { Composer } from "./composer";
import { errorMessage, type StatusKey, type Strings } from "./i18n";
import { native, type Game } from "./native";

const STATUSES: StatusKey[] = ["playing", "completed", "platinumed", "backlog", "dropped", "wishlist"];

type View =
  | { mode: "detecting" }
  | { mode: "game"; game: Game; status: StatusKey | null }
  | { mode: "search"; reason: "unknown" | "nothing" | "change"; executable: string | null; query: string };

interface Props {
  t: Strings;
  /** Changes every time the overlay is shown, to detect again. */
  shownCount: number;
  /** Changes when a screenshot was taken, to switch to the Post tab. */
  composeCount: number;
}

type Tab = "status" | "post";

export function GamePanel({ t, shownCount, composeCount }: Props) {
  const [tab, setTab] = useState<Tab>("status");
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
          setView({ mode: "game", game: detection.game, status: detection.status });
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

  const choose = (game: Game) => {
    setError(null);
    native
      .chooseGame(game)
      .then((chosen) => {
        setPrevious(null);
        setView({ mode: "game", game: chosen.game, status: chosen.status });
      })
      .catch((key) => setError(errorMessage(t, key)));
  };

  // While searching for a different game, posts keep the one shown before.
  const currentGame =
    view.mode === "game" ? view.game : previous?.mode === "game" ? previous.game : null;

  const tabs: { id: Tab; label: string }[] = [
    { id: "status", label: t.tabStatus },
    { id: "post", label: t.tabPost },
  ];

  return (
    <>
      <div class="tabs" role="tablist">
        {tabs.map(({ id, label }) => (
          <button
            key={id}
            role="tab"
            class="tab"
            aria-selected={tab === id}
            onClick={() => setTab(id)}
          >
            {label}
          </button>
        ))}
      </div>
      {/* Both stay mounted so a post draft survives switching tabs. */}
      <div class="tab-panel" role="tabpanel" hidden={tab !== "status"}>
        <StatusTab
          t={t}
          view={view}
          previous={previous}
          error={error}
          onView={setView}
          onChange={(game) => {
            setPrevious(game);
            if (game.mode === "game") {
              setView({ mode: "search", reason: "change", executable: null, query: game.game.title });
            }
          }}
          onChoose={choose}
          onDetect={detect}
        />
      </div>
      <div class="tab-panel" role="tabpanel" hidden={tab !== "post"}>
        <Composer t={t} game={currentGame} active={tab === "post"} />
      </div>
    </>
  );
}

interface StatusTabProps {
  t: Strings;
  view: View;
  previous: View | null;
  error: string | null;
  onView: (view: View) => void;
  onChange: (current: View) => void;
  onChoose: (game: Game) => void;
  onDetect: () => void;
}

function StatusTab({ t, view, previous, error, onView, onChange, onChoose, onDetect }: StatusTabProps) {
  if (view.mode === "detecting") {
    return <p class="muted">{t.detecting}</p>;
  }

  if (view.mode === "game") {
    return (
      <>
        <GameStatusCard
          t={t}
          game={view.game}
          status={view.status}
          onSaved={(status) => onView({ ...view, status })}
        />
        <div class="actions">
          <button class="button subtle" onClick={() => onChange(view)}>
            {t.changeGame}
          </button>
        </div>
      </>
    );
  }

  const heading =
    view.reason === "unknown" && view.executable ? t.whichGame(view.executable) : view.reason === "nothing" ? t.noGame : null;
  const hint = view.reason === "unknown" ? t.whichGameHint : view.reason === "nothing" ? t.noGameHint : null;

  return (
    <>
      {heading && <h1>{heading}</h1>}
      {hint && <p class="muted">{hint}</p>}
      {error && (
        <p class="error" role="alert">
          {error}
        </p>
      )}
      <GameSearch key={`${view.reason}:${view.query}`} t={t} initialQuery={view.query} onChoose={onChoose} />
      <div class="actions">
        {previous ? (
          <button class="button subtle" onClick={() => onView(previous)}>
            {t.backToGame}
          </button>
        ) : (
          <button class="button subtle" onClick={onDetect}>
            {t.detectAgain}
          </button>
        )}
      </div>
    </>
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

interface CardProps {
  t: Strings;
  game: Game;
  status: StatusKey | null;
  onSaved: (status: StatusKey) => void;
}

function GameStatusCard({ t, game, status, onSaved }: CardProps) {
  const [saving, setSaving] = useState<StatusKey | null>(null);
  const [message, setMessage] = useState<{ text: string; error: boolean } | null>(null);
  const year = releaseYear(game);

  useEffect(() => setMessage(null), [game.id]);

  const save = (next: StatusKey) => {
    if (saving || next === status) return;
    setSaving(next);
    setMessage(null);
    native
      .setGameStatus(game.id, next)
      .then((saved) => {
        onSaved(saved);
        setMessage({ text: t.savedAs(t.statuses[saved]), error: false });
      })
      .catch((key) => setMessage({ text: errorMessage(t, key), error: true }))
      .finally(() => setSaving(null));
  };

  return (
    <div class="game-card">
      <div class="game-header">
        <Cover game={game} size="large" />
        <div>
          <h1>{game.title}</h1>
          <p class="muted small">
            {year}
            {year && " · "}
            {status ? t.statuses[status] : t.notTracked}
          </p>
        </div>
      </div>
      <div class="status-grid" role="group">
        {STATUSES.map((value) => (
          <button
            key={value}
            class={`status-chip status-${value}`}
            aria-pressed={value === status}
            data-primary={value === "playing" ? true : undefined}
            disabled={saving !== null}
            onClick={() => save(value)}
          >
            {saving === value ? t.saving : t.statuses[value]}
          </button>
        ))}
      </div>
      {message && (
        <p class={message.error ? "error" : "success"} role="status">
          {message.text}
        </p>
      )}
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
      <input
        ref={input}
        class="search-input"
        type="search"
        value={query}
        placeholder={t.searchPlaceholder}
        aria-label={t.searchPlaceholder}
        onInput={(event) => setQuery(event.currentTarget.value)}
      />
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
                <span class="muted small">{releaseYear(game)}</span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
