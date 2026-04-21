/** Payload returned by the open_file / read_file Rust commands. */
export type OpenedFile =
  | { kind: 'text'; path: string; contents: string }
  | { kind: 'docx'; path: string; bytes_b64: string };

/** Payload returned by the save_file_as Rust command. */
export interface SaveAsResult {
  path: string;
}

/** Structured error from Rust commands, serialized over IPC. */
export interface AnteError {
  kind: 'io' | 'not_utf8' | 'binary_file' | 'dialog_cancelled' | 'file_too_large';
  message: string;
}

/** User-facing error messages mapped from AnteError.kind. */
export const ERROR_MESSAGES: Record<AnteError['kind'], string> = {
  io: 'Could not read/write the file. Check that you have permission and the file still exists.',
  not_utf8: "This file doesn't appear to be a text file (invalid encoding).",
  binary_file: 'This looks like a binary file. ante only opens plain text files.',
  dialog_cancelled: '',
  file_too_large: 'This file is too large. ante works best with files under 10 MB.',
};

/** Theme mode: follow OS or force light/dark. */
export type ThemeMode = 'system' | 'light' | 'dark';

/** Resolved theme after applying system preference. */
export type ResolvedTheme = 'light' | 'dark';

/** Available page sizes for the document layout. */
export type PageSize = 'letter' | 'a4' | 'legal';

/** Pixel dimensions for a page size at 96 dpi. */
export interface PageDimensions {
  width: number;
  height: number;
}
