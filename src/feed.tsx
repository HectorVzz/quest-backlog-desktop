// The Feed and My posts tabs: read posts, like them and their comments,
// comment, and delete your own posts, without leaving the game. Anything
// else (profiles, older comments, videos, editing) opens the post on the
// website. `PostCard` is also used to open a post from a notification.

import { useEffect, useRef, useState } from "preact/hooks";

import { formatHours } from "./game-panel";
import { errorMessage, type Strings } from "./i18n";
import { Icon } from "./icons";
import { native, type Author, type FeedComment, type FeedPost, type FeedView, type Game } from "./native";
import { useToast } from "./toast";

// Mirrors COMMENT_MAX_CHARS in src-tauri/src/feed.rs and the API limit.
const MAX_COMMENT_CHARS = 500;

interface Props {
  t: Strings;
  /** "feed" is the home feed with For you and Following; "mine" the viewer's posts. */
  source: "feed" | "mine";
  /** True while this tab is shown. */
  active: boolean;
  /** Changes every time the overlay is shown, to reload on the next visit. */
  shownCount: number;
  /** The detected or chosen game: adds "This game" to the Feed tab. */
  game?: Game | null;
  /** Opens the Post tab with the game attached, from an empty "This game". */
  onCompose?: () => void;
}

/** The home feed's views, and the posts about the current game. */
type Segment = FeedView | "game";

interface ListState {
  posts: FeedPost[];
  next: string | null;
  /** The `shownCount` and view the list was loaded for. */
  loadedFor: string | null;
}

const EMPTY: ListState = { posts: [], next: null, loadedFor: null };

export function FeedTab({ t, source, active, shownCount, game = null, onCompose }: Props) {
  const [chosen, setView] = useState<Segment>("feed");
  // Without a game, "This game" falls back to For you.
  const view: Segment = chosen === "game" && !game ? "feed" : chosen;
  const gameSlug = view === "game" ? game!.slug : "";
  const [list, setList] = useState<ListState>(EMPTY);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Answers from an older request (another view, a refresh) are dropped.
  const request = useRef(0);

  const key = `${source}:${view}:${gameSlug}:${shownCount}`;

  const load = (more: boolean) => {
    const id = ++request.current;
    const next = more ? list.next : null;
    setLoading(true);
    setError(null);
    const page =
      source === "mine"
        ? native.myPostsPage(next)
        : view === "game"
          ? native.gamePostsPage(gameSlug, next)
          : native.feedPage(view, next);
    page
      .then((result) => {
        if (id !== request.current) return;
        setList((current) => ({
          posts: more ? [...current.posts, ...result.posts.filter((post) => !current.posts.some((p) => p.id === post.id))] : result.posts,
          next: result.next,
          loadedFor: key,
        }));
      })
      .catch((reason) => {
        if (id === request.current) setError(errorMessage(t, reason));
      })
      .finally(() => {
        if (id === request.current) setLoading(false);
      });
  };

  // Load when the tab is opened for the first time since the panel was
  // shown, or the view changed.
  useEffect(() => {
    if (active && list.loadedFor !== key) load(false);
  }, [active, key]);

  // Another game's posts never show under this one.
  useEffect(() => {
    if (view === "game") setList(EMPTY);
  }, [gameSlug]);

  const update = (post: FeedPost) =>
    setList((current) => ({ ...current, posts: current.posts.map((p) => (p.id === post.id ? post : p)) }));
  const remove = (postId: string) =>
    setList((current) => ({ ...current, posts: current.posts.filter((p) => p.id !== postId) }));

  const empty = source === "mine" ? t.myPostsEmpty : view === "following" ? t.followingEmpty : t.feedEmpty;
  const segments: Segment[] = game ? ["feed", "following", "game"] : ["feed", "following"];
  const showEmpty = list.posts.length === 0 && !loading && !error && list.loadedFor !== null;

  return (
    <div class="feed">
      <div class="feed-toolbar">
        {source === "feed" ? (
          <div class="segmented" role="group">
            {segments.map((option) => (
              <button
                key={option}
                class="segment"
                aria-pressed={view === option}
                title={option === "game" && game ? t.feedThisGameLabel(game.title) : undefined}
                onClick={() => {
                  if (option === view) return;
                  setList(EMPTY);
                  setView(option);
                }}
              >
                {option === "feed" ? t.feedForYou : option === "following" ? t.feedFollowing : t.feedThisGame}
              </button>
            ))}
          </div>
        ) : (
          <span />
        )}
        <button class="icon-button" aria-label={t.refresh} title={t.refresh} disabled={loading} onClick={() => load(false)}>
          <Icon name="refresh" />
        </button>
      </div>

      {error && (
        <p class="error" role="alert">
          {error}
        </p>
      )}

      {list.posts.length === 0 && loading && <p class="muted feed-note">{t.loading}</p>}
      {showEmpty &&
        (view === "game" && game ? (
          <div class="feed-note feed-empty">
            <p class="muted">{t.gameFeedEmpty(game.title)}</p>
            {onCompose && (
              <button class="button default small" onClick={onCompose}>
                {t.postFirst}
              </button>
            )}
          </div>
        ) : (
          <p class="muted feed-note">{empty}</p>
        ))}

      <ul class="post-list">
        {list.posts.map((post) => (
          <li key={post.id}>
            <PostCard t={t} post={post} onChange={update} onRemove={remove} />
          </li>
        ))}
      </ul>

      {list.next && list.posts.length > 0 && (
        <div class="feed-more">
          <button class="button default small" disabled={loading} onClick={() => load(true)}>
            {loading ? t.loading : t.loadMore}
          </button>
        </div>
      )}
    </div>
  );
}

