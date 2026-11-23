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

  fastScan: 'Fast scan (admin)',
  fastScanAria: 'Fast scan (admin): restart as administrator and scan the selected drive quickly',
  waitingForPermission: 'Waiting for permission…',
  settings: 'Settings',
  alwaysElevated: 'Always run as administrator',
  alwaysElevatedHelp:
    'ByteForest will ask for permission each time it starts, so scans of NTFS drives are always fast.',
  settingsLoadFailed: 'Could not load settings',
  settingsSaveFailed: (reason: string) => `Could not save the setting: ${reason}`,

  // Elevation hint bar
  elevationBarText: (secs: string) =>
    `Scanned in ${secs} in standard mode. With admin rights it takes a few seconds.`,
  restartAsAdmin: 'Restart as administrator',
  badgeRestartHint: 'Click to restart with administrator rights for fast scanning',

  // Empty and scanning states
  emptyTitle: 'Explore your storage',
  emptyBody: 'Select a drive to explore its folders and largest files.',
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

  // Errors
  scanFailed: 'Scan failed',
  elevateFailed: 'Could not restart as administrator',
  dismiss: 'Dismiss',
} as const;
