import { ConfigSpec } from 'src/app/pkg-config/config-types'
import { Url } from '@start9labs/shared'
import { MarketplaceManifest } from '@start9labs/marketplace'
import { BasicInfo } from 'src/app/pages/developer-routes/developer-menu/form-info'
import { PackageState } from 'src/app/types/package-state'
import { InstallProgress } from 'src/app/types/install-progress'

export interface DataModel {
  'server-info': ServerInfo
  'package-data': { [id: string]: PackageDataEntry }
  'recovered-packages': { [id: string]: RecoveredPackageDataEntry }
  ui: UIData
}

export interface UIData {
  name: string
  'auto-check-updates': boolean
  'pkg-order': string[]
  'ack-welcome': string // EOS version
  marketplace: UIMarketplaceData
  dev: DevData
  gaming:
    | {
        snake: {
          'high-score': number
        }
      }
    | undefined
}

export interface UIMarketplaceData {
  'selected-id': string | null
  'known-hosts': {
    [id: string]: {
      url: string
      name: string
    }
  }
}

export interface DevData {
  [id: string]: DevProjectData
}

export interface DevProjectData {
  name: string
  instructions?: string
  config?: string
  'basic-info'?: BasicInfo
}

export interface ServerInfo {
  id: string
  version: string
  'last-backup': string | null
  'lan-address': Url
  'tor-address': Url
  'unread-notification-count': number
  'status-info': {
    'backing-up': boolean
    updated: boolean
    'update-progress': { size: number | null; downloaded: number } | null
  }
  'eos-version-compat': string
  'password-hash': string
}

export enum ServerStatus {
  Running = 'running',
  Updated = 'updated',
  BackingUp = 'backing-up',
}
export interface RecoveredPackageDataEntry {
  title: string
  icon: Url
  version: string
}

export interface PackageDataEntry {
  state: PackageState
  'static-files': {
    license: Url
    instructions: Url
    icon: Url
  }
  manifest: Manifest
  installed?: InstalledPackageDataEntry // exists when: installed, updating
  'install-progress'?: InstallProgress // exists when: installing, updating
}

export interface InstalledPackageDataEntry {
  status: Status
  manifest: Manifest
  'last-backup': string | null
  'system-pointers': any[]
  'current-dependents': { [id: string]: CurrentDependencyInfo }
  'current-dependencies': { [id: string]: CurrentDependencyInfo }
  'dependency-info': {
    [id: string]: {
      manifest: Manifest
      icon: Url
    }
  }
  'interface-addresses': {
    [id: string]: { 'tor-address': string; 'lan-address': string }
  }
  'marketplace-url': string | null
  'developer-key': string
}

export interface CurrentDependencyInfo {
  pointers: any[]
  'health-checks': string[] // array of health check IDs
}

export interface Manifest extends MarketplaceManifest<DependencyConfig> {
  main: ActionImpl
  'health-checks': Record<
    string,
    ActionImpl & { name: string; 'success-message': string | null }
  >
  config: ConfigActions | null
  volumes: Record<string, Volume>
  'min-os-version': string
  interfaces: Record<string, InterfaceDef>
  backup: BackupActions
  migrations: Migrations
  actions: Record<string, Action>
  permissions: any // @TODO 0.3.1
}

export interface DependencyConfig {
  check: ActionImpl
  'auto-configure': ActionImpl
}

export interface ActionImpl {
  type: 'docker'
  image: string
  system: boolean
  entrypoint: string
  args: string[]
  mounts: { [id: string]: string }
  'io-format': DockerIoFormat | null
  inject: boolean
  'shm-size': string
  'sigterm-timeout': string | null
}

export enum DockerIoFormat {
  Json = 'json',
  Yaml = 'yaml',
  Cbor = 'cbor',
  Toml = 'toml',
}

export interface ConfigActions {
  get: ActionImpl
  set: ActionImpl
}

export type Volume = VolumeData

export interface VolumeData {
  type: VolumeType.Data
  readonly: boolean
}

export interface VolumeAssets {
  type: VolumeType.Assets
}

export interface VolumePointer {
  type: VolumeType.Pointer
  'package-id': string
  'volume-id': string
  path: string
  readonly: boolean
}

export interface VolumeCertificate {
  type: VolumeType.Certificate
  'interface-id': string
}

