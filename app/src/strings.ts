/** English UI strings (v0.1 ships English only). */
export const strings = {
  appName: 'ByteForest',

  // Top bar
  loadingDrives: 'Loading drives…',
  noDrives: 'No drives found',
  scan: 'Scan',
  rescan: 'Rescan',
  cancel: 'Cancel',
  scanAria: 'Scan the selected drive',
  cancelAria: 'Cancel the running scan',
  phase: {
    Reading: 'Reading MFT',
    Parsing: 'Parsing',
    Walking: 'Walking folders',
    Building: 'Building tree',
  },
  starting: 'Starting',
  files: 'files',

  fastScan: 'Fast scan (admin)',
  fastScanAria: 'Fast scan (admin): restart as administrator and scan the selected drive quickly',
  waitingForPermission: 'Waiting for permission…',
  settings: 'Settings',
  alwaysElevated: 'Always run as administrator',
  alwaysElevatedHelp:
    'ByteForest will ask for permission each time it starts, so scans of NTFS drives are always fast.',
  settingsLoadFailed: 'Could not load settings',
  settingsSaveFailed: (reason: string) => `Could not save the setting: ${reason}`,

  // Scan exclusions
  exclusionsTitle: 'Excluded from scans',
  exclusionsEmpty: 'Nothing is excluded.',
  exclusionPlaceholder: 'node_modules, *.tmp or a full path',
  exclusionInputLabel: 'Name, wildcard or full path to exclude',
  exclusionAdd: 'Add',
  exclusionRemove: (pattern: string) => `Remove ${pattern}`,
  exclusionTooLong: 'That pattern is too long (260 characters at most).',
  exclusionTooMany: 'Too many patterns (200 at most).',
  exclusionsHelp:
    'A name or wildcard (node_modules, *.tmp, $RECYCLE.BIN) matches anywhere; * stands for any text and ? for one character. A full path (C:\\Windows\\WinSxS, /Users/me/Library/Caches) matches only that folder.',
  exclusionsNextScan: 'Applies to the next scan.',
  excludedSummary: (count: number, items: string, size: string) =>
    count === 1 ? `Excluded 1 item · ${size}` : `Excluded ${items} items · ${size}`,
  excludedHint: 'Left out of this scan by your exclusion settings.',

  // Report export
  exportReport: 'Export report…',
  reportExported: (path: string) => `Report saved to ${path}`,
  reportExportFailed: 'Could not export the report',

  // Elevation hint bar
  elevationBarText: (secs: string) =>
    `Scanned in ${secs} in standard mode. With admin rights it takes a few seconds.`,
  restartAsAdmin: 'Restart as administrator',
  badgeRestartHint: 'Click to restart with administrator rights for fast scanning',

  // Empty and scanning states
  emptyTitle: 'Explore your storage',
  emptyBody: 'Select a drive to explore its folders and largest files.',
  emptyBodyMac: 'Choose a volume or folder to see what takes up space.',
  emptyChooseLocation: 'Pick a location in the sidebar to begin.',
  emptySelected: 'Selected location',
  volumeUsed: 'Used',
  volumeAvailable: 'Available',
  scanningTitle: (drive: string) => `Scanning ${drive}`,

  // Tree
  treeLabel: 'Folders and files',
  colName: 'Name',
  colSize: 'Size',
  colPercent: '% of parent',
  colFiles: 'Files',
  colModified: 'Modified',
  loadingMore: 'Loading…',
  loadFailed: 'Could not load this folder',
  expand: 'Expand',
  collapse: 'Collapse',

  // Treemap and breadcrumbs
  treemapLabel: 'Treemap of the focused folder',
  treemapRoleDescription: 'treemap',
  treemapSelected: (name: string, kind: string, size: string) => `${name}, ${kind}, ${size}`,
  breadcrumbsLabel: 'Path of the focused folder',
  up: 'Up one level',
  ofView: 'of view',
  folder: 'Folder',
  file: 'File',
  treemapEmpty: 'This folder is empty',

  // Splitter
  splitterLabel: 'Resize the disk map',

  // Status bar
  ready: 'Ready',
  statusSummary: (files: string, dirs: string, secs: string, method: string) =>
    `${files} files · ${dirs} folders in ${secs} · ${method}`,
  methodName: { Mft: 'MFT', Walk: 'Walk' },
  used: (used: string, total: string) => `${used} used of ${total}`,
  admin: 'Admin',
  adminHint: 'Running as administrator: the MFT is read directly for the fastest scan.',
  standard: 'Standard mode (slower)',
  standardHint:
    'Running without administrator rights: folders are walked one by one, which is slower.',
  mftFallbackHint: (reason: string) =>
    `Running as administrator, but the MFT was read the slower way: ${reason}`,
  scanIssues: (n: string) => `${n} unreadable`,
  scanIssuesHint: 'Entries that could not be read were skipped.',

  // Screen reader announcements
  announceStart: (drive: string) => `Scanning ${drive}`,
  announceDone: (files: string, secs: string) => `Scan finished: ${files} files in ${secs}`,
  announceCancelled: 'Scan cancelled',
  announceFailed: 'Scan failed',

  // Context menu and file actions
  fileActionsLabel: 'Actions for the selected item',
  openFolder: 'Open folder',
  showInExplorer: 'Show in Explorer',
  showInFinder: 'Show in Finder',
  copyPath: 'Copy Path',
  pathCopied: 'Path copied',
  moveToRecycleBin: 'Move to Recycle Bin',
  moveToTrash: 'Move to Trash',
  trashBinName: (isMac: boolean) => (isMac ? 'Trash' : 'Recycle Bin'),
  confirmTrashTitle: (bin: string) => `Move to ${bin}?`,
  confirmTrashFolderContents: (files: string, dirs: string) =>
    `Contains ${files} files and ${dirs} folders.`,
  confirmTrashBody: (bin: string) =>
    `It will be moved to the ${bin}. You can restore it from there. Disk space is freed once the ${bin} is emptied.`,
  confirmTrashSize: (size: string) => `Size: ${size}`,
  movedToTrash: (name: string, bin: string) => `Moved "${name}" to the ${bin}`,
  trashFailed: (reason: string) => `Could not move the item: ${reason}`,
  noNativePath: 'This entry has no usable filesystem path.',
  working: 'Working…',

  // File types
  colorModeLabel: 'Treemap colors',
  colorModeFolders: 'Folders',
  colorModeTypes: 'File types',
  legendLabel: 'File type colors',
  categoryNames: {
    video: 'Video',
    archive: 'Archives',
    diskimage: 'Disk images',
    codebuild: 'Code and build',
    media: 'Audio and images',
    system: 'System',
    other: 'Other',
  },
  fileTypes: 'File types',
  fileTypesLoading: 'Loading file types…',
  fileTypesFailed: 'Could not load file types',
  topExtensions: 'Top extensions',
  noExtension: 'No extension',
  otherExtensions: 'Other extensions',
  searchCategory: 'File type',
  searchAnyType: 'All types',
  searchInFolder: (name: string) => `in ${name}`,
  searchClearScope: 'Search everywhere',
  searchScopeLabel: 'Search scope',
  searchPresetsLabel: 'Quick searches',
  searchPresetNames: {
    node_modules: 'node_modules',
    target: 'target',
    venv: '.venv',
    git: '.git',
    diskImages: 'Disk images',
    huge: 'Over 1 GB',
  },
  searchFoldersHint: 'Searching folders by name. File type filters do not apply.',
  searchFilesInstead: 'Search files',
  searchFolderMarker: 'Folder',
  searchNone: (folders: boolean) => (folders ? 'No matching folders' : 'No matching files'),
  searchSummary: (shown: string, total: string, noun: string, size: string) =>
    shown === total ? `${total} ${noun} · ${size}` : `Showing ${shown} of ${total} ${noun} · ${size}`,
  searchNouns: { files: 'files', folders: 'folders' },
  folderMetadata: 'Folder metadata',
  breakdownRow: (name: string, size: string, share: string) => `${name}, ${size}, ${share}`,
  tooltipCategory: (name: string) => `Type: ${name}`,

  // Snapshots and comparison
  snapshots: 'Snapshots',
  snapshotsMenuLabel: 'Snapshot actions',
  snapshotSave: 'Save snapshot…',
  snapshotCompare: 'Compare with snapshot…',
  snapshotStop: 'Stop comparing',
  snapshotSaved: (path: string) => `Snapshot saved to ${path}`,
  snapshotSaveFailed: 'Could not save the snapshot',
  snapshotCompareFailed: 'Could not compare with the snapshot',
  comparingWith: (label: string, date: string, net: string) =>
    `Comparing with ${label} · ${date} · net ${net}`,
  comparingSummary: (added: string, removed: string) => `${added} new · ${removed} gone`,
  stopComparing: 'Stop',
  compareBannerLabel: 'Snapshot comparison',
  colChange: 'Change',
  diffNew: 'New',
  changesMode: 'Changes',
  changesLoading: 'Loading changes…',
  changesNone: 'Nothing grew or is new since the snapshot',
  changesSummary: (n: string) => `${n} biggest increases since the snapshot`,
  changesFailed: 'Could not load the changes',
  sinceSnapshot: 'Since snapshot',
  goneSinceSnapshot: 'Gone since snapshot',
  goneSummary: (count: string, size: string) => `${count} items · ${size}`,

  // Sidebar
  sidebarLabel: 'Locations and recent scans',
  sidebarCollapse: 'Collapse sidebar',
  sidebarExpand: 'Expand sidebar',
  locationsTitle: 'Locations',
  chooseFolder: 'Choose folder…',
  chooseFolderFailed: 'Unable to choose a folder',
  driveFree: (free: string) => `${free} free`,
  driveUsageLabel: (percent: string) => `${percent} used`,
  userFolderNames: { desktop: 'Desktop', downloads: 'Downloads', documents: 'Documents' },
  recentTitle: 'Recent Scans',
  recentEmpty: 'Scans you run will show up here.',
  recentRescan: (label: string) => `Scan ${label} again`,
  discardTitle: 'Discard Pile',
  discardEmpty: 'Queue files here to clean up in one go',
  discardEmptyHint: 'Add files or folders from the tree or the map, then move them all at once.',
  addToDiscardPile: 'Add to Discard Pile',
  removeFromDiscardPile: 'Remove from Discard Pile',
  inDiscardPile: 'In Discard Pile',
  discardSummary: (count: string, size: string, one: boolean) =>
    `${count} ${one ? 'item' : 'items'} · ${size}`,
  discardListLabel: 'Items in the Discard Pile',
  discardRemoveItem: (name: string) => `Remove ${name} from Discard Pile`,
  discardShowItem: (name: string, path: string) => `Show ${name} in the folder tree. ${path}`,
  discardClear: 'Clear',
  discardMoveAll: (isMac: boolean) =>
    isMac ? 'Move All to Trash…' : 'Move All to Recycle Bin…',
  discardBadge: (count: string) => `${count} in the Discard Pile`,
  discardConfirmTitle: (count: string, bin: string, one: boolean) =>
    `Move ${count} ${one ? 'item' : 'items'} to the ${bin}?`,
  discardConfirmBody: (bin: string) =>
    `They will be moved to the ${bin}. You can restore them from there. Disk space is freed once the ${bin} is emptied.`,
  discardMoving: 'Moving…',
  discardMovingStatus: (count: string) => `Moving ${count} items…`,
  discardResult: (moved: string, size: string, failed: string, movedOne: boolean) => {
    const head = moved === '0'
      ? 'Nothing was moved.'
      : `Moved ${moved} ${movedOne ? 'item' : 'items'} (${size}).`;
    return failed === '0' ? head : `${head} ${failed} couldn't be moved.`;
  },
  discardFailedList: (failed: string) => `Show the ${failed} that couldn't be moved`,
  discardFailedLabel: 'Items that could not be moved',
  discardFailedItem: (name: string, reason: string) => `${name}: ${reason}`,
  discardBatchFailed: (reason: string) => `Could not move the items: ${reason}`,

  // View switch
  mapViewLabel: 'Disk map',
  mapViewTreemap: 'Treemap',
  mapViewSunburst: 'Sunburst',
  mapViewSoon: 'Coming in a later update',
  topbarTargetLabel: (target: string) => `Scan target: ${target}`,

  // Errors
  scanFailed: 'Scan failed',
  elevateFailed: 'Could not restart as administrator',
  dismiss: 'Dismiss',
} as const;
