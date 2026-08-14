export function ago(when: string | null): string {
  if (!when) return "";
  const seconds = Math.max(0, (Date.now() - new Date(when).getTime()) / 1000);
  return spellOut(seconds);
}

export function between(from: string, to: string): string {
  return spellOut(Math.max(0, (new Date(to).getTime() - new Date(from).getTime()) / 1000));
}

export function spellOut(seconds: number | null): string {
  if (seconds === null) return "—";
  const whole = Math.round(seconds);
  if (whole < 60) return `${whole}s`;
  if (whole < 3600) return `${Math.floor(whole / 60)}m ${whole % 60}s`;
  const hours = Math.floor(whole / 3600);
  return `${hours}h ${Math.floor((whole % 3600) / 60)}m`;
}
