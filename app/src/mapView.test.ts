import { describe, expect, it } from 'vitest';
import { parseMapView } from './mapView';

describe('parseMapView', () => {
  it('reads the stored view and falls back to the treemap', () => {
    expect(parseMapView('sunburst')).toBe('sunburst');
    expect(parseMapView('treemap')).toBe('treemap');
    expect(parseMapView(null)).toBe('treemap');
    expect(parseMapView('pie')).toBe('treemap');
  });
});
