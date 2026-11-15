import logo from '../assets/logo.png';

/** The app logo, shown next to the app name and in the empty state. */
export function TreeGlyph({ size = 26 }: { size?: number }) {
  return <img className="brand-logo" src={logo} width={size} height={size} alt="" />;
}
