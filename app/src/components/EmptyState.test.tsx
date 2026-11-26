import { renderToStaticMarkup } from 'react-dom/server';
import { expect, it } from 'vitest';
import { VolumeUsage } from './EmptyState';

it.each([[0, 100], [25, 75], [100, 0], [110, 0]])(
  'shows the same used percentage in the ring and text with %i free',
  (free, percent) => {
    const html = renderToStaticMarkup(<VolumeUsage drive={{ letter: 'C', label: '', fs: 'NTFS', total: 100, free }} />);
    expect(html).toContain(`<strong>${percent}%</strong>`);
    expect(html).toContain(`stroke-dashoffset="${100 - percent}"`);
  },
);