interface PostCardProps {
  t: Strings;
  post: FeedPost;
  onChange: (post: FeedPost) => void;
  /** The post was deleted. */
  onRemove: (postId: string) => void;
  /** Start with the comments shown (opened from a notification). */
  initiallyOpen?: boolean;
  /** A comment to bring into view and mark once the comments load. */
  highlight?: string | null;
}

export function PostCard({ t, post, onChange, onRemove, initiallyOpen = false, highlight = null }: PostCardProps) {
  const [revealed, setRevealed] = useState(false);
  const [open, setOpen] = useState(initiallyOpen);
  const [liking, setLiking] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const deleteButton = useRef<HTMLButtonElement>(null);
  const cancelButton = useRef<HTMLButtonElement>(null);
  const toast = useToast();
  const hidden = post.spoiler && !revealed && !post.mine;

  useEffect(() => {
    if (confirming) cancelButton.current?.focus();
  }, [confirming]);

  const cancelDelete = () => {
    setConfirming(false);
    requestAnimationFrame(() => deleteButton.current?.focus());
  };

  const confirmDelete = () => {
    if (deleting) return;
    setDeleting(true);
    setError(null);
    native
      .deletePost(post.id)
      .then(() => {
        toast({ text: t.postDeleted });
        onRemove(post.id);
      })
      .catch((reason) => {
        setError(errorMessage(t, reason));
        setDeleting(false);
        setConfirming(false);
        requestAnimationFrame(() => deleteButton.current?.focus());
      });
  };

  const toggleLike = () => {
    if (liking) return;
    const liked = !post.likedByMe;
    setLiking(true);
    setError(null);
    // Shown at once; the server's counts replace it.
    onChange({ ...post, likedByMe: liked, likeCount: Math.max(0, post.likeCount + (liked ? 1 : -1)) });
    native
      .likePost(post.id, liked)
      .then((likes) => onChange({ ...post, ...likes }))
      .catch((reason) => {
        onChange(post);
        setError(errorMessage(t, reason));
      })
      .finally(() => setLiking(false));
  };

  return (
    <article class="post">
      <header class="post-header">
        <Avatar author={post.author} />
        <div class="post-byline">
          <span class="post-name">{post.author.displayName}</span>
          <span class="muted small">
            @{post.author.username} · <time dateTime={post.createdAt}>{timeAgo(t, post.createdAt)}</time>
          </span>
        </div>
      </header>

      {post.game && (
        <div class="post-game">
          {post.game.coverImageUrl ? (
            <img class="cover cover-tiny" src={post.game.coverImageUrl} alt="" />
          ) : (
            <span class="cover cover-tiny cover-empty" aria-hidden="true" />
          )}
          <span class="post-game-title">{post.game.title}</span>
          {post.kind === "status-update" && post.status && (
            <span class="post-status" style={`--status: var(--qb-status-${post.status})`}>
              <span class="status-dot" aria-hidden="true" />
              {t.statuses[post.status]}
              {post.hoursPlayed !== null && <span class="tabular"> · {t.hours(formatHours(t, post.hoursPlayed))}</span>}
            </span>
          )}
          {post.kind === "review" && post.rating !== null && (
            <span class="post-status post-rating tabular">
              <Icon name="star" size={14} filled />
              {t.ratingValue(formatHours(t, post.rating))}
            </span>
          )}
        </div>
      )}

      {hidden ? (
        <button class="spoiler-cover" onClick={() => setRevealed(true)}>
          {t.showSpoiler}
        </button>
      ) : (
        <>
          {post.content && <p class="post-text">{post.content}</p>}
          {post.image && (
            <img
              class="post-image"
              src={post.image.url}
              alt=""
              loading="lazy"
              style={post.image.aspectRatio ? { aspectRatio: String(post.image.aspectRatio) } : undefined}
            />
          )}
          {post.hasVideo && (
            <button class="link-button post-video" onClick={() => native.openPost(post.id)}>
              {t.videoOnWeb}
            </button>
          )}
        </>
      )}

      <footer class="post-actions">
        <button
          class="post-action"
          aria-pressed={post.likedByMe}
          aria-label={post.likedByMe ? t.unlike : t.like}
          title={post.likedByMe ? t.unlike : t.like}
          disabled={liking}
          onClick={toggleLike}
        >
          <Icon name="heart" size={16} filled={post.likedByMe} />
          <span class="tabular">{post.likeCount}</span>
        </button>
        <button
          class="post-action"
          aria-expanded={open}
          aria-label={t.comments}
          title={t.comments}
          onClick={() => setOpen(!open)}
        >
          <Icon name="comment" size={16} />
          <span class="tabular">{post.commentCount}</span>
        </button>
        <span class="post-actions-end">
          {post.mine && (
            <button
              ref={deleteButton}
              class="post-action"
              aria-label={t.deletePost}
              title={t.deletePost}
              aria-expanded={confirming}
              disabled={deleting}
              onClick={() => (confirming ? cancelDelete() : setConfirming(true))}
            >
              <Icon name="trash" size={16} />
            </button>
          )}
          <button class="post-action" aria-label={t.openOnWeb} title={t.openOnWeb} onClick={() => native.openPost(post.id)}>
            <Icon name="external" size={16} />
          </button>
        </span>
      </footer>

      {confirming && (
        <div
          class="confirm"
          role="group"
          aria-labelledby={`delete-${post.id}`}
          onKeyDown={(event) => {
            // Esc cancels here instead of closing the panel.
            if (event.key === "Escape") {
              event.stopPropagation();
              if (!deleting) cancelDelete();
            }
          }}
        >
          <p id={`delete-${post.id}`} class="confirm-text">
            {t.deletePostConfirm}
          </p>
          <div class="actions">
            <button class="button danger small" disabled={deleting} onClick={confirmDelete}>
              {deleting ? t.deleting : t.delete}
            </button>
            <button ref={cancelButton} class="button subtle small" disabled={deleting} onClick={cancelDelete}>
              {t.cancel}
            </button>
          </div>
        </div>
      )}

      {error && (
        <p class="error small" role="alert">
          {error}
        </p>
      )}

      {open && (
        <Thread
          t={t}
          post={post}
          highlight={highlight}
          onAdded={() => onChange({ ...post, commentCount: post.commentCount + 1 })}
        />
      )}
    </article>
  );
}

