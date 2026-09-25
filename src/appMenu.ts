import { Menu, MenuItem, PredefinedMenuItem, Submenu } from "@tauri-apps/api/menu";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { LogicalPosition } from "@tauri-apps/api/dpi";
import { APP_NAME } from "./branding";
import { messages } from "./i18n/index.ts";

const ABOUT_WIDTH = 300;
const ABOUT_HEIGHT = 520;

export async function installAppMenu(): Promise<void> {
  const about = await MenuItem.new({
    id: "about-crypt8",
    text: messages.aboutMenu,
    action: () => {
      void openAbout();
    },
  });
  const separator = await PredefinedMenuItem.new({ item: "Separator" });
  const quit = await PredefinedMenuItem.new({ item: "Quit" });
  const mac = navigator.userAgent.includes("Mac");
  const items = mac
    ? [
        about,
        separator,
        await PredefinedMenuItem.new({ item: "Hide" }),
        await PredefinedMenuItem.new({ item: "HideOthers" }),
        await PredefinedMenuItem.new({ item: "ShowAll" }),
        await PredefinedMenuItem.new({ item: "Separator" }),
        quit,
      ]
    : [about, separator, quit];
  const app = await Submenu.new({ text: APP_NAME, items });
  const edit = await Submenu.new({
    text: messages.editMenu,
    items: [
      await PredefinedMenuItem.new({ item: "Undo" }),
      await PredefinedMenuItem.new({ item: "Redo" }),
      await PredefinedMenuItem.new({ item: "Separator" }),
      await PredefinedMenuItem.new({ item: "Cut" }),
      await PredefinedMenuItem.new({ item: "Copy" }),
      await PredefinedMenuItem.new({ item: "Paste" }),
      await PredefinedMenuItem.new({ item: "Separator" }),
      await PredefinedMenuItem.new({ item: "SelectAll" }),
    ],
  });
  const menu = await Menu.new({ items: [app, edit] });
  await menu.setAsAppMenu();
}

export async function openAbout(): Promise<void> {
  const existing = await WebviewWindow.getByLabel("about");
  if (existing) {
    await existing.setFocus();
    return;
  }
  const main = await WebviewWindow.getByLabel("main");
  let x: number | undefined;
  let y: number | undefined;
  if (main) {
    const factor = await main.scaleFactor();
    const position = (await main.outerPosition()).toLogical(factor);
    const size = (await main.outerSize()).toLogical(factor);
    x = Math.round(position.x + (size.width - ABOUT_WIDTH) / 2);
    y = Math.round(position.y + (size.height - ABOUT_HEIGHT) / 2);
  }
  const aboutWindow = new WebviewWindow("about", {
    url: `${globalThis.location.pathname}?about=1`,
    title: APP_NAME,
    width: ABOUT_WIDTH,
    height: ABOUT_HEIGHT,
    resizable: false,
    maximizable: false,
    minimizable: false,
    center: x == null,
    x,
    y,
    parent: "main",
    alwaysOnTop: true,
    visibleOnAllWorkspaces: true,
    transparent: true,
    hiddenTitle: true,
    titleBarStyle: "overlay",
    trafficLightPosition: new LogicalPosition(14, 16),
    shadow: true,
    backgroundColor: "#00000000",
  });
  aboutWindow.once("tauri://error", (event) => {
    console.error(event);
  });
}
