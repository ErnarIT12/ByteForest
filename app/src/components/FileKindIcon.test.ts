import { expect, it } from 'vitest';
import { File, FileVideo, Folder } from 'lucide-react';
import { fileGlyph } from './FileKindIcon';

it('recognizes mixed-case extensions and respects directories and extensionless files', () => {
  expect(fileGlyph('clip.Mp4', false)).toBe(FileVideo);
  expect(fileGlyph('clip.mp4', true)).toBe(Folder);
  expect(fileGlyph('mp4', false)).toBe(File);
});
