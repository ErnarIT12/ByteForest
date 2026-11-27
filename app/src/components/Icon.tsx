import type { LucideIcon, LucideProps } from 'lucide-react';

/** The icons the app uses, re-exported so the bundle only contains these glyphs. */
export {
  Activity,
  ArrowUp,
  Box,
  Check,
  ChevronRight,
  CircleAlert,
  Clock,
  Copy,
  Cpu,
  Database,
  Disc,
  Download,
  Ellipsis,
  File,
  FileText,
  Folder,
  FolderOpen,
  GitBranch,
  HardDrive,
  History,
  House,
  Layers,
  LayoutGrid,
  MemoryStick,
  Minus,
  Monitor,
  Package,
  PanelLeftClose,
  PanelLeftOpen,
  Search,
  Settings,
  ShieldCheck,
  Square,
  Trash2,
  X,
} from 'lucide-react';
export type { LucideIcon };

interface IconProps extends Omit<LucideProps, 'ref' | 'aria-label'> {
  icon: LucideIcon;
  /** Accessible name; icons without one are hidden from assistive technology. */
  label?: string;
}

/**
 * One icon style for the whole app: 16 px, a 1.5 px stroke whatever the size, decorative
 * unless `label` is given.
 */
export function Icon({ icon: Glyph, label, size = 16, strokeWidth = 1.5, ...rest }: IconProps) {
  return (
    <Glyph
      size={size}
      strokeWidth={strokeWidth}
      absoluteStrokeWidth
      aria-hidden={label ? undefined : true}
      aria-label={label}
      role={label ? 'img' : undefined}
      focusable="false"
      {...rest}
    />
  );
}
