// The notifications tab: the latest notifications, newest first. A post
// notification opens the post with its comments right here; a follow opens
// the follower's profile on the website. Opening one marks it read.

import { useEffect, useRef, useState } from "preact/hooks";

import { Avatar, PostCard, timeAgo } from "./feed";
import { errorMessage, type Strings } from "./i18n";
import { Icon, type IconName } from "./icons";
import { native, type FeedPost, type NotificationKind, type PanelNotification } from "./native";

interface Props {
  t: Strings;
  /** True while this tab is shown. */
  active: boolean;
  /** Changes every time the overlay is shown, to reload on the next visit. */
  shownCount: number;
  /** Updates the unread count shown on the tab. */
  onUnread: (update: (count: number) => number) => void;
}

/** A post opened from a notification. */
interface Opened {
  notificationId: string;
  commentId: string | null;
  post: FeedPost | null;
  error: string | null;
}

const KIND_ICONS: Record<NotificationKind, IconName> = {
  like: "heart",
  commentLike: "heart",
  comment: "comment",
  commentReply: "comment",
  mention: "at",
  follow: "follow",
};

export function NotificationsTab({ t, active, shownCount, onUnread }: Props) {
  const [items, setItems] = useState<PanelNotification[]>([]);
  const [loadedFor, setLoadedFor] = useState<number | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [opened, setOpened] = useState<Opened | null>(null);
  const request = useRef(0);
  const root = useRef<HTMLDivElement>(null);

  const load = () => {
    const id = ++request.current;
    setLoading(true);
    setError(null);
    native
      .notifications()
      .then((list) => {
        if (id !== request.current) return;
        setItems(list);
        setLoadedFor(shownCount);
      })
      .catch((reason) => id === request.current && setError(errorMessage(t, reason)))
      .finally(() => id === request.current && setLoading(false));
  };

  // Load when the tab is opened for the first time since the panel was shown.
  useEffect(() => {
    if (active && loadedFor !== shownCount) load();
  }, [active, shownCount]);

  const setRead = (id: string, read: boolean) =>
    setItems((current) => current.map((item) => (item.id === id ? { ...item, read } : item)));

  /** Marked at once; put back if the server refuses. */
  const markRead = (item: PanelNotification) => {
    if (item.read) return;
    setRead(item.id, true);
    onUnread((count) => Math.max(0, count - 1));
    native.markNotificationRead(item.id).catch(() => {
      setRead(item.id, false);
      onUnread((count) => count + 1);
    });
  };

  const markAll = () => {
    setError(null);
    native
      .markAllNotificationsRead()
      .then(() => {
        setItems((current) => current.map((item) => ({ ...item, read: true })));
        onUnread(() => 0);
      })
      .catch((reason) => setError(errorMessage(t, reason)));
  };

  const open = (item: PanelNotification) => {
    const target = item.target;
    if (target.kind === "none") return;
    markRead(item);
    setError(null);
    if (target.kind === "profile") {
      native.openProfile(target.username).catch((reason) => setError(errorMessage(t, reason)));
      return;
    }
    setOpened({ notificationId: item.id, commentId: target.commentId, post: null, error: null });
    native
      .getPost(target.postId)
      .then((post) => setOpened((current) => (current?.notificationId === item.id ? { ...current, post } : current)))
      .catch((reason) =>
        setOpened((current) =>
          current?.notificationId === item.id ? { ...current, error: errorMessage(t, reason) } : current,
        ),
      );
  };

  // Back to the list, with focus on the notification that was opened, or
  // the first one when it is gone.
  const back = () => {
    const id = opened?.notificationId ?? "";
    setOpened(null);
    requestAnimationFrame(() => {
      const list = root.current;
      const target =
        list?.querySelector<HTMLElement>(`[data-notification="${CSS.escape(id)}"]`) ??
        list?.querySelector<HTMLElement>("button.notification") ??
        list?.querySelector<HTMLElement>(".feed-toolbar button:not(:disabled)");
      target?.focus();
    });
  };

  const isAbout = (item: PanelNotification, postId: string) =>
    item.target.kind === "post" && item.target.postId === postId;

  if (opened) {
    return (
      <div class="feed" ref={root}>
        <PostView
          t={t}
          opened={opened}
          onBack={back}
          onChange={(post) => setOpened((current) => current && { ...current, post })}
          onRemove={(postId) => {
            // The server deletes the post's notifications with it.
            const gone = items.filter((item) => !item.read && isAbout(item, postId)).length;
            setItems((current) => current.filter((item) => !isAbout(item, postId)));
            if (gone > 0) onUnread((count) => Math.max(0, count - gone));
            back();
          }}
        />
      </div>
    );
  }

  const unread = items.some((item) => !item.read);

  return (
    <div class="feed" ref={root}>
      <div class="feed-toolbar">
        <button class="button subtle small" disabled={!unread} onClick={markAll}>
          <Icon name="check" size={16} />
          {t.markAllRead}
        </button>
        <button class="icon-button" aria-label={t.refresh} title={t.refresh} disabled={loading} onClick={load}>
          <Icon name="refresh" />
        </button>
      </div>

      {error && (
        <p class="error" role="alert">
          {error}
        </p>
      )}

      {items.length === 0 && loading && <p class="muted feed-note">{t.loading}</p>}
      {items.length === 0 && !loading && !error && loadedFor !== null && (
        <p class="muted feed-note">{t.notificationsEmpty}</p>
      )}

      <ul class="notification-list">
        {items.map((item) => (
          <li key={item.id}>
            <NotificationRow t={t} item={item} onOpen={() => open(item)} />
          </li>
        ))}
      </ul>
    </div>
  );
}

