use std::{env, ffi::OsString, path::PathBuf};

use colorize::AnsiColor;
use kube::config::Kubeconfig;

fn main() {
    println!("");
    println!("🏥 kubectl-config-doctor v{}", env!("CARGO_PKG_VERSION"));
    println!("");

    let kubeconfig_value = env::var_os("KUBECONFIG");

    println!("1. Environment Variables");
    print_env_var("KUBECONFIG");
    print_env_var("HTTP_PROXY");
    print_env_var("HTTPS_PROXY");
    print_env_var("NO_PROXY");
    println!("");

    let files = match kubeconfig_value {
        Some(ref value) => files_from_env(value),
        None => files_from_default_path(),
    };
    
    println!("2. Kubeconfig Files");
    let cfgs = inspect_files(&files);
    println!("");

    println!("3. Looking for Duplicates");
    verify_duplicates(cfgs);
    println!("");
}

fn print_env_var(name: &str) {
    println!("- {}: {}", name, match env::var(name) {
        Ok(value) => value,
        Err(_) => "<not set>".to_string().magenta(),
    });
}

fn inspect_files(files: &Vec<PathBuf>) -> Vec<Kubeconfig> {
    let mut cfgs: Vec<Kubeconfig> = Vec::new();
    for file in files {
        match Kubeconfig::read_from(&file) {
            Ok(cfg) => {
                println!("✅ {} - {}", file.display().to_string().blue(), "exists".green());
                println!(" - Contexts: {}", cfg.contexts.iter().map(|c| c.name.clone()).collect::<Vec<String>>().join(", "));
                println!(" - Clusters: {}", cfg.clusters.iter().map(|c| c.name.clone()).collect::<Vec<String>>().join(", "));
                println!(" - Users: {}", cfg.auth_infos.iter().map(|a| a.name.clone()).collect::<Vec<String>>().join(", "));
                cfgs.push(cfg)
            },
            Err(err) => {
                println!("❌ {} - {}", file.display(), err.to_string().red());
            }
        }
    }
    return cfgs;
}

fn verify_duplicates(cfgs: Vec<Kubeconfig>) {
    let mut contexts = vec![];
    let mut clusters = vec![];
    let mut users = vec![];

    let mut i = 0;

    for cfg in cfgs {
        for context in cfg.contexts {
            if contexts.contains(&context.name) {
                println!("❌ Context {} is defined in two or more files", context.name.red());
                i += 1;
            } else {
                contexts.push(context.name);
            }
        }

        for cluster in cfg.clusters {
            if clusters.contains(&cluster.name) {
                println!("❌ Cluster {} is defined in two or more files", cluster.name.red());
                i += 1;
            } else {
                clusters.push(cluster.name);
            }
        }

        for user in cfg.auth_infos {
            if users.contains(&user.name) {
                println!("❌ User {} is defined in two or more files", user.name.red());
                i += 1;
            } else {
                users.push(user.name);
            }
        }
    }

    if i == 0 {
        println!("✅ {}", "No duplicates found".green());
    }
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