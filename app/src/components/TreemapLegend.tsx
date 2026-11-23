import { useColorScheme } from '../hooks/useColorScheme';
import { strings } from '../strings';
import { CATEGORIES, categoryColor } from '../treemapPaint';

/** Names the file-type colours of the treemap (shown in the "File types" mode). */
export function TreemapLegend() {
  const theme = useColorScheme();
  return (
    <ul className="treemap-legend" aria-label={strings.legendLabel}>
      {CATEGORIES.map((category) => (
        <li key={category}>
          <span
            className="legend-swatch"
            aria-hidden="true"
            style={{ background: categoryColor(theme, category, 'file') }}
          />
          {strings.categoryNames[category]}
        </li>
      ))}
    </ul>
  );
}
