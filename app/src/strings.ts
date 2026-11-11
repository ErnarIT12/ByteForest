/** English UI strings (v0.1 ships English only). */
export const strings = {
  appName: 'ByteForest',

  // Top bar
  driveLabel: 'Drive to scan',
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

  // Empty and scanning states
  emptyTitle: 'See what takes up space',
  emptyBody: 'Pick a drive and scan it to see what takes up space.',
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
  breadcrumbsLabel: 'Path of the focused folder',
  up: 'Up one level',
  ofView: 'of view',
  folder: 'Folder',
  file: 'File',
  treemapEmpty: 'This folder is empty',

  // Splitter
  splitterLabel: 'Resize the folder tree',

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

  // Errors
  scanFailed: 'Scan failed',
  dismiss: 'Dismiss',
} as const;
