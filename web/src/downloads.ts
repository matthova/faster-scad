// Per-OS desktop download resolution, shared by the marketing page (about.ts)
// and the in-app "Get the desktop app" callout (App.tsx). Both need to turn the
// visitor's OS into a direct GitHub "latest release" asset URL.

/** GitHub release assets get a stable, version-less alias (see
 *  .github/workflows/release.yml → `stable-name`), so these URLs always resolve
 *  to the latest release. Keep asset names in sync with index.html's per-OS
 *  cards. */
export const RELEASES_LATEST =
  "https://github.com/matthova/faster-scad/releases/latest";
export const DL = `${RELEASES_LATEST}/download`;

/** Stable release-asset filenames, per platform. */
export const ASSETS = {
  macArm: "Quito-macos-aarch64.dmg",
  macIntel: "Quito-macos-x64.dmg",
  windows: "Quito-windows-x64-setup.exe",
  linux: "Quito-linux-x86_64.AppImage",
} as const;

export type Os = "mac" | "windows" | "linux" | "other";

/** Best-effort OS classification from the UA string. Arch (Apple Silicon vs
 *  Intel) is resolved separately, asynchronously, since it isn't in the UA. */
export function detectOs(): Os {
  const ua = navigator.userAgent;
  // iPadOS reports as "Macintosh"; treat any touch-primary device as mobile so
  // we don't offer a desktop .dmg to a tablet.
  const touch = navigator.maxTouchPoints > 1;
  if (/Android/i.test(ua)) return "other";
  if (/iPhone|iPad|iPod/i.test(ua)) return "other";
  if (/Mac/i.test(ua)) return touch ? "other" : "mac";
  if (/Win/i.test(ua)) return "windows";
  if (/Linux|X11/i.test(ua)) return "linux";
  return "other";
}

/** True on Apple Silicon when the browser exposes it (Chromium's UA-CH). Safari
 *  and Firefox don't report arch on macOS, so we default to Apple Silicon —
 *  it's the common case now, and Intel stays one click away in the OS grid. */
export async function isAppleSilicon(): Promise<boolean> {
  const uaData = (
    navigator as Navigator & {
      userAgentData?: {
        getHighEntropyValues: (
          hints: string[],
        ) => Promise<{ architecture?: string }>;
      };
    }
  ).userAgentData;
  if (!uaData?.getHighEntropyValues) return true; // unknown → assume ARM
  try {
    const { architecture } = await uaData.getHighEntropyValues([
      "architecture",
    ]);
    // Chromium reports "arm" for Apple Silicon, "x86" for Intel.
    return architecture ? architecture !== "x86" : true;
  } catch {
    return true;
  }
}

/** Direct download URL for the visitor's OS, or null on phones/tablets and
 *  anything we can't place — there's no desktop build to push there. */
export async function pickDownloadUrl(): Promise<string | null> {
  switch (detectOs()) {
    case "mac":
      return `${DL}/${(await isAppleSilicon()) ? ASSETS.macArm : ASSETS.macIntel}`;
    case "windows":
      return `${DL}/${ASSETS.windows}`;
    case "linux":
      return `${DL}/${ASSETS.linux}`;
    default:
      return null;
  }
}
