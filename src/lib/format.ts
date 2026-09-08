/** Shorten a home-relative path the way a shell prompt would. */
export function tildePath(path: string, home: string | null): string {
  if (home && path.startsWith(home)) return "~" + path.slice(home.length);
  return path;
}


export function shortSha(sha: string): string {
  return sha.slice(0, 7);
}


/**
 * A path shortened for a narrow row. The end of a path identifies it, so the
 * middle is what gets dropped: `/a/very/long/prefix/wt/feature-login` becomes
 * `…/wt/feature-login`.
 */
export function displayPath(path: string, home: string | null, maxChars = 54): string {
  const short = tildePath(path, home);
  if (short.length <= maxChars) return short;

  const parts = short.split("/").filter(Boolean);
  // Keep adding segments from the end until one more would overflow.
  const tail: string[] = [];
  for (let i = parts.length - 1; i >= 0; i--) {
    const segment = parts[i]!;
    if (tail.length > 0 && [segment, ...tail].join("/").length + 2 > maxChars) break;
    tail.unshift(segment);
  }
  return "…/" + tail.join("/");
}
