use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use patch_db::json_ptr::JsonPointer;
use patch_db::{HasModel, Map, MapModel, OptionModel};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use torut::onion::TorSecretKeyV3;

use crate::config::spec::{PackagePointerSpec, SystemPointerSpec};
use crate::install::progress::InstallProgress;
use crate::net::interface::InterfaceId;
use crate::s9pk::manifest::{Manifest, ManifestModel, PackageId};
use crate::status::health_check::HealthCheckId;
use crate::status::Status;
use crate::util::Version;

#[derive(Debug, Deserialize, Serialize, HasModel)]
#[serde(rename_all = "kebab-case")]
pub struct Database {
    #[model]
    pub server_info: ServerInfo,
    #[model]
    pub package_data: AllPackageData,
    pub broken_packages: Vec<PackageId>,
    pub ui: Value,
}
impl Database {
    pub fn init(id: String, hostname: &str, tor_key: &TorSecretKeyV3) -> Self {
        // TODO
        Database {
            server_info: ServerInfo {
                id,
                version: emver::Version::new(0, 3, 0, 0).into(),
                lan_address: format!("https://{}.local", hostname).parse().unwrap(),
                tor_address: format!("http://{}", tor_key.public().get_onion_address())
                    .parse()
                    .unwrap(),
                status: ServerStatus::Running {},
                eos_marketplace: "https://beta-registry-0-3.start9labs.com".parse().unwrap(),
                package_marketplace: None,
                wifi: WifiInfo {
                    ssids: Vec::new(),
                    connected: None,
                    selected: None,
                },
                unread_notification_count: 0,
                connection_addresses: ConnectionAddresses {
                    tor: Vec::new(),
                    clearnet: Vec::new(),
                },
                share_stats: false,
                update_progress: None,
            },
            package_data: AllPackageData::default(),
            broken_packages: Vec::new(),
            ui: Value::Object(Default::default()),
        }
    }
}
impl DatabaseModel {
    pub fn new() -> Self {
        Self::from(JsonPointer::default())
    }
}

#[derive(Debug, Deserialize, Serialize, HasModel)]
#[serde(rename_all = "kebab-case")]
pub struct ServerInfo {
    pub id: String,
    pub version: Version,
    pub lan_address: Url,
    pub tor_address: Url,
    pub status: ServerStatus,
    pub eos_marketplace: Url,
    pub package_marketplace: Option<Url>, // None implies use eos_marketplace
    pub wifi: WifiInfo,
    pub unread_notification_count: u64,
    pub connection_addresses: ConnectionAddresses,
    pub share_stats: bool,
    #[model]
    pub update_progress: Option<UpdateProgress>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ServerStatus {
    Running,
    Updating,
    Updated,
    BackingUp,
}

#[derive(Debug, Deserialize, Serialize, HasModel)]
#[serde(rename_all = "kebab-case")]
pub struct UpdateProgress {
    pub size: Option<u64>,
    pub downloaded: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct WifiInfo {
    pub ssids: Vec<String>,
    pub selected: Option<String>,
    pub connected: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct ServerSpecs {
    pub cpu: String,
    pub disk: String,
    pub memory: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct ConnectionAddresses {
    pub tor: Vec<String>,
    pub clearnet: Vec<String>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct AllPackageData(pub BTreeMap<PackageId, PackageDataEntry>);
impl Map for AllPackageData {
    type Key = PackageId;
    type Value = PackageDataEntry;
    fn get(&self, key: &Self::Key) -> Option<&Self::Value> {
        self.0.get(key)
    }
}
impl HasModel for AllPackageData {
    type Model = MapModel<Self>;
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct StaticFiles {
    license: String,
    instructions: String,
    icon: String,
}
impl StaticFiles {
    pub fn local(id: &PackageId, version: &Version, icon_type: &str) -> Self {
        StaticFiles {
            license: format!("/public/package-data/{}/{}/LICENSE.md", id, version),
            instructions: format!("/public/package-data/{}/{}/INSTRUCTIONS.md", id, version),
            icon: format!("/public/package-data/{}/{}/icon.{}", id, version, icon_type),
        }
    }
    pub fn remote(id: &PackageId, version: &Version) -> Self {
        StaticFiles {
            license: format!("/marketplace/package/license/{}?spec=={}", id, version),
            instructions: format!("/marketplace/package/instructions/{}?spec=={}", id, version),
            icon: format!("/marketplace/package/icon/{}?spec=={}", id, version),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, HasModel)]
#[serde(tag = "state")]
#[serde(rename_all = "kebab-case")]
pub enum PackageDataEntry {
    #[serde(rename_all = "kebab-case")]
    Installing {
        static_files: StaticFiles,
        manifest: Manifest,
        install_progress: Arc<InstallProgress>,
    }, // { state: "installing", 'install-progress': InstallProgress }
    #[serde(rename_all = "kebab-case")]
    Updating {
        static_files: StaticFiles,
        manifest: Manifest,
        installed: InstalledPackageDataEntry,
        install_progress: Arc<InstallProgress>,
    },
    #[serde(rename_all = "kebab-case")]
    Removing {
        static_files: StaticFiles,
        manifest: Manifest,
    },
    #[serde(rename_all = "kebab-case")]
    Installed {
        static_files: StaticFiles,
        manifest: Manifest,
        installed: InstalledPackageDataEntry,
    },
}
impl PackageDataEntryModel {
    pub fn installed(self) -> OptionModel<InstalledPackageDataEntry> {
        self.0.child("installed").into()
    }
    pub fn install_progress(self) -> OptionModel<InstallProgress> {
        self.0.child("install-progress").into()
    }
    pub fn manifest(self) -> ManifestModel {
        self.0.child("manifest").into()
    }
}

#[derive(Debug, Deserialize, Serialize, HasModel)]
#[serde(rename_all = "kebab-case")]
pub struct InstalledPackageDataEntry {
    #[model]
    pub status: Status,
    #[model]
    pub manifest: Manifest,
    pub system_pointers: Vec<SystemPointerSpec>,
    #[model]
    pub dependency_info: BTreeMap<PackageId, StaticDependencyInfo>,
    #[model]
    pub current_dependents: BTreeMap<PackageId, CurrentDependencyInfo>,
    #[model]
    pub current_dependencies: BTreeMap<PackageId, CurrentDependencyInfo>,
    #[model]
    pub interface_addresses: InterfaceAddressMap,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, HasModel)]
#[serde(rename_all = "kebab-case")]
pub struct StaticDependencyInfo {
    pub manifest: Option<Manifest>,
    pub icon: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, HasModel)]
#[serde(rename_all = "kebab-case")]
pub struct CurrentDependencyInfo {
    pub pointers: Vec<PackagePointerSpec>,
    pub health_checks: BTreeSet<HealthCheckId>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct InterfaceAddressMap(pub BTreeMap<InterfaceId, InterfaceAddresses>);
impl Map for InterfaceAddressMap {
    type Key = InterfaceId;
    type Value = InterfaceAddresses;
    fn get(&self, key: &Self::Key) -> Option<&Self::Value> {
        self.0.get(key)
    }
}
impl HasModel for InterfaceAddressMap {
    type Model = MapModel<Self>;
}

#[derive(Debug, Deserialize, Serialize, HasModel)]
#[serde(rename_all = "kebab-case")]
pub struct InterfaceAddresses {
    #[model]
    pub tor_address: Option<String>,
    #[model]
    pub lan_address: Option<String>,
}
