/**
 * Types that mirror the engine's serialised models.
 *
 * The Rust side serialises with `rename_all = "camelCase"`, so these are a
 * direct transcription. They are kept in one file, next to the IPC wrappers, so
 * there is exactly one place to update when a model changes.
 */

export type EntryKind = "file" | "directory" | "symlink" | "other";

export type Category =
  | "unknown"
  | "applications"
  | "documents"
  | "media"
  | "downloads"
  | "developer"
  | "caches"
  | "system"
  | "other";

export type ScanStatus = "running" | "completed" | "cancelled" | "failed" | "interrupted";

export type ScanErrorKind =
  "permissionDenied" | "notFound" | "vanished" | "traversalLoop" | "crossDevice" | "io";

export type VolumeKind = "internal" | "external" | "network" | "virtual" | "unknown";

export interface VolumeInfo {
  id: string;
  name: string;
  mountPoint: string;
  device: string | null;
  fsType: string;
  kind: VolumeKind;
  totalBytes: number | null;
  availableBytes: number | null;
  usedBytes: number | null;
  isRemovable: boolean;
  isReadOnly: boolean;
  isBrowsable: boolean;
  isReadable: boolean;
}

export interface EntryRow {
  id: number;
  parentId: number | null;
  name: string;
  path: string;
  kind: EntryKind;
  depth: number;
  /** Bytes attributed to this row: the subtree total for a folder. */
  size: number;
  allocatedSize: number;
  fileCount: number;
  dirCount: number;
  extension: string | null;
  modifiedAt: number | null;
  category: Category;
  isHidden: boolean;
  /** False when DiskMate could not read into this folder. Not the same as empty. */
  accessible: boolean;
  shareOfParent: number;
  /** True when the bytes are shared with another hard link to the same file. */
  sharesStorage: boolean;
}

export interface ScanSummary {
  scanId: number;
  volumeId: number | null;
  rootPath: string;
  status: ScanStatus;
  startedAt: number;
  finishedAt: number | null;
  durationMs: number;
  fileCount: number;
  dirCount: number;
  symlinkCount: number;
  otherCount: number;
  errorCount: number;
  logicalBytes: number;
  allocatedBytes: number;
  entriesPerSecond: number;
  indexBytes: number;
  engineVersion: string;
}

export interface ScanProgress {
  scanId: number;
  status: ScanStatus;
  entriesScanned: number;
  filesScanned: number;
  directoriesScanned: number;
  symlinksScanned: number;
  bytesIndexed: number;
  errors: number;
  elapsedMs: number;
  entriesPerSecond: number;
  currentPath: string | null;
}

export interface ScanFinished {
  root: string;
  status: ScanStatus;
  summary: ScanSummary | null;
  error: string | null;
}

export interface CategoryBucket {
  category: Category;
  label: string;
  fileCount: number;
  totalSize: number;
}

export interface ExtensionBucket {
  extension: string | null;
  fileCount: number;
  totalSize: number;
}

export interface ErrorBucket {
  kind: ScanErrorKind;
  count: number;
  explanation: string;
}

export interface StorageSnapshot {
  scan: ScanSummary;
  root: EntryRow;
  largestFolders: EntryRow[];
  largestFiles: EntryRow[];
  categories: CategoryBucket[];
  extensions: ExtensionBucket[];
  errors: ErrorBucket[];
}

export type SortKey = "size" | "name" | "modified" | "items";
export type SortDirection = "asc" | "desc";

export interface ChildQuery {
  scanId: number;
  parentId: number;
  sort: SortKey;
  direction: SortDirection;
  limit: number;
  offset: number;
  hideHidden: boolean;
}

export interface ChildPage {
  rows: EntryRow[];
  total: number;
}

export interface AppInfo {
  version: string;
  engine: string;
  platform: string;
  os: string;
  arch: string;
  dataDir: string;
  databasePath: string;
  /** "installed" | "portable" | "explicit" */
  storageMode: string;
  portableFallbackReason: string | null;
}

export type MemoryPressure = "normal" | "warning" | "critical" | "unknown";

export interface MemorySnapshot {
  totalBytes: number;
  usedBytes: number;
  cachedBytes: number;
  freeBytes: number;
  wiredBytes: number | null;
  compressedBytes: number | null;
  swapUsedBytes: number | null;
  swapTotalBytes: number | null;
  pressure: MemoryPressure;
  sampledAt: number;
}

export interface MemoryReport {
  snapshot: MemorySnapshot;
  headline: string;
  detail: string;
}

export type StartupSource =
  | "launchAgent"
  | "launchDaemon"
  | "loginItem"
  | "userService"
  | "systemService"
  | "autostart"
  | "unknown";

