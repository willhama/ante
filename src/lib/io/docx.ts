/**
 * DOCX <-> HTML interop.
 *
 * Import path: mammoth converts a `.docx` byte buffer to HTML that TipTap
 * can ingest via `setContent`.
 *
 * Export path: a focused walker turns the HTML the editor produces into a
 * `docx.Document` tree, then `Packer` serialises it to bytes. The walker
 * covers the MVP surface from issue #18: paragraphs, headings, marks,
 * lists, hyperlinks, alignment. Out-of-scope features (tables, images,
 * footnotes) are dropped silently to avoid blocking export.
 */

import mammoth from 'mammoth';
import {
  Document,
  Packer,
  Paragraph,
  TextRun,
  HeadingLevel,
  AlignmentType,
  ExternalHyperlink,
  LevelFormat,
} from 'docx';

const NUMBERED_REF = 'ante-numbered';

const HEADING_LEVELS: Record<string, (typeof HeadingLevel)[keyof typeof HeadingLevel]> = {
  H1: HeadingLevel.HEADING_1,
  H2: HeadingLevel.HEADING_2,
  H3: HeadingLevel.HEADING_3,
  H4: HeadingLevel.HEADING_4,
  H5: HeadingLevel.HEADING_5,
  H6: HeadingLevel.HEADING_6,
};

const ALIGNMENT: Record<string, (typeof AlignmentType)[keyof typeof AlignmentType]> = {
  left: AlignmentType.LEFT,
  center: AlignmentType.CENTER,
  right: AlignmentType.RIGHT,
  justify: AlignmentType.JUSTIFIED,
};

export interface ImportResult {
  html: string;
  warnings: string[];
}

