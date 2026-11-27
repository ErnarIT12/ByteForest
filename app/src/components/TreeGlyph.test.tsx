import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { TreeGlyph } from './TreeGlyph';

describe('TreeGlyph', () => {
  it('gives every rendered logo its own gradient ids', () => {
    const html = renderToStaticMarkup(
      <>
        <TreeGlyph />
        <TreeGlyph size={100} />
      </>,
    );
    const ids = [...html.matchAll(/ id="([^"]+)"/g)].map((m) => m[1]);
    expect(ids.length).toBeGreaterThan(0);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it('is decorative and takes colors from theme tokens', () => {
    const html = renderToStaticMarkup(<TreeGlyph size={40} />);
    expect(html).toContain('aria-hidden="true"');
    expect(html).toContain('width="40"');
    expect(html).toContain('var(--logo-tile)');
    expect(html).not.toMatch(/stop-color="#/);
  });

  it('omits the square tile when rendered frameless for the sidebar', () => {
    const html = renderToStaticMarkup(<TreeGlyph size={18} frameless />);
    expect(html).toContain('width="18"');
    expect(html).toContain('bf-logo-frameless');
    expect(html).not.toContain('var(--logo-tile)');
  });
});
