/** Theme resolution shared by both windows. The Appearance setting
 * (system / light / dark) lives in localStorage with the rest of the
 * settings; the resolved theme is stamped as data-theme on <html>, which
 * styles.css keys its dark palette off. */

import { getCurrentWindow } from "@tauri-apps/api/window";

import { api } from "./api";

const SETTINGS_KEY = "openwith.settings";

export type Appearance = "system" | "light" | "dark";

function storedAppearance(): Appearance {
  try {
    const raw = localStorage.getItem(SETTINGS_KEY);
    const value = raw ? (JSON.parse(raw) as { appearance?: unknown }).appearance : undefined;
    return value === "light" || value === "dark" ? value : "system";
  } catch {
    return "system";
  }
}

const systemDark = window.matchMedia("(prefers-color-scheme: dark)");

/** The OS appearance as macOS reports it, once known. WebKit's
 * prefers-color-scheme can answer "light" for a window that hasn't been shown
 * yet (seen on a macOS 27 beta), so the hidden main window resolved the light
 * palette, revealed on it, and flipped to dark a frame later (issue #22). The
 * native value is app-wide and doesn't depend on window visibility; the media
 * query only stands in until it arrives. */
let nativeDark: boolean | null = null;

/** Stamp the resolved theme on <html>. Call whenever the setting changes. */
export function applyTheme(): void {
  const appearance = storedAppearance();
  const dark =
    appearance === "system" ? (nativeDark ?? systemDark.matches) : appearance === "dark";
  document.documentElement.dataset.theme = dark ? "dark" : "light";
  // The Dock icon follows the resolved theme too — macOS only swaps bundle
  // icons for the *system* appearance, so the app does it itself. Both
  // windows call this on a theme change; the swap is idempotent.
  api.setDockIconDark(dark).catch(() => {});
}

// Follow the OS live while the setting is "system".
systemDark.addEventListener("change", applyTheme);

// `storage` fires only in *other* windows of the same origin — exactly the
// cross-window path: changing the setting in the main window restyles the
// open menu-bar popover (and vice versa).
window.addEventListener("storage", applyTheme);

applyTheme();

/** Resolves once the native appearance is applied (or couldn't be read).
 * Startup awaits it before revealing the main window. */
export const themeReady: Promise<void> = (async () => {
  const win = getCurrentWindow();
  try {
    const theme = await win.theme();
    if (theme) {
      nativeDark = theme === "dark";
      applyTheme();
    }
    await win.onThemeChanged(({ payload }) => {
      nativeDark = payload === "dark";
      applyTheme();
    });
  } catch {
    // Keep the media query's answer.
  }
})();