type CommentPatch = Pick<FeedComment, "likeCount" | "likedByMe">;

/** Updates one comment or reply, found by id. */
function patchComment(comments: FeedComment[], id: string, patch: CommentPatch): FeedComment[] {
  return comments.map((comment) =>
    comment.id === id
      ? { ...comment, ...patch }
      : { ...comment, replies: comment.replies.map((reply) => (reply.id === id ? { ...reply, ...patch } : reply)) },
  );
}

interface ThreadProps {
  t: Strings;
  post: FeedPost;
  highlight: string | null;
  onAdded: () => void;
}

function Thread({ t, post, highlight, onAdded }: ThreadProps) {
  const [comments, setComments] = useState<FeedComment[] | null>(null);
  const [total, setTotal] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [replyTo, setReplyTo] = useState<FeedComment | null>(null);
  const [text, setText] = useState("");
  const [sending, setSending] = useState(false);
  const input = useRef<HTMLTextAreaElement>(null);
  const section = useRef<HTMLElement>(null);

  // Bring the comment a notification points at into view once loaded.
  useEffect(() => {
    if (!highlight || !comments) return;
    section.current
      ?.querySelector(`[data-comment="${CSS.escape(highlight)}"]`)
      ?.scrollIntoView({ block: "nearest" });
  }, [highlight, comments !== null]);

  const changeComment = (id: string, patch: CommentPatch) =>
    setComments((current) => patchComment(current ?? [], id, patch));

  useEffect(() => {
    native
      .postComments(post.id)
      .then((result) => {
        setComments(result.comments);
        setTotal(result.total);
      })
      .catch((reason) => setError(errorMessage(t, reason)));
  }, [post.id]);

  const shown = (comments ?? []).reduce((count, comment) => count + 1 + comment.replies.length, 0);
  const older = Math.max(0, total - shown);
  const length = [...text.trim()].length;
  const canSend = !sending && length > 0 && length <= MAX_COMMENT_CHARS;

  const send = () => {
    if (!canSend) return;
    setSending(true);
    setError(null);
    native
      .addComment(post.id, text, replyTo?.id ?? null)
      .then((comment) => {
        setComments((current) =>
          replyTo
            ? (current ?? []).map((c) => (c.id === replyTo.id ? { ...c, replies: [...c.replies, comment] } : c))
            : [...(current ?? []), comment],
        );
        setTotal((count) => count + 1);
        setText("");
        setReplyTo(null);
        onAdded();
      })
      .catch((reason) => setError(errorMessage(t, reason)))
      .finally(() => setSending(false));
  };

  const startReply = (comment: FeedComment) => {
    setReplyTo(comment);
    requestAnimationFrame(() => input.current?.focus());
  };

  return (
    <section class="thread" aria-label={t.comments} ref={section}>
      {comments === null && !error && <p class="muted small">{t.loading}</p>}
      {older > 0 && (
        <button class="link-button small" onClick={() => native.openPost(post.id)}>
          {t.olderComments(older)}
        </button>
      )}
      {comments?.length === 0 && <p class="muted small">{t.noComments}</p>}
      {comments && comments.length > 0 && (
        <ul class="comment-list">
          {comments.map((comment) => (
            <li key={comment.id}>
              <CommentItem
                t={t}
                comment={comment}
                highlighted={comment.id === highlight}
                onChange={changeComment}
                onReply={() => startReply(comment)}
              />
              {comment.replies.length > 0 && (
                <ul class="comment-list comment-replies">
                  {comment.replies.map((reply) => (
                    <li key={reply.id}>
                      <CommentItem
                        t={t}
                        comment={reply}
                        highlighted={reply.id === highlight}
                        onChange={changeComment}
                        onReply={() => startReply(comment)}
                      />
                    </li>
                  ))}
                </ul>
              )}
            </li>
          ))}
        </ul>
      )}

      {error && (
        <p class="error small" role="alert">
          {error}
        </p>
      )}

      <div class="comment-form">
        {replyTo && (
          <div class="game-chip">
            <span>{t.replyTo(replyTo.author.displayName)}</span>
            <button class="chip-remove" aria-label={t.cancel} title={t.cancel} onClick={() => setReplyTo(null)}>
              <Icon name="close" size={12} />
            </button>
          </div>
        )}
        <div class="comment-row">
          <textarea
            ref={input}
            class="composer-input comment-input"
            rows={1}
            value={text}
            placeholder={replyTo ? t.replyTo(replyTo.author.displayName) : t.commentPlaceholder}
            aria-label={t.commentPlaceholder}
            onInput={(event) => setText(event.currentTarget.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter" && !event.shiftKey) {
                event.preventDefault();
                send();
              }
            }}
          />
          <button class="button primary small" disabled={!canSend} onClick={send}>
            {sending ? t.sending : t.send}
          </button>
        </div>
        {length > MAX_COMMENT_CHARS && (
          <span class="counter counter-over">
            {length}/{MAX_COMMENT_CHARS}
          </span>
        )}
      </div>
    </section>
  );
}

