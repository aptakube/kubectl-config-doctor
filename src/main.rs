mod doctor;
mod style;

use std::{env, ffi::OsString, path::PathBuf};

use kube::config::{Kubeconfig, KubeconfigError};
use style::{print_kubeconfigerror, print_title};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("");
    println!("🏥 kubectl-config-doctor v{}", doctor::version());
    println!("");

    print_title("1. Environment Variables");
    doctor::inspect_env_var("KUBECONFIG");
    doctor::inspect_env_var("HTTP_PROXY");
    doctor::inspect_env_var("HTTPS_PROXY");
    doctor::inspect_env_var("NO_PROXY");
    println!("");

    let files = match env::var_os("KUBECONFIG") {
        Some(ref value) => files_from_env(value),
        None => files_from_default_path(),
    };
    
    print_title("2. Kubeconfig Files");
    let cfgs = doctor::inspect_files(files);

    print_title("3. Looking for Duplicates");
    doctor::verify_duplicates(&cfgs);
    println!("");

    print_title("4. Running Tests");
    match merge_kubeconfigs(cfgs) {
        Ok(merged) => {
            for context in &merged.contexts {
                doctor::inspect_context(&merged, context.name.clone()).await;
                println!("");
            }
        },
        Err(err) => print_kubeconfigerror(err)
    }

    Ok(())
}

fn merge_kubeconfigs(cfgs: Vec<Kubeconfig>) -> Result<Kubeconfig, KubeconfigError> {
    cfgs.iter().try_fold(Kubeconfig::default(), |m, c| m.merge(c.clone()))
}

fn files_from_env(value: &OsString) -> Vec<PathBuf> {
    let paths = std::env::split_paths(value)
        .filter(|p| !p.as_os_str().is_empty())
        .collect::<Vec<_>>();
    if paths.is_empty() {
        return vec![];
    }

    paths
}


fn files_from_default_path() -> Vec<PathBuf> {
    let path = home::home_dir().expect("could not get user HOME dir").join(".kube").join("config");
    return vec![path];
}