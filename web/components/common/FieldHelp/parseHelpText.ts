/**
 * Parses the plain-text help copy into blocks the tooltip can render.
 *
 * The copy is stored as one string — the same string the upstream product
 * ships — rather than pre-split into JSX at every call site. Two block kinds
 * are recognised:
 *
 * - a blank line separates paragraphs;
 * - a paragraph whose lines all start with `- ` is a bullet list.
 *
 * Kept separate from the component so the parsing is testable without a DOM.
 */

export interface HelpParagraph {
  kind: 'paragraph';
  /** Inline markup left intact; the renderer interprets it. */
  text: string;
}

export interface HelpBulletList {
  kind: 'list';
  items: string[];
}

export type HelpBlock = HelpParagraph | HelpBulletList;

export const parseHelpText = (text: string): HelpBlock[] =>
  text
    .split('\n\n')
    .map((block) => block.trim())
    .filter(Boolean)
    .map((block) =>
      block.startsWith('- ')
        ? { kind: 'list' as const, items: block.split('\n').map((line) => line.replace(/^-\s*/, '')) }
        : { kind: 'paragraph' as const, text: block },
    );

/**
 * Splits a line into plain runs, `**bold**` runs and `` `code` `` runs.
 *
 * Deliberately minimal: the input is a translation string, not user content, so
 * there is no HTML, no links and no escaping to worry about.
 */
export type InlineRun =
  | { kind: 'text'; text: string }
  | { kind: 'bold'; text: string }
  | { kind: 'code'; text: string };

export const parseHelpInline = (text: string): InlineRun[] =>
  text
    .split(/(\*\*[^*]+\*\*|`[^`]+`)/g)
    .filter(Boolean)
    .map((part) => {
      if (part.startsWith('**') && part.endsWith('**')) {
        return { kind: 'bold' as const, text: part.slice(2, -2) };
      }
      if (part.startsWith('`') && part.endsWith('`')) {
        return { kind: 'code' as const, text: part.slice(1, -1) };
      }
      return { kind: 'text' as const, text: part };
    });
