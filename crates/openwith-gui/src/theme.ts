/** Theme resolution shared by both windows. The Appearance setting
 * (system / light / dark) lives in localStorage with the rest of the
 * settings; the resolved theme is stamped as data-theme on <html>, which
 * styles.css keys its dark palette off. */

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

/** Stamp the resolved theme on <html>. Call whenever the setting changes. */
export function applyTheme(): void {
  const appearance = storedAppearance();
  const dark =
    appearance === "system" ? systemDark.matches : appearance === "dark";
  document.documentElement.dataset.theme = dark ? "dark" : "light";
}

// Follow the OS live while the setting is "system".
systemDark.addEventListener("change", applyTheme);

// `storage` fires only in *other* windows of the same origin — exactly the
// cross-window path: changing the setting in the main window restyles the
// open menu-bar popover (and vice versa).
window.addEventListener("storage", applyTheme);

applyTheme();
