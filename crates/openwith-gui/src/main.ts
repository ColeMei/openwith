import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

// One Vite bundle serves both windows; the label picks the UI.
if (getCurrentWebviewWindow().label === "menubar") {
  document.documentElement.classList.add("popover-window");
  void import("./menubar");
} else {
  void import("./app");
}
