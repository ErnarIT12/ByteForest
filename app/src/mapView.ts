/** Which disk map is shown. Sunburst arrives in a later update. */
export type MapView = 'treemap' | 'sunburst';

export const MAP_VIEW_KEY = 'byteforest.mapView';

export function parseMapView(raw: string | null): MapView {
  return raw === 'sunburst' ? 'sunburst' : 'treemap';
}

export function loadMapView(): MapView {
  try {
    return parseMapView(localStorage.getItem(MAP_VIEW_KEY));
  } catch {
    return 'treemap';
  }
}

export function saveMapView(view: MapView): void {
  try {
    localStorage.setItem(MAP_VIEW_KEY, view);
  } catch {
    /* Applies to this session only. */
  }
}
