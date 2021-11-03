use std::collections::BTreeMap;
use std::convert::TryInto;
use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::Poll;
use std::time::Duration;

use bollard::container::StopContainerOptions;
use color_eyre::eyre::eyre;
use num_enum::TryFromPrimitive;
use patch_db::DbHandle;
use sqlx::{Executor, Sqlite};
use tokio::sync::watch::error::RecvError;
use tokio::sync::watch::{channel, Receiver, Sender};
use tokio::sync::{Notify, RwLock};
use torut::onion::TorSecretKeyV3;
use tracing::instrument;

use crate::action::docker::DockerAction;
use crate::action::NoOutput;
use crate::context::RpcContext;
use crate::manager::sync::synchronizer;
use crate::net::interface::InterfaceId;
use crate::notifications::NotificationLevel;
use crate::s9pk::manifest::{Manifest, PackageId};
use crate::util::{Container, NonDetachingJoinHandle, Version};
use crate::Error;

pub mod health;
mod sync;

#[derive(Default)]
pub struct ManagerMap(RwLock<BTreeMap<(PackageId, Version), Arc<Manager>>>);
impl ManagerMap {
    #[instrument(skip(self, ctx, db, secrets))]
    pub async fn init<Db: DbHandle, Ex>(
        &self,
        ctx: &RpcContext,
        db: &mut Db,
        secrets: &mut Ex,
    ) -> Result<(), Error>
    where
        for<'a> &'a mut Ex: Executor<'a, Database = Sqlite>,
    {
        let mut res = BTreeMap::new();
        for package in crate::db::DatabaseModel::new()
            .package_data()
            .keys(db, true)
            .await?
        {
            let man: Manifest = if let Some(manifest) = crate::db::DatabaseModel::new()
                .package_data()
                .idx_model(&package)
                .and_then(|pkg| pkg.installed())
                .map(|m| m.manifest())
                .get(db, true)
                .await?
                .to_owned()
            {
                manifest
            } else {
                continue;
            };
            let tor_keys = man.interfaces.tor_keys(secrets, &package).await?;
            res.insert(
                (package, man.version.clone()),
                Arc::new(Manager::create(ctx.clone(), man, tor_keys).await?),
            );
        }
        *self.0.write().await = res;
        Ok(())
    }

