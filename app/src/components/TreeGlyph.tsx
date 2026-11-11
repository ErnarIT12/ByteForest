/** Small tree mark used next to the app name and in the empty state. */
export function TreeGlyph({ size = 20 }: { size?: number }) {
  return (
    <svg className="tree-glyph" width={size} height={size} viewBox="0 0 24 24" aria-hidden="true">
      <rect x="2" y="2" width="20" height="20" rx="5" className="tree-glyph-bg" />
      <path d="M12 5.5l-4.5 6h2.6L7 15.5h10l-3.1-4h2.6z" className="tree-glyph-leaf" />
      <rect x="11" y="15.5" width="2" height="3.5" rx="0.6" className="tree-glyph-trunk" />
    </svg>
  );
}