interface CommentItemProps {
  t: Strings;
  comment: FeedComment;
  highlighted: boolean;
  onChange: (id: string, patch: CommentPatch) => void;
  onReply: () => void;
}

function CommentItem({ t, comment, highlighted, onChange, onReply }: CommentItemProps) {
  const [liking, setLiking] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Like a post's like: shown at once, the server's counts replace it.
  const toggleLike = () => {
    if (liking) return;
    const liked = !comment.likedByMe;
    setLiking(true);
    setError(null);
    const before = { likeCount: comment.likeCount, likedByMe: comment.likedByMe };
    onChange(comment.id, { likedByMe: liked, likeCount: Math.max(0, comment.likeCount + (liked ? 1 : -1)) });
    native
      .likeComment(comment.id, liked)
      .then((likes) => onChange(comment.id, likes))
      .catch((reason) => {
        onChange(comment.id, before);
        setError(errorMessage(t, reason));
      })
      .finally(() => setLiking(false));
  };

  return (
    <div class="comment" data-comment={comment.id} data-highlight={highlighted || undefined}>
      <Avatar author={comment.author} small />
      <div class="comment-body">
        <p class="comment-meta">
          <span class="post-name">{comment.author.displayName}</span>{" "}
          <span class="muted small">
            <time dateTime={comment.createdAt}>{timeAgo(t, comment.createdAt)}</time>
          </span>
        </p>
        {comment.content && <p class="comment-text">{comment.content}</p>}
        {comment.image && <img class="comment-image" src={comment.image.url} alt="" loading="lazy" />}
        <div class="comment-actions">
          <button
            class="post-action comment-like"
            aria-pressed={comment.likedByMe}
            aria-label={comment.likedByMe ? t.unlike : t.like}
            title={comment.likedByMe ? t.unlike : t.like}
            disabled={liking}
            onClick={toggleLike}
          >
            <Icon name="heart" size={14} filled={comment.likedByMe} />
            {comment.likeCount > 0 && <span class="tabular">{comment.likeCount}</span>}
          </button>
          <button class="link-button small" onClick={onReply}>
            {t.reply}
          </button>
        </div>
        {error && (
          <p class="error small" role="alert">
            {error}
          </p>
        )}
      </div>
    </div>
  );
}