export type StartupScope = "user" | "system" | "os";
export type StartupImpact = "unmeasured" | "low" | "medium" | "high";

export interface StartupItem {
  id: string;
  name: string;
  source: StartupSource;
  scope: StartupScope;
  configPath: string;
  program: string | null;
  runsAtLoad: boolean | null;
  disabled: boolean | null;
  impact: StartupImpact;
  isOsProvided: boolean;
}

export interface StartupReport {
  items: StartupItem[];
  userCount: number;
  systemCount: number;
  osCount: number;
  impactUnavailable: boolean;
}

// ------------------------------------------------------------- cleanup --

export type CandidateKind =
  | "trash"
  | "cache"
  | "developerArtifact"
  | "oldDownload"
  | "bigAndUntouched"
  | "duplicateCopy"
  | "appLeftover"
  | "appBundle"
  | "log"
  | "other";

export type CandidateState =
  "proposed" | "staged" | "removed" | "refused" | "failed" | "vanished";

export type SafetyClass = "safe" | "review" | "protected";

export interface CleanupCandidate {
  id: number;
  scanId: number;
  entryId: number | null;
  path: string;
  name: string;
  kind: CandidateKind;
  safety: SafetyClass;
  rule: string;
  /** Why this was proposed, in a sentence. Never empty. */
  reason: string;
  detail: string | null;
  bytes: number;
  modifiedAt: number | null;
  state: CandidateState;
  groupKey: string | null;
}

export interface CandidateGroup {
  kind: CandidateKind;
  label: string;
  safety: SafetyClass;
  count: number;
  bytes: number;
  summary: string;
}

/** Exactly what the user reviewed. The engine refuses if it no longer matches. */
export interface Confirmation {
  candidateIds: number[];
  expectedTotalBytes: number;
}

export interface RemovalOutcome {
  candidateId: number;
  path: string;
  state: CandidateState;
  bytes: number;
  detail: string | null;
}

export interface RemovalReport {
  outcomes: RemovalOutcome[];
  removedCount: number;
  removedBytes: number;
  refusedCount: number;
  failedCount: number;
}

// ---------------------------------------------------------- duplicates --

export interface DuplicateMember {
  entryId: number;
  path: string;
  name: string;
  modifiedAt: number | null;
  isKeeper: boolean;
}

export interface DuplicateGroup {
  id: number;
  logicalSize: number;
  memberCount: number;
  reclaimable: number;
  members: DuplicateMember[];
}

export interface DuplicateStats {
  sizeCandidates: number;
  partiallyHashed: number;
  fullyHashed: number;
  groups: number;
  reclaimableBytes: number;
  bytesRead: number;
}

export interface DuplicateProgress {
  stage: string;
  done: number;
  total: number;
}

// -------------------------------------------------------- applications --

export interface Application {
  id: number;
  name: string;
  bundleId: string | null;
  path: string;
  version: string | null;
  bundleBytes: number;
  leftoverBytes: number;
  leftoverCount: number;
  lastUsedAt: number | null;
  isOsProvided: boolean;
}

export interface Leftover {
  path: string;
  location: string;
  bytes: number;
  safety: SafetyClass;
  reason: string;
}

// ----------------------------------------------------------- snapshots --

export interface Snapshot {
  id: number;
  scanId: number;
  label: string;
  rootPath: string;
  capturedAt: number;
  indexedBytes: number;
  entryCount: number;
  fileCount: number;
  dirCount: number;
  totalBytes: number | null;
  usedBytes: number | null;
}

export type ChangeKind = "grew" | "shrank" | "added" | "removed";

export interface Delta {
  path: string;
  name: string;
  change: ChangeKind;
  beforeBytes: number;
  afterBytes: number;
  deltaBytes: number;
}

export interface Comparison {
  id: number;
  base: Snapshot;
  target: Snapshot;
  bytesAdded: number;
  bytesRemoved: number;
  entriesAdded: number;
  entriesRemoved: number;
  netBytes: number;
  deltas: Delta[];
}

// ------------------------------------------------------------- monitor --

export interface Sample {
  at: number;
  memoryUsedBytes: number;
  memoryCachedBytes: number;
  memoryTotalBytes: number;
  swapUsedBytes: number | null;
  pressure: MemoryPressure;
  diskAvailableBytes: number | null;
}

export interface MonitorReport {
  running: boolean;
  intervalMs: number;
  samples: Sample[];
  latest: Sample | null;
  /** Highest pressure across the window, so a spike that passed is not lost. */
  peakPressure: MemoryPressure;
  headline: string;
  detail: string;
}

// ----------------------------------------------------------- age & map --

export interface AgeBucket {
  label: string;
  fromDays: number;
  toDays: number;
  fileCount: number;
  totalSize: number;
}
