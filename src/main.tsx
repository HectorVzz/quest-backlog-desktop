import "@fontsource-variable/hanken-grotesk";
import "./tokens.css";
import "./app.css";

import { getCurrentWindow } from "@tauri-apps/api/window";
import { render } from "preact";

import { App } from "./app";
import { PreferencesApp } from "./preferences";

// One bundle, two windows: the quick panel ("overlay") and Preferences.
const isPreferences = getCurrentWindow().label === "preferences";
render(isPreferences ? <PreferencesApp /> : <App />, document.getElementById("app")!);
