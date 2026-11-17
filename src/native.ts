// The only bridge to the Rust core. The webview never handles the device
// token, screenshots (beyond a preview) or HTTP requests itself.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { ErrorKey, StatusKey } from "./i18n";

export interface Game {
  id: string;
  slug: string;
  title: string;
  coverImageUrl: string | null;
  releaseDate: string | null;
}

export type Detection =
  | { kind: "game"; game: Game; status: StatusKey | null; executable: string | null }
  | { kind: "unknown"; executable: string; query: string }
  | { kind: "nothing" };

export interface User {
  id: string;
  username: string;
  displayName: string;
  avatarUrl: string | null;
}

export type AuthView =
  | { state: "loading" }
  | { state: "signedOut"; error: ErrorKey | null }
  | { state: "pending"; userCode: string }
  | { state: "signedIn"; user: User }
  | { state: "offline" };

export interface HotkeyStatus {
  toggle: string | null;
  screenshot: string | null;
  failed: boolean;
  /** Wayland: the desktop owns the bindings. */
  systemManaged: boolean;
}

export interface AppInfo {
  version: string;
  language: "en" | "es";
  hotkey: HotkeyStatus;
  /** The first-run shortcut tips were dismissed. */
  tipsSeen: boolean;
}

export type ScreenshotView =
  | { state: "none" }
  | { state: "capturing" }
  | { state: "ready"; preview: string; savedTo: string | null; saveFailed: boolean }
  | { state: "failed" };

export interface Preferences {
  hotkey: string;
  screenshotHotkey: string;
  saveScreenshots: boolean;
  screenshotFolder: string | null;
  language: "en" | "es" | null;
  startWithSystem: boolean;
  hotkeys: HotkeyStatus;
}

export type PreferencesInput = Omit<Preferences, "hotkeys">;

export interface NewPost {
  content: string;
  gameId: string | null;
  spoiler: boolean;
  withScreenshot: boolean;
}

// Commands that can fail reject with a key from `errors` in i18n.ts, or
// `server:<message>`; show them with `errorMessage`.
export const native = {
  appInfo: () => invoke<AppInfo>("get_app_info"),
  dismissTips: () => invoke<void>("dismiss_tips"),
  auth: () => invoke<AuthView>("get_auth"),
  startSignIn: () => invoke<void>("start_sign_in"),
  cancelSignIn: () => invoke<void>("cancel_sign_in"),
  retryAuth: () => invoke<void>("retry_auth"),
  signOut: () => invoke<void>("sign_out"),
  openSignInPage: () => invoke<void>("open_sign_in_page"),
  hide: () => invoke<void>("hide_overlay"),

  detectGame: () => invoke<Detection>("detect_game"),
  searchGames: (query: string) => invoke<Game[]>("search_games", { query }),
  chooseGame: (game: Game) =>
    invoke<{ game: Game; status: StatusKey | null }>("choose_game", { game }),
  setGameStatus: (gameId: string, status: StatusKey) =>
    invoke<StatusKey>("set_game_status", { gameId, status }),

  screenshot: () => invoke<ScreenshotView>("get_screenshot"),
  takeScreenshot: (chooseWindow = false) => invoke<void>("take_screenshot", { chooseWindow }),
  discardScreenshot: () => invoke<void>("discard_screenshot"),
  createPost: (post: NewPost) => invoke<string>("create_post", { ...post }),
  openPost: (postId: string) => invoke<void>("open_post", { postId }),

  openPreferences: () => invoke<void>("open_preferences"),
  closePreferences: () => invoke<void>("close_preferences"),
  preferences: () => invoke<Preferences>("get_preferences"),
  updatePreferences: (input: PreferencesInput) =>
    invoke<Preferences>("update_preferences", { input }),
  chooseScreenshotFolder: () => invoke<string | null>("choose_screenshot_folder"),
  configureShortcuts: () => invoke<void>("configure_shortcuts"),

  onAuthChanged: (handler: (view: AuthView) => void): Promise<UnlistenFn> =>
    listen<AuthView>("auth://changed", (event) => handler(event.payload)),
  onShown: (handler: () => void): Promise<UnlistenFn> =>
    listen("overlay://shown", () => handler()),
  /** The panel is about to hide: play the closing animation. */
  onHiding: (handler: () => void): Promise<UnlistenFn> =>
    listen("overlay://hiding", () => handler()),
  onSettingsChanged: (handler: () => void): Promise<UnlistenFn> =>
    listen("settings://changed", () => handler()),
  onCompose: (handler: () => void): Promise<UnlistenFn> =>
    listen("overlay://compose", () => handler()),
  onScreenshot: (handler: (view: ScreenshotView) => void): Promise<UnlistenFn> =>
    listen<ScreenshotView>("screenshot://changed", (event) => handler(event.payload)),
};
