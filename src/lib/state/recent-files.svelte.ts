const STORAGE_KEY = 'ante.recentFiles';
const MAX_RECENTS = 10;

export interface RecentFile {
  path: string;
  openedAt: number;
}

function safeRead(): RecentFile[] {
  if (typeof localStorage === 'undefined') return [];
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw) as unknown;
    if (!Array.isArray(parsed)) return [];
    return parsed
      .filter(
        (x): x is RecentFile =>
          typeof x === 'object' &&
          x !== null &&
          typeof (x as RecentFile).path === 'string' &&
          typeof (x as RecentFile).openedAt === 'number',
      )
      .slice(0, MAX_RECENTS);
  } catch {
    return [];
  }
}

function safeWrite(list: RecentFile[]): void {
  if (typeof localStorage === 'undefined') return;
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(list));
  } catch {
    /* quota or unavailable; ignore */
  }
}

let recents = $state<RecentFile[]>(safeRead());

export const recentFiles = {
  get list(): RecentFile[] {
    return recents;
  },
  add(path: string): void {
    const next = [{ path, openedAt: Date.now() }, ...recents.filter((r) => r.path !== path)].slice(
      0,
      MAX_RECENTS,
    );
    recents = next;
    safeWrite(next);
  },
  remove(path: string): void {
    const next = recents.filter((r) => r.path !== path);
    recents = next;
    safeWrite(next);
  },
  clear(): void {
    recents = [];
    safeWrite([]);
  },
};

export function filenameFromPath(path: string): string {
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] || path;
}

export function dirFromPath(path: string): string {
  const parts = path.split(/[\\/]/);
  parts.pop();
  return parts.join('/') || '';
}
