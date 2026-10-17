// Match Rust str::trim (Unicode White_Space), not JavaScript trim (which includes BOM).
// The shared editor remains responsible for validating and applying the script.
export function isBlankScriptLine(text: string): boolean {
  return /^\p{White_Space}*$/u.test(text);
}
