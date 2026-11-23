import { describe, expect, it } from 'vitest';
import { folderLabel, locationOf, scanTargetFor } from './scanTargets';

describe('scanTargetFor', () => {
  it('turns a drive letter into a drive root', () => {
    expect(scanTargetFor('c')).toBe('C:');
    expect(scanTargetFor('D')).toBe('D:');
  });

  it('keeps folder paths unchanged', () => {
    expect(scanTargetFor('C:\\Users\\me\\Desktop')).toBe('C:\\Users\\me\\Desktop');
    expect(scanTargetFor('/Users/me')).toBe('/Users/me');
  });
});

describe('locationOf', () => {
  it('maps a drive root back to its letter and leaves folders alone', () => {
    expect(locationOf('C:')).toBe('C');
    expect(locationOf('c:\\')).toBe('C');
    expect(locationOf('C:\\Users\\me')).toBe('C:\\Users\\me');
    expect(locationOf('/Volumes/Data')).toBe('/Volumes/Data');
  });
});

describe('folderLabel', () => {
  it('names a folder by its last segment', () => {
    expect(folderLabel('C:\\Users\\me\\Desktop')).toBe('Desktop');
    expect(folderLabel('/Users/me/')).toBe('me');
    expect(folderLabel('/')).toBe('/');
  });
});
