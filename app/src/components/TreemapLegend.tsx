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

/** Key of the change marks of the map, shown while comparing with a snapshot. */
export function DiffLegend() {
  const items = [
    ['grown', strings.diffLegendGrown],
    ['added', strings.diffLegendNew],
    ['shrunk', strings.diffLegendShrunk],
  ] as const;
  return (
    <ul className="treemap-legend diff-legend" aria-label={strings.diffLegendLabel}>
      {items.map(([tone, text]) => (
        <li key={tone}>
          <span className="legend-mark" data-tone={tone} aria-hidden="true" />
          {text}
        </li>
      ))}
    </ul>
  );
}