/** Decode base64 to Uint8Array. */
function base64ToUint8Array(b64: string): Uint8Array {
  const binary = atob(b64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

/** Encode Uint8Array as base64. Uses chunked conversion to stay safe on big files. */
function uint8ArrayToBase64(bytes: Uint8Array): string {
  let binary = '';
  const chunk = 0x8000;
  for (let i = 0; i < bytes.length; i += chunk) {
    binary += String.fromCharCode(...bytes.subarray(i, i + chunk));
  }
  return btoa(binary);
}

/** Convert .docx bytes (base64) to HTML via mammoth. */
export async function docxBytesToHtml(bytesB64: string): Promise<ImportResult> {
  const bytes = base64ToUint8Array(bytesB64);
  // mammoth wants an ArrayBuffer; copy into a fresh one to avoid SharedArrayBuffer typing issues.
  const ab = bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
  const result = await mammoth.convertToHtml({ arrayBuffer: ab as ArrayBuffer });
  return {
    html: result.value,
    warnings: result.messages.map((m) => m.message),
  };
}

/** Convert TipTap HTML to a .docx file (returned as base64). */
export async function htmlToDocxBytes(html: string): Promise<string> {
  const doc = buildDocument(html);
  const buffer = await Packer.toBuffer(doc);
  return uint8ArrayToBase64(new Uint8Array(buffer));
}

interface RunStyle {
  bold?: boolean;
  italics?: boolean;
  underline?: { type: 'single' };
  strike?: boolean;
  color?: string;
  font?: string;
  size?: number;
  link?: string;
}

function buildDocument(html: string): Document {
  const dom = new DOMParser().parseFromString(`<!doctype html><body>${html}</body>`, 'text/html');
  const body = dom.body;
  const children: Paragraph[] = [];
  for (const node of Array.from(body.childNodes)) {
    walkBlock(node, children, {});
  }
  if (children.length === 0) children.push(new Paragraph(''));

  return new Document({
    numbering: {
      config: [
        {
          reference: NUMBERED_REF,
          levels: [
            {
              level: 0,
              format: LevelFormat.DECIMAL,
              text: '%1.',
              alignment: AlignmentType.LEFT,
            },
          ],
        },
      ],
    },
    sections: [{ properties: {}, children }],
  });
}

interface BlockContext {
  list?: 'bullet' | 'numbered';
}

function walkBlock(node: Node, out: Paragraph[], ctx: BlockContext): void {
  if (node.nodeType === Node.TEXT_NODE) {
    const text = node.textContent?.trim();
    if (text) out.push(paragraphFromInline([textRun(text, {})], {}));
    return;
  }
  if (node.nodeType !== Node.ELEMENT_NODE) return;
  const el = node as HTMLElement;
  const tag = el.tagName;

  if (tag === 'P' || tag === 'DIV') {
    out.push(paragraphFromInline(collectInline(el, {}), { alignment: alignmentFor(el), list: ctx.list }));
    return;
  }
  if (HEADING_LEVELS[tag]) {
    out.push(
      paragraphFromInline(collectInline(el, {}), {
        heading: HEADING_LEVELS[tag],
        alignment: alignmentFor(el),
      }),
    );
    return;
  }
  if (tag === 'BLOCKQUOTE') {
    for (const child of Array.from(el.childNodes)) walkBlock(child, out, ctx);
    return;
  }
  if (tag === 'UL') {
    for (const li of Array.from(el.children)) {
      if (li.tagName === 'LI') walkBlock(li, out, { list: 'bullet' });
    }
    return;
  }
  if (tag === 'OL') {
    for (const li of Array.from(el.children)) {
      if (li.tagName === 'LI') walkBlock(li, out, { list: 'numbered' });
    }
    return;
  }
  if (tag === 'LI') {
    out.push(paragraphFromInline(collectInline(el, {}), { list: ctx.list }));
    return;
  }
  if (tag === 'BR') {
    out.push(new Paragraph(''));
    return;
  }

  // Unknown element - recurse to recover any text inside it.
  for (const child of Array.from(el.childNodes)) walkBlock(child, out, ctx);
}

function alignmentFor(el: HTMLElement): (typeof AlignmentType)[keyof typeof AlignmentType] | undefined {
  const raw = (el.style.textAlign || el.getAttribute('align') || '').toLowerCase();
  return ALIGNMENT[raw];
}

interface ParagraphOpts {
  heading?: (typeof HeadingLevel)[keyof typeof HeadingLevel];
  alignment?: (typeof AlignmentType)[keyof typeof AlignmentType];
  list?: 'bullet' | 'numbered';
}

function paragraphFromInline(
  runs: (TextRun | ExternalHyperlink)[],
  opts: ParagraphOpts,
): Paragraph {
  return new Paragraph({
    children: runs,
    ...(opts.heading ? { heading: opts.heading } : {}),
    ...(opts.alignment ? { alignment: opts.alignment } : {}),
    ...(opts.list === 'bullet' ? { bullet: { level: 0 } } : {}),
    ...(opts.list === 'numbered'
      ? { numbering: { reference: NUMBERED_REF, level: 0 } }
      : {}),
  });
}

function collectInline(el: Element, parentStyle: RunStyle): (TextRun | ExternalHyperlink)[] {
  const runs: (TextRun | ExternalHyperlink)[] = [];
  for (const child of Array.from(el.childNodes)) {
    runs.push(...collectInlineNode(child, parentStyle));
  }
  return runs;
}

function collectInlineNode(node: Node, parentStyle: RunStyle): (TextRun | ExternalHyperlink)[] {
  if (node.nodeType === Node.TEXT_NODE) {
    const text = node.textContent ?? '';
    if (!text) return [];
    return [textRun(text, parentStyle)];
  }
  if (node.nodeType !== Node.ELEMENT_NODE) return [];
  const el = node as HTMLElement;
  const tag = el.tagName;

  if (tag === 'BR') return [textRun('\n', parentStyle)];

  if (tag === 'A') {
    const href = el.getAttribute('href') ?? '';
    const inner = collectInline(el, { ...parentStyle, link: href });
    if (!href) return inner;
    // Wrap the inner runs in an ExternalHyperlink. ExternalHyperlink only
    // accepts TextRun children, so unwrap any nested hyperlinks.
    const flatRuns: TextRun[] = inner.flatMap((r) => (r instanceof TextRun ? [r] : []));
    return [new ExternalHyperlink({ link: href, children: flatRuns })];
  }

  const style: RunStyle = { ...parentStyle };
  if (tag === 'STRONG' || tag === 'B') style.bold = true;
  if (tag === 'EM' || tag === 'I') style.italics = true;
  if (tag === 'U') style.underline = { type: 'single' };
  if (tag === 'S' || tag === 'STRIKE' || tag === 'DEL') style.strike = true;
  if (tag === 'SPAN' || tag === 'FONT') {
    const fam = el.style.fontFamily;
    if (fam) style.font = fam.replace(/^['"]|['"]$/g, '');
    const sizeRaw = el.style.fontSize;
    if (sizeRaw && sizeRaw.endsWith('px')) {
      const px = parseInt(sizeRaw, 10);
      // docx wants size in half-points; px -> pt at 96dpi is px * 0.75; half-points = pt * 2.
      if (!isNaN(px)) style.size = Math.round(px * 1.5);
    }
    const color = el.style.color;
    if (color) style.color = normalizeColor(color);
  }

  return collectInline(el, style);
}

function textRun(text: string, style: RunStyle): TextRun {
  return new TextRun({
    text,
    bold: style.bold,
    italics: style.italics,
    underline: style.underline,
    strike: style.strike,
    color: style.color,
    font: style.font,
    size: style.size,
  });
}

/** Normalize CSS color values to a 6-digit hex without leading `#`. */
function normalizeColor(raw: string): string | undefined {
  const trimmed = raw.trim();
  if (trimmed.startsWith('#')) {
    const hex = trimmed.slice(1);
    if (/^[0-9a-fA-F]{6}$/.test(hex)) return hex.toUpperCase();
    if (/^[0-9a-fA-F]{3}$/.test(hex)) {
      return hex.split('').map((c) => c + c).join('').toUpperCase();
    }
    return undefined;
  }
  const m = trimmed.match(/^rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)/i);
  if (m) {
    const [, r, g, b] = m;
    return [r, g, b]
      .map((n) => Math.min(255, parseInt(n, 10)).toString(16).padStart(2, '0'))
      .join('')
      .toUpperCase();
  }
  return undefined;
}
