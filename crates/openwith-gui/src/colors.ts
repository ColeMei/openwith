const HUES = [10, 35, 60, 145, 190, 220, 260, 300, 335];

function hash(input: string): number {
  let h = 0;
  for (let i = 0; i < input.length; i++) {
    h = (h * 31 + input.charCodeAt(i)) | 0;
  }
  return Math.abs(h);
}

export function avatarColor(name: string): string {
  const hue = HUES[hash(name) % HUES.length];
  return `oklch(0.58 0.13 ${hue})`;
}

/** Two-character chip initials in the prototype's style:
 * "Google Chrome" → "GC", "TextEdit" → "TE", "IINA" → "II", "Typora" → "Ty". */
export function initials(name: string): string {
  const words = name
    .trim()
    .split(/\s+/)
    .filter((w) => /[a-z0-9]/i.test(w));
  if (words.length === 0) return "?";
  if (words.length >= 2) {
    return (words[0][0] + words[1][0]).toUpperCase();
  }
  const word = words[0];
  const caps = word.match(/[A-Z]/g) ?? [];
  if (caps.length >= 2) return caps.slice(0, 2).join("");
  return word[0].toUpperCase() + (word[1] ?? "");
}