    #[instrument(skip(self, ctx))]
    pub async fn add(
        &self,
        ctx: RpcContext,
        manifest: Manifest,
        tor_keys: BTreeMap<InterfaceId, TorSecretKeyV3>,
    ) -> Result<(), Error> {
        let mut lock = self.0.write().await;
        let id = (manifest.id.clone(), manifest.version.clone());
        if let Some(man) = lock.remove(&id) {
            if !man.thread.is_empty().await {
                man.exit().await?;
            }
        }
        lock.insert(
            id,
            Arc::new(Manager::create(ctx, manifest, tor_keys).await?),
        );
        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn remove(&self, id: &(PackageId, Version)) {
        if let Some(man) = self.0.write().await.remove(id) {
            if let Err(e) = man.exit().await {
                tracing::error!("Error shutting down manager: {}", e);
                tracing::debug!("{:?}", e);
            }
        }
    }

    #[instrument(skip(self))]
    pub async fn empty(&self) -> Result<(), Error> {
        let res =
            futures::future::join_all(std::mem::take(&mut *self.0.write().await).into_iter().map(
                |((id, version), man)| async move {
                    man.exit().await?;
                    tracing::debug!("Manager for {}@{} shutdown", id, version);
                    Ok::<_, Error>(())
                },
            ))
            .await;
        res.into_iter().fold(Ok(()), |res, x| match (res, x) {
            (Ok(()), x) => x,
            (Err(e), Ok(())) => Err(e),
            (Err(e1), Err(e2)) => Err(Error::new(eyre!("{}, {}", e1.source, e2.source), e1.kind)),
        })
    }

    #[instrument(skip(self))]
    pub async fn get(&self, id: &(PackageId, Version)) -> Option<Arc<Manager>> {
        self.0.read().await.get(id).cloned()
    }
}

pub struct Manager {
    shared: Arc<ManagerSharedState>,
    thread: Container<NonDetachingJoinHandle<()>>,
}

#[derive(TryFromPrimitive)]
#[repr(usize)]
pub enum Status {
    Running = 0,
    Stopped = 1,
    Paused = 2,
}

pub struct ManagerSharedState {
    ctx: RpcContext,
    status: AtomicUsize,
    on_stop: Sender<OnStop>,
    manifest: Manifest,
    container_name: String,
    tor_keys: BTreeMap<InterfaceId, TorSecretKeyV3>,
    synchronized: Notify,
    synchronize_now: Notify,
}

#[derive(Clone, Copy)]
pub enum OnStop {
    Restart,
    Sleep,
    Exit,
}

#[instrument(skip(state))]
async fn run_main(
    state: &Arc<ManagerSharedState>,
) -> Result<Result<NoOutput, (i32, String)>, Error> {
    let rt_state = state.clone();
    let mut runtime = tokio::spawn(async move {
        rt_state
            .manifest
            .main
            .execute::<(), NoOutput>(
                &rt_state.ctx,
                &rt_state.manifest.id,
                &rt_state.manifest.version,
                None,
                &rt_state.manifest.volumes,
                None,
                false,
            )
            .await
    });
    let ip;
    loop {
        match state
            .ctx
            .docker
            .inspect_container(&state.container_name, None)
            .await
        {
            Ok(res) => {
                if let Some(ip_addr) = res
                    .network_settings
                    .and_then(|ns| ns.networks)
                    .and_then(|mut n| n.remove("start9"))
                    .and_then(|es| es.ip_address)
                    .filter(|ip| !ip.is_empty())
                    .map(|ip| ip.parse())
                    .transpose()?
                {
                    ip = ip_addr;
                    break;
                }
            }
            Err(bollard::errors::Error::DockerResponseNotFoundError { .. }) => (),
            Err(e) => Err(e)?,
        }
        match futures::poll!(&mut runtime) {
            Poll::Ready(res) => {
                return res
                    .map_err(|_| {
                        Error::new(eyre!("Manager runtime panicked!"), crate::ErrorKind::Docker)
                    })
                    .and_then(|a| a)
            }
            _ => (),
        }
    }

    state
        .ctx
        .net_controller
        .add(
            &state.manifest.id,
            ip,
            state
                .manifest
                .interfaces
                .0
                .iter()
                .map(|(id, info)| {
                    Ok((
                        id.clone(),
                        info,
                        state
                            .tor_keys
                            .get(id)
                            .ok_or_else(|| {
                                Error::new(
                                    eyre!("interface {} missing key", id),
                                    crate::ErrorKind::Tor,
                                )
                            })?
                            .clone(),
                    ))
                })
                .collect::<Result<Vec<_>, Error>>()?,
        )
        .await?;
    let health = async {
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let mut db = state.ctx.db.handle();
            if let Err(e) = health::check(&state.ctx, &mut db, &state.manifest.id).await {
                tracing::error!(
                    "Failed to run health check for {}: {}",
                    &state.manifest.id,
                    e
                );
                tracing::debug!("{:?}", e);
            }
        }
    };
    let res = tokio::select! {
        a = runtime => a.map_err(|_| Error::new(eyre!("Manager runtime panicked!"), crate::ErrorKind::Docker)).and_then(|a| a),
        _ = health => Err(Error::new(eyre!("Health check daemon exited!"), crate::ErrorKind::Unknown)),
    };
    state
        .ctx
        .net_controller
        .remove(
            &state.manifest.id,
            state.manifest.interfaces.0.keys().cloned(),
        )
        .await?;
    res
}

impl Manager {
    #[instrument(skip(ctx))]
    async fn create(
        ctx: RpcContext,
        manifest: Manifest,
        tor_keys: BTreeMap<InterfaceId, TorSecretKeyV3>,
    ) -> Result<Self, Error> {
        let (on_stop, recv) = channel(OnStop::Sleep);
        let shared = Arc::new(ManagerSharedState {
            ctx,
            status: AtomicUsize::new(Status::Stopped as usize),
            on_stop,
            container_name: DockerAction::container_name(&manifest.id, None),
            manifest,
            tor_keys,
            synchronized: Notify::new(),
            synchronize_now: Notify::new(),
        });
        let thread_shared = shared.clone();
        let thread = tokio::spawn(async move {
            tokio::select! {
                _ = manager_thread_loop(recv, &thread_shared) => (),
                _ = synchronizer(&*thread_shared) => (),
            }
        });
        Ok(Manager {
            shared,
            thread: Container::new(Some(thread.into())),
        })
    }

    #[instrument(skip(self))]
    async fn exit(&self) -> Result<(), Error> {
        let _ = self.shared.on_stop.send(OnStop::Exit);
        match self
            .shared
            .ctx
            .docker
            .stop_container(
                &self.shared.container_name,
                Some(StopContainerOptions { t: 30 }),
            )
            .await
        {
            Err(bollard::errors::Error::DockerResponseNotFoundError { .. })
            | Err(bollard::errors::Error::DockerResponseConflictError { .. })
            | Err(bollard::errors::Error::DockerResponseNotModifiedError { .. }) => (), // Already stopped
            a => a?,
        };
        self.shared.status.store(
            Status::Stopped as usize,
            std::sync::atomic::Ordering::SeqCst,
        );
        if let Some(thread) = self.thread.take().await {
            thread.await.map_err(|e| {
                Error::new(
                    eyre!("Manager thread panicked: {}", e),
                    crate::ErrorKind::Docker,
                )
            })?;
        }
        Ok(())
    }

