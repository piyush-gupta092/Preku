import "@fontsource-variable/plus-jakarta-sans";
import "@fontsource/jetbrains-mono/500.css";
import "./styles/tokens.css";
import "./styles/base.css";
import "./windows/island/ui.css";

import { mount, type Component } from "svelte";
import { getCurrentWindow } from "@tauri-apps/api/window";
import Island from "./windows/island/Island.svelte";
import Settings from "./windows/settings/Settings.svelte";

// One bundle serves every window; the window label picks the view.
const views: Record<string, Component> = { island: Island, settings: Settings };
const label = getCurrentWindow().label;
const view = views[label];

const target = document.getElementById("app");
if (!target || !view) {
  throw new Error(`No view for window "${label}"`);
}
document.documentElement.dataset.window = label;

export default mount(view, { target });