export function Avatar({ author, small = false }: { author: Author; small?: boolean }) {
  const [broken, setBroken] = useState(false);
  const size = small ? "avatar avatar-small" : "avatar";
  if (author.avatarUrl && !broken) {
    return <img class={size} src={author.avatarUrl} alt="" onError={() => setBroken(true)} />;
  }
  const initial = [...(author.displayName || author.username)][0]?.toUpperCase() ?? "?";
  return (
    <span class={`${size} avatar-initial`} aria-hidden="true">
      {initial}
    </span>
  );
}

const UNITS: [Intl.RelativeTimeFormatUnit, number][] = [
  ["year", 365 * 24 * 3600],
  ["month", 30 * 24 * 3600],
  ["week", 7 * 24 * 3600],
  ["day", 24 * 3600],
  ["hour", 3600],
  ["minute", 60],
];

/** "3 h ago", "yesterday"; under a minute reads as "now". */
export function timeAgo(t: Strings, iso: string) {
  const seconds = (Date.now() - new Date(iso).getTime()) / 1000;
  if (!Number.isFinite(seconds)) return "";
  const format = new Intl.RelativeTimeFormat(t.locale, { numeric: "auto", style: "short" });
  for (const [unit, size] of UNITS) {
    if (Math.abs(seconds) >= size) return format.format(-Math.round(seconds / size), unit);
  }
  return format.format(0, "second");
}