    pub async fn synchronize(&self) {
        self.shared.synchronize_now.notify_waiters();
        self.shared.synchronized.notified().await
    }
}

async fn manager_thread_loop(mut recv: Receiver<OnStop>, thread_shared: &Arc<ManagerSharedState>) {
    loop {
        fn handle_stop_action<'a>(
            recv: &'a mut Receiver<OnStop>,
        ) -> (
            OnStop,
            Option<impl Future<Output = Result<(), RecvError>> + 'a>,
        ) {
            let val = *recv.borrow_and_update();
            match val {
                OnStop::Sleep => (OnStop::Sleep, Some(recv.changed())),
                a => (a, None),
            }
        }
        let (stop_action, fut) = handle_stop_action(&mut recv);
        match stop_action {
            OnStop::Sleep => {
                if let Some(fut) = fut {
                    thread_shared.status.store(
                        Status::Stopped as usize,
                        std::sync::atomic::Ordering::SeqCst,
                    );
                    fut.await.unwrap();
                    continue;
                }
            }
            OnStop::Exit => {
                thread_shared.status.store(
                    Status::Stopped as usize,
                    std::sync::atomic::Ordering::SeqCst,
                );
                break;
            }
            OnStop::Restart => {
                thread_shared.status.store(
                    Status::Running as usize,
                    std::sync::atomic::Ordering::SeqCst,
                );
            }
        }
        match run_main(&thread_shared).await {
            Ok(Ok(NoOutput)) => {
                thread_shared
                    .on_stop
                    .send(OnStop::Sleep)
                    .map_err(|_| ())
                    .unwrap(); // recv is still in scope, cannot fail
            }
            Ok(Err(e)) => {
                let res = thread_shared.ctx.notification_manager
                    .notify(
                        &mut thread_shared.ctx.db.handle(),
                        Some(thread_shared.manifest.id.clone()),
                        NotificationLevel::Warning,
                        String::from("Service Crashed"),
                        format!("The service {} has crashed with the following exit code: {}\nDetails: {}", thread_shared.manifest.id.clone(), e.0, e.1),
                        (),
                        Some(900) // 15 minutes
                    )
                    .await;
                match res {
                    Err(e) => {
                        tracing::error!("Failed to issue notification: {}", e);
                        tracing::debug!("{:?}", e);
                    }
                    Ok(()) => {}
                }
                tracing::error!("service crashed: {}: {}", e.0, e.1);
            }
            Err(e) => {
                tracing::error!("failed to start service: {}", e);
                tracing::debug!("{:?}", e);
            }
        }
    }
}

#[instrument(skip(shared))]
async fn stop(shared: &ManagerSharedState) -> Result<(), Error> {
    shared.on_stop.send(OnStop::Sleep).map_err(|_| {
        Error::new(
            eyre!("Manager has already been shutdown"),
            crate::ErrorKind::Docker,
        )
    })?;
    if matches!(
        shared.status.load(Ordering::SeqCst).try_into().unwrap(),
        Status::Paused
    ) {
        resume(shared).await?;
    }
    match shared
        .ctx
        .docker
        .stop_container(&shared.container_name, Some(StopContainerOptions { t: 30 }))
        .await
    {
        Err(bollard::errors::Error::DockerResponseNotFoundError { .. })
        | Err(bollard::errors::Error::DockerResponseConflictError { .. })
        | Err(bollard::errors::Error::DockerResponseNotModifiedError { .. }) => (), // Already stopped
        a => a?,
    };
    shared.status.store(
        Status::Stopped as usize,
        std::sync::atomic::Ordering::SeqCst,
    );
    Ok(())
}

#[instrument(skip(shared))]
async fn start(shared: &ManagerSharedState) -> Result<(), Error> {
    shared.on_stop.send(OnStop::Restart).map_err(|_| {
        Error::new(
            eyre!("Manager has already been shutdown"),
            crate::ErrorKind::Docker,
        )
    })?;
    shared.status.store(
        Status::Running as usize,
        std::sync::atomic::Ordering::SeqCst,
    );
    Ok(())
}

#[instrument(skip(shared))]
async fn pause(shared: &ManagerSharedState) -> Result<(), Error> {
    shared
        .ctx
        .docker
        .pause_container(&shared.container_name)
        .await?;
    shared
        .status
        .store(Status::Paused as usize, std::sync::atomic::Ordering::SeqCst);
    Ok(())
}

#[instrument(skip(shared))]
async fn resume(shared: &ManagerSharedState) -> Result<(), Error> {
    shared
        .ctx
        .docker
        .unpause_container(&shared.container_name)
        .await?;
    shared.status.store(
        Status::Running as usize,
        std::sync::atomic::Ordering::SeqCst,
    );
    Ok(())
}
