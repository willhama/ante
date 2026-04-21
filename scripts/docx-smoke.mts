// DOCX import/export smoke test for ante.
//
// Run with: pnpm dlx tsx scripts/docx-smoke.mts
//
// We exercise:
//   - htmlToDocxBytes (export): TipTap-style HTML -> docx bytes, written to
//     disk, ZIP magic verified, then read back via mammoth's Node {buffer}
//     entrypoint and asserted feature-by-feature.
//   - mammoth import on a textutil-generated .docx: proves we can read
//     real-world Word output. The thin docxBytesToHtml wrapper isn't called
//     here because mammoth's Node bundle doesn't accept arrayBuffer (browser
//     -only API), so we call mammoth the same way the wrapper would.

import { parseHTML } from 'linkedom';
import mammoth from 'mammoth';
import fs from 'node:fs';
import path from 'node:path';

class LinkedomDOMParser {
  parseFromString(html: string, _type: string) {
    const { document } = parseHTML(html);
    return document;
  }
}
// @ts-expect-error injecting browser global into node
globalThis.DOMParser = LinkedomDOMParser;
// @ts-expect-error linkedom nodes use numeric nodeType but no Node global
globalThis.Node = { TEXT_NODE: 3, ELEMENT_NODE: 1 };
if (!globalThis.atob) {
  // @ts-expect-error
  globalThis.atob = (s: string) => Buffer.from(s, 'base64').toString('binary');
}
if (!globalThis.btoa) {
  // @ts-expect-error
  globalThis.btoa = (s: string) => Buffer.from(s, 'binary').toString('base64');
}

const anteRoot = '/Users/marc/Code/open-source/ante';
const docxModule = await import(path.join(anteRoot, 'src/lib/io/docx.ts'));
const { htmlToDocxBytes } = docxModule;

const SAMPLE_HTML = fs.readFileSync('/tmp/ante-smoke/sample.html', 'utf8');

function check(label: string, ok: boolean): boolean {
  console.log(`  ${ok ? 'OK  ' : 'FAIL'} ${label}`);
  return ok;
}

async function testExportRoundTrip(): Promise<void> {
  console.log('\n=== EXPORT: sample HTML -> .docx ===');
  const b64 = await htmlToDocxBytes(SAMPLE_HTML);
  const buf = Buffer.from(b64, 'base64');
  const out = '/tmp/ante-smoke/exported.docx';
  fs.writeFileSync(out, buf);
  console.log(`  wrote ${out} (${buf.length} bytes)`);

  if (!(buf[0] === 0x50 && buf[1] === 0x4b && buf[2] === 0x03 && buf[3] === 0x04)) {
    throw new Error('output is not a valid ZIP (no PK magic bytes)');
  }
  console.log('  ZIP magic OK');

  console.log('\n=== ROUND-TRIP: read exported.docx via mammoth ===');
  const result = await mammoth.convertToHtml({ buffer: buf });
  console.log('  resulting HTML:');
  console.log(result.value.split('\n').map((l) => '    ' + l).join('\n'));
  console.log(`  warnings: ${result.messages.length}`);

  const html = result.value;
  let failed = 0;
  if (!check('h1 heading',     /<h1[^>]*>[^<]*Ante DOCX Smoke Test/.test(html))) failed++;
  if (!check('h2 heading',     /<h2[^>]*>[^<]*Subheading/.test(html))) failed++;
  if (!check('bold survives',  /<strong>bold<\/strong>/.test(html))) failed++;
  if (!check('italic survives',/<em>italic<\/em>/.test(html))) failed++;
  if (!check('hyperlink',      /<a href="https:\/\/example\.com">a link<\/a>/.test(html))) failed++;
  if (!check('bullet list',    /<ul>[\s\S]*First bullet[\s\S]*<\/ul>/.test(html))) failed++;
  if (!check('numbered list',  /<ol>[\s\S]*One[\s\S]*Two[\s\S]*Three[\s\S]*<\/ol>/.test(html))) failed++;
  if (failed > 0) throw new Error(`${failed} round-trip assertion(s) failed`);
}

async function testImportOfRealWordDoc(): Promise<void> {
  console.log('\n=== IMPORT: textutil-generated .docx -> HTML ===');
  // Note: macOS textutil emits a flatter DOCX than real Word - it represents
  // headings as bold paragraphs and bullets as tab-prefixed text rather than
  // real Heading/List styles. So we only assert that text + inline marks
  // survive. The round-trip test above is the stronger fidelity proof.
  const buf = fs.readFileSync('/tmp/ante-smoke/wordlike.docx');
  const result = await mammoth.convertToHtml({ buffer: buf });
  console.log('  resulting HTML:');
  console.log(result.value.split('\n').map((l) => '    ' + l).join('\n'));
  console.log(`  warnings: ${result.messages.length}`);

  const html = result.value;
  let failed = 0;
  if (!check('title text present',   /From Word-ish/.test(html))) failed++;
  if (!check('bold survived',        /<strong>bold<\/strong>/.test(html))) failed++;
  if (!check('italic survived',      /<em>italic<\/em>/.test(html))) failed++;
  if (!check('list items present',   /Alpha[\s\S]*Beta/.test(html))) failed++;
  if (failed > 0) throw new Error(`${failed} real-Word-doc import assertion(s) failed`);
}

async function main() {
  await testExportRoundTrip();
  await testImportOfRealWordDoc();
  console.log('\n=== ALL OK ===');
}

main().catch((e) => {
  console.error('\n=== FAIL ===\n', e);
  process.exit(1);
});
