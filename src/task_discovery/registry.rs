use crate::task_discovery::{
    DiscoveredTasks, TaskDiscovery, cmake::CmakeDiscovery, docker_compose::DockerComposeDiscovery,
    github_actions::GithubActionsDiscovery, gradle::GradleDiscovery, justfile::JustfileDiscovery,
    make::MakefileDiscovery, maven::MavenDiscovery, npm::NpmDiscovery, python::PythonDiscovery,
    shell_scripts::ShellScriptDiscovery, taskfile::TaskfileDiscovery, travis_ci::TravisCiDiscovery,
    turbo::TurboDiscovery,
};
use std::path::Path;

static MAKEFILE_DISCOVERY: MakefileDiscovery = MakefileDiscovery;
static NPM_DISCOVERY: NpmDiscovery = NpmDiscovery;
static PYTHON_DISCOVERY: PythonDiscovery = PythonDiscovery;
static TASKFILE_DISCOVERY: TaskfileDiscovery = TaskfileDiscovery;
static TURBO_DISCOVERY: TurboDiscovery = TurboDiscovery;
static MAVEN_DISCOVERY: MavenDiscovery = MavenDiscovery;
static GRADLE_DISCOVERY: GradleDiscovery = GradleDiscovery;
static GITHUB_ACTIONS_DISCOVERY: GithubActionsDiscovery = GithubActionsDiscovery;
static DOCKER_COMPOSE_DISCOVERY: DockerComposeDiscovery = DockerComposeDiscovery;
static TRAVIS_CI_DISCOVERY: TravisCiDiscovery = TravisCiDiscovery;
static CMAKE_DISCOVERY: CmakeDiscovery = CmakeDiscovery;
static JUSTFILE_DISCOVERY: JustfileDiscovery = JustfileDiscovery;
static SHELL_SCRIPT_DISCOVERY: ShellScriptDiscovery = ShellScriptDiscovery;

pub(crate) fn registered_discoveries() -> Vec<&'static dyn TaskDiscovery> {
    vec![
        &MAKEFILE_DISCOVERY,
        &NPM_DISCOVERY,
        &PYTHON_DISCOVERY,
        &TASKFILE_DISCOVERY,
        &TURBO_DISCOVERY,
        &MAVEN_DISCOVERY,
        &GRADLE_DISCOVERY,
        &GITHUB_ACTIONS_DISCOVERY,
        &DOCKER_COMPOSE_DISCOVERY,
        &TRAVIS_CI_DISCOVERY,
        &CMAKE_DISCOVERY,
        &JUSTFILE_DISCOVERY,
        &SHELL_SCRIPT_DISCOVERY,
    ]
}

#[allow(dead_code)]
pub(crate) fn run_discovery(
    discovery: &dyn TaskDiscovery,
    dir: &Path,
    discovered: &mut DiscoveredTasks,
) {
    discovery.discover(dir, discovered);
}
