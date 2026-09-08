/**
 * A CSS stack for the terminal. "Symbols Nerd Font Mono" sits behind the chosen
 * face so icon glyphs still resolve when the face itself is not patched.
 */
export function terminalFontStack(family: string | null): string {
  const base = `ui-monospace, "SF Mono", "Cascadia Mono", Menlo, Consolas, monospace`;
  if (!family?.trim()) return base;
  return `"${family}", "Symbols Nerd Font Mono", ${base}`;
}
