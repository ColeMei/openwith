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

export function initial(name: string): string {
  const trimmed = name.trim();
  return trimmed.length > 0 ? trimmed[0].toUpperCase() : "?";
}
