use std::{env, error::Error, ffi::OsString, path::PathBuf};

use colorize::AnsiColor;
use kube::{config::{Config, KubeConfigOptions, Kubeconfig}, Client};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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
    println!("");
    let cfgs = inspect_files(files);
    println!("");

    println!("3. Looking for Duplicates");
    verify_duplicates(&cfgs);
    println!("");

    println!("4. Running Tests");
    println!("");
    match cfgs.iter().try_fold(Kubeconfig::default(), |m, c| m.merge(c.clone())) {
        Ok(merged) => {
            for context in &merged.contexts {
                inspect_context(&merged, context.name.clone()).await;
                println!("");
            }
        },
        Err(err) => println!("{} {}", red_cross(), err.to_string().red())
    }
    println!("");

    Ok(())
}

async fn inspect_context(kubeconfig: &Kubeconfig, context: String) {
    let opts = KubeConfigOptions{
        context: Some(context.clone()),
        cluster: None,
        user: None,
    };

    let config = Config::from_custom_kubeconfig(kubeconfig.clone(), &opts).await;
    match config {
        Ok(mut config) => {
            config.connect_timeout = Some(std::time::Duration::from_secs(3));

            println!("{} {} - {}", green_check(), context.cyan(), "is valid".green());

            let connected = match Client::try_from(config.clone()) {
                Ok(c) => c.apiserver_version().await,
                Err(err) => Err(err)
            };

            println!(" - API Server: {} {} - {}", config.cluster_url, match &config.proxy_url { 
                Some(url) => format!("(proxy: {})", url.to_string()).grey(),
                None => "(no proxy)".to_string().grey(),
            }, match connected {
                Ok(_) => "connected".green(),
                Err(err) => long_error(err).red()
            });
        },
        Err(err) => {
            println!("{} {} - {}", red_cross(), context.cyan(), "is invalid".red());
            println!("{}", err.to_string().red())
        }
    }
}

fn long_error(err: kube::Error) -> String {
    let mut error_msg = err.to_string();
    let mut source = err.source();

    while let Some(err_source) = source {
        error_msg.push_str(&format!(": {}", err_source));
        source = err_source.source();
    }

    return error_msg;
}

fn print_env_var(name: &str) {
    println!("- {}: {}", name, match env::var(name) {
        Ok(value) => value,
        Err(_) => "<not set>".to_string().magenta(),
    });
}

fn inspect_files(files: Vec<PathBuf>) -> Vec<Kubeconfig> {
    let mut cfgs: Vec<Kubeconfig> = Vec::new();
    for file in files {
        match Kubeconfig::read_from(&file) {
            Ok(cfg) => {
                println!("{} {} - {}", green_check(), file.display().to_string().cyan(), "exists".green());
                println!(" - Contexts: {}", cfg.contexts.iter().map(|c| c.name.clone()).collect::<Vec<String>>().join(", "));
                println!(" - Clusters: {}", cfg.clusters.iter().map(|c| c.name.clone()).collect::<Vec<String>>().join(", "));
                println!(" - Users: {}", cfg.auth_infos.iter().map(|a| a.name.clone()).collect::<Vec<String>>().join(", "));
                cfgs.push(cfg)
            },
            Err(err) => {
                println!("{} {} - {}", red_cross(), file.display(), err.to_string().red());
            }
        }
        println!("");
    }
    return cfgs;
}

fn verify_duplicates(cfgs: &Vec<Kubeconfig>) {
    let mut contexts = vec![];
    let mut clusters = vec![];
    let mut users = vec![];

    let mut i = 0;

    for cfg in cfgs {
        for context in &cfg.contexts {
            if contexts.contains(&context.name) {
                println!("{} Context {} is defined in two or more files", red_cross(), context.name.clone().red());
                i += 1;
            } else {
                contexts.push(context.name.clone());
            }
        }

        for cluster in &cfg.clusters {
            if clusters.contains(&cluster.name) {
                println!("{} Cluster {} is defined in two or more files", red_cross(), cluster.name.clone().red());
                i += 1;
            } else {
                clusters.push(cluster.name.clone());
            }
        }

        for user in &cfg.auth_infos {
            if users.contains(&user.name) {
                println!("{} User {} is defined in two or more files", red_cross(), user.name.clone().red());
                i += 1;
            } else {
                users.push(user.name.clone());
            }
        }
    }

    if i == 0 {
        println!("{} {}", green_check(), "No duplicates found".green());
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

fn green_check() -> String {
    "✓".green().to_string()
}

fn red_cross() -> String {
    "✖".red().to_string()
}