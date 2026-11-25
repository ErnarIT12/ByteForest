import { useEffect, useRef, useState } from 'react';
import { applicationIcon } from '../api';
import { useAppStore } from '../store';
import { AppWindow, CodeXml, File, FileArchive, FileImage, FileMusic, FileText, FileVideo, Folder } from 'lucide-react';
import { Icon } from './Icon';

export function fileGlyph(name: string, directory: boolean) {
  if (directory) return Folder;
  const extension = name.includes('.') ? name.toLowerCase().split('.').at(-1) : '';
  switch (extension) {
    case 'mp4': case 'mkv': case 'mov': case 'avi': case 'webm': return FileVideo;
    case 'wav': case 'mp3': case 'aac': case 'flac': return FileMusic;
    case 'png': case 'jpeg': case 'jpg': case 'gif': case 'svg': case 'webp': return FileImage;
    case 'zip': case '7z': case 'rar': case 'tar': case 'gz': return FileArchive;
    case 'rs': case 'ts': case 'tsx': case 'js': case 'py': case 'cpp': return CodeXml;
    case 'exe': case 'msi': case 'dmg': return AppWindow;
    case 'pdf': case 'docx': case 'txt': case 'md': return FileText;
    default: return File;
  }
}

function ApplicationIcon({ node }: { node: number }) {
  const scan = useAppStore((s) => !s.isMac && s.resultSource.kind === 'live' ? s.resultScanId : null);
  const ref = useRef<HTMLSpanElement>(null);
  const [src, setSrc] = useState('');
  useEffect(() => {
    if (scan === null || !ref.current) return;
    let alive = true;
    const observer = new IntersectionObserver(([entry]) => {
      if (!entry.isIntersecting) return;
      observer.disconnect();
      void applicationIcon(scan, node).then((bytes) => {
        if (alive) setSrc('data:image/png;base64,' + btoa(Array.from(new Uint8Array(bytes), (n) => String.fromCharCode(n)).join('')));
      }).catch(() => {});
    });
    observer.observe(ref.current);
    return () => { alive = false; observer.disconnect(); };
  }, [scan, node]);
  return <span ref={ref} className="kind-icon" aria-hidden="true">{src
    ? <img src={src} width="16" height="16" alt="" onError={() => setSrc('')} />
    : <Icon icon={AppWindow} />}</span>;
}

export function FileKindIcon({ name, directory, node }: { name: string; directory: boolean; node: number }) {
  if (!directory && name.toLowerCase().endsWith('.exe')) return <ApplicationIcon key={node} node={node} />;
  return <Icon icon={fileGlyph(name, directory)} className={`kind-icon ${directory ? 'folder' : 'file'}`} />;
}
