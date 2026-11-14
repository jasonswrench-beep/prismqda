// What the keys are called on this computer, so a shortcut is shown the way the person presses it.

/** True on macOS and iOS-like platforms, where the command key replaces Control. */
export function isApplePlatform(nav: { platform?: string; userAgent?: string } = typeof navigator === 'undefined' ? {} : navigator): boolean {
  return /Mac|iPhone|iPad/i.test(nav.platform ?? '') || /Mac OS X/i.test(nav.userAgent ?? '');
}

/** "Ctrl+K" on Windows and Linux, "⌘K" on a Mac. */
export function shortcutLabel(key: string, apple: boolean = isApplePlatform()): string {
  return apple ? `⌘${key.toUpperCase()}` : `Ctrl+${key.toUpperCase()}`;
}
