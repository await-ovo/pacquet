use crate::Install;
use derive_more::{Display, Error};
use miette::Diagnostic;
use pacquet_lockfile::Lockfile;
use pacquet_network::ThrottledClient;
use pacquet_npmrc::Npmrc;
use pacquet_package_manifest::PackageManifestError;
use pacquet_package_manifest::{DependencyGroup, PackageManifest};
use pacquet_registry::{PackageTag, PackageVersion};
use pacquet_tarball::MemCache;

/// This subroutine does everything `pacquet add` is supposed to do.
#[must_use]
pub struct Add<'a, ListDependencyGroups, DependencyGroupList>
where
    ListDependencyGroups: Fn() -> DependencyGroupList,
    DependencyGroupList: IntoIterator<Item = DependencyGroup>,
{
    pub tarball_mem_cache: &'a MemCache,
    pub http_client: &'a ThrottledClient,
    pub config: &'static Npmrc,
    pub manifest: &'a mut PackageManifest,
    pub lockfile: Option<&'a Lockfile>,
    pub list_dependency_groups: ListDependencyGroups, // must be a function because it is called multiple times
    pub package_name: &'a str, // TODO: 1. support version range, 2. multiple arguments, 3. name this `packages`
    pub save_exact: bool,      // TODO: add `save-exact` to `.npmrc`, merge configs, and remove this
}

/// Error type of [`Add`].
#[derive(Debug, Display, Error, Diagnostic)]
pub enum AddError {
    #[display("Failed to add package to manifest: {_0}")]
    AddDependencyToManifest(#[error(source)] PackageManifestError),
    #[display("Failed save the manifest file: {_0}")]
    SaveManifest(#[error(source)] PackageManifestError),
}

impl<'a, ListDependencyGroups, DependencyGroupList>
    Add<'a, ListDependencyGroups, DependencyGroupList>
where
    ListDependencyGroups: Fn() -> DependencyGroupList,
    DependencyGroupList: IntoIterator<Item = DependencyGroup>,
{
    pub async fn run(self) -> Result<(), AddError> {
        let Add {
            tarball_mem_cache,
            http_client,
            config,
            manifest,
            lockfile,
            list_dependency_groups,
            package_name,
            save_exact,
        } = self;

        let latest_version = PackageVersion::fetch_from_registry(
            package_name,
            PackageTag::Latest, // TODO: add support for specifying tags
            http_client,
            &config.registry,
        )
        .await
        .expect("resolve latest tag"); // TODO: properly propagate this error

        let version_range = latest_version.serialize(save_exact);
        for dependency_group in list_dependency_groups() {
            manifest
                .add_dependency(package_name, &version_range, dependency_group)
                .map_err(AddError::AddDependencyToManifest)?;
        }

        Install {
            tarball_mem_cache,
            http_client,
            config,
            manifest,
            lockfile,
            dependency_groups: list_dependency_groups(),
            frozen_lockfile: false,
        }
        .run()
        .await;

        manifest.save().map_err(AddError::SaveManifest)?;

        Ok(())
    }
}
