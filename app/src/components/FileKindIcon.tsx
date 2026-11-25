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

export function FileKindIcon({ name, directory }: { name: string; directory: boolean }) {
  return <Icon icon={fileGlyph(name, directory)} className={`kind-icon ${directory ? 'folder' : 'file'}`} />;
}