function NotificationRow({ t, item, onOpen }: { t: Strings; item: PanelNotification; onOpen: () => void }) {
  const name = item.actor?.displayName ?? t.deletedUser;
  const preview = item.preview && item.kind in t.notificationPreview ? item.preview : null;
  const text = preview
    ? t.notificationPreview[item.kind as keyof Strings["notificationPreview"]](preview)
    : t.notificationText[item.kind];
  const profile = item.target.kind === "profile";

  const content = (
    <>
      <span class="notification-avatar">
        <Avatar author={item.actor ?? { username: "", displayName: t.deletedUser, avatarUrl: null }} />
        <span class="notification-kind" data-kind={item.kind} aria-hidden="true">
          <Icon name={KIND_ICONS[item.kind]} size={11} filled={item.kind === "like" || item.kind === "commentLike"} />
        </span>
      </span>
      <span class="notification-body">
        <span class="notification-text">
          <span class="post-name">{name}</span> {text}
        </span>
        <span class="muted small">
          <time dateTime={item.createdAt}>{timeAgo(t, item.createdAt)}</time>
        </span>
      </span>
      {profile && <Icon name="external" size={14} />}
      {!item.read && (
        <span class="unread-dot">
          <span class="visually-hidden">{t.unread}</span>
        </span>
      )}
    </>
  );

  if (item.target.kind === "none") {
    return (
      <div class="notification" data-read={item.read}>
        {content}
      </div>
    );
  }
  return (
    <button
      class="notification"
      data-read={item.read}
      data-notification={item.id}
      title={profile ? t.openProfile : undefined}
      onClick={onOpen}
    >
      {content}
    </button>
  );
}

interface PostViewProps {
  t: Strings;
  opened: Opened;
  onBack: () => void;
  onChange: (post: FeedPost) => void;
  onRemove: (postId: string) => void;
}

function PostView({ t, opened, onBack, onChange, onRemove }: PostViewProps) {
  const backButton = useRef<HTMLButtonElement>(null);
  useEffect(() => backButton.current?.focus(), []);

  return (
    <>
      <div class="feed-toolbar">
        <button ref={backButton} class="button subtle small" onClick={onBack}>
          <Icon name="back" size={16} />
          {t.backToNotifications}
        </button>
      </div>
      {opened.error && (
        <p class="error" role="alert">
          {opened.error}
        </p>
      )}
      {!opened.post && !opened.error && <p class="muted feed-note">{t.loading}</p>}
      {opened.post && (
        <PostCard
          t={t}
          post={opened.post}
          onChange={onChange}
          onRemove={onRemove}
          initiallyOpen
          highlight={opened.commentId}
        />
      )}
    </>
  );
}