export interface VolumeBackup {
  type: VolumeType.Backup
  readonly: boolean
}

export enum VolumeType {
  Data = 'data',
  Assets = 'assets',
  Pointer = 'pointer',
  Certificate = 'certificate',
  Backup = 'backup',
}

export interface InterfaceDef {
  name: string
  description: string
  'tor-config': TorConfig | null
  'lan-config': LanConfig | null
  ui: boolean
  protocols: string[]
}

export interface TorConfig {
  'port-mapping': { [port: number]: number }
}

export type LanConfig = {
  [port: number]: { ssl: boolean; mapping: number }
}

export interface BackupActions {
  create: ActionImpl
  restore: ActionImpl
}

export interface Migrations {
  from: { [versionRange: string]: ActionImpl }
  to: { [versionRange: string]: ActionImpl }
}

export interface Action {
  name: string
  description: string
  warning: string | null
  implementation: ActionImpl
  'allowed-statuses': (PackageMainStatus.Stopped | PackageMainStatus.Running)[]
  'input-spec': ConfigSpec
}

export interface Status {
  configured: boolean
  main: MainStatus
  'dependency-errors': { [id: string]: DependencyError | null }
}

export type MainStatus =
  | MainStatusStopped
  | MainStatusStopping
  | MainStatusStarting
  | MainStatusRunning
  | MainStatusBackingUp

export interface MainStatusStopped {
  status: PackageMainStatus.Stopped
}

export interface MainStatusStopping {
  status: PackageMainStatus.Stopping
}

export interface MainStatusStarting {
  status: PackageMainStatus.Starting
}

export interface MainStatusRunning {
  status: PackageMainStatus.Running
  started: string // UTC date string
  health: { [id: string]: HealthCheckResult }
}

export interface MainStatusBackingUp {
  status: PackageMainStatus.BackingUp
  started: string | null // UTC date string
}

export enum PackageMainStatus {
  Starting = 'starting',
  Running = 'running',
  Stopping = 'stopping',
  Stopped = 'stopped',
  BackingUp = 'backing-up',
}

export type HealthCheckResult =
  | HealthCheckResultStarting
  | HealthCheckResultLoading
  | HealthCheckResultDisabled
  | HealthCheckResultSuccess
  | HealthCheckResultFailure

export enum HealthResult {
  Starting = 'starting',
  Loading = 'loading',
  Disabled = 'disabled',
  Success = 'success',
  Failure = 'failure',
}

export interface HealthCheckResultStarting {
  result: HealthResult.Starting
}

export interface HealthCheckResultDisabled {
  result: HealthResult.Disabled
}

export interface HealthCheckResultSuccess {
  result: HealthResult.Success
}

export interface HealthCheckResultLoading {
  result: HealthResult.Loading
  message: string
}

export interface HealthCheckResultFailure {
  result: HealthResult.Failure
  error: string
}

export type DependencyError =
  | DependencyErrorNotInstalled
  | DependencyErrorNotRunning
  | DependencyErrorIncorrectVersion
  | DependencyErrorConfigUnsatisfied
  | DependencyErrorHealthChecksFailed
  | DependencyErrorTransitive

export enum DependencyErrorType {
  NotInstalled = 'not-installed',
  NotRunning = 'not-running',
  IncorrectVersion = 'incorrect-version',
  ConfigUnsatisfied = 'config-unsatisfied',
  HealthChecksFailed = 'health-checks-failed',
  InterfaceHealthChecksFailed = 'interface-health-checks-failed',
  Transitive = 'transitive',
}

export interface DependencyErrorNotInstalled {
  type: DependencyErrorType.NotInstalled
}

export interface DependencyErrorNotRunning {
  type: DependencyErrorType.NotRunning
}

export interface DependencyErrorIncorrectVersion {
  type: DependencyErrorType.IncorrectVersion
  expected: string // version range
  received: string // version
}

export interface DependencyErrorConfigUnsatisfied {
  type: DependencyErrorType.ConfigUnsatisfied
  error: string
}

export interface DependencyErrorHealthChecksFailed {
  type: DependencyErrorType.HealthChecksFailed
  check: HealthCheckResult
}

export interface DependencyErrorTransitive {
  type: DependencyErrorType.Transitive
}
