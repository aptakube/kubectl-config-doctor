use std::{env, path::PathBuf, time::Duration};

use colored::Colorize;
use http::Request;
use kube::{config::{AuthInfo, Cluster, KubeConfigOptions, Kubeconfig}, Client, Config};
use secrecy::{ExposeSecret, SecretString};

use crate::style::{expand_kubeconfigerror, expand_kubeerror, green_check, print_error, red_cross, ColorizeExt};

pub fn version() -> &'static str {
    return env!("CARGO_PKG_VERSION");
}

pub fn inspect_files(files: Vec<PathBuf>) -> Vec<Kubeconfig> {
    let mut cfgs: Vec<Kubeconfig> = Vec::new();
    for file in files {
        match Kubeconfig::read_from(&file) {
            Ok(cfg) => {
                println!("{} {} - {}", green_check(), file.display().to_string().cyan(), "exists".green());
                println!(" - {} {}", "Contexts:".grey(), cfg.contexts.iter().map(|c| c.name.clone()).collect::<Vec<String>>().join(", "));
                println!(" - {} {}", "Clusters:".grey(), cfg.clusters.iter().map(|c| c.name.clone()).collect::<Vec<String>>().join(", "));
                println!(" - {} {}", "Users:".grey(), cfg.auth_infos.iter().map(|a| a.name.clone()).collect::<Vec<String>>().join(", "));
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

pub fn inspect_env_var(name: &str) {
    println!("- {}: {}", name.cyan(), match env::var(name) {
        Ok(value) => value,
        Err(_) => "<not set>".light_grey().to_string(),
    });
}

pub fn verify_duplicates(cfgs: &Vec<Kubeconfig>) {
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

pub async fn inspect_context(kubeconfig: &Kubeconfig, context: String) {
    let (cluster_name, cluster, user, authinfo) = find_context(kubeconfig, &context);
    println!("{} {}", context.cyan().underline().bold(), format!("(Cluster: {}, User: {})", cluster_name.bold(), user.bold()));
  
    inspect_cluster(cluster_name, cluster);
    inspect_authinfo(user, authinfo);

    let config = Config::from_custom_kubeconfig(kubeconfig.clone(), &KubeConfigOptions{
        context: Some(context.clone()),
        cluster: None,
        user: None,
    }).await;

    match config {
        Ok(config) => {
            inspect_proxy_reachable(&config.proxy_url).await;
            inspect_server_reachable(config.clone()).await;
            inspect_server_auth(config.clone()).await;
        },
        Err(err) => print_error(expand_kubeconfigerror(err))
    }
}

async fn inspect_server_auth(mut config: Config) {
    config.connect_timeout = Some(std::time::Duration::from_secs(5));
    let version = match Client::try_from(config) {
        Ok(c) => c.apiserver_version().await,
        Err(err) => Err(err)
    };

    match version {
        Ok(info) => println!("{} {} v{}.{} - {}", green_check(), "Server Version".grey(), info.major, info.minor, "OK".green()),
        Err(err) => print_error(expand_kubeerror(err))
    };
}

async fn inspect_server_reachable(mut config: Config) {
    config.connect_timeout = Some(std::time::Duration::from_secs(5));

    // Reset the auth info to force a 403 error if the server is reachable
    config.auth_info.username = None;
    config.auth_info.password = None;
    config.auth_info.token = None;
    config.auth_info.token_file = None;
    config.auth_info.client_certificate = None;
    config.auth_info.client_certificate_data = None;
    config.auth_info.client_key = None;
    config.auth_info.client_key_data = None;
    config.auth_info.impersonate = None;
    config.auth_info.impersonate_groups = None;
    config.auth_info.auth_provider = None;
    config.auth_info.exec = None;

    let reachable = match Client::try_from(config.clone()) {
        Ok(c) => { 
            let req = Request::builder().uri("/").body(vec![]).map_err(kube::Error::HttpError);
            match req {
                Ok(req) => { 
                    let res = c.request_text(req).await;
                    match res {
                        Ok(_) => Ok(true),
                        Err(err) => {
                            if let kube::Error::Api(e) = err {
                                // Server is reachable as long as the HTTP status code is not a server error
                                Ok(e.code < 500)
                            } else {
                                Err(err)
                            }
                        }
                    }
                },
                Err(err) => Err(err)
            }
        },
        Err(err) => Err(err)
    };

    println!("{} {}: {} - {}", match reachable { 
        Ok(_) => green_check(),
        Err(_) => red_cross()
    }, "Server URL".grey(), config.cluster_url.to_string(), match reachable {
        Ok(_) => "reachable".green(),
        Err(_) => "unreachable".red()
    });
}

async fn inspect_proxy_reachable(url: &Option<http::Uri>) {
    match &url {
        Some(url) => {
            let port = url.port_u16().unwrap_or_else(|| match url.scheme_str() {
                Some("https") => 443,
                _ => 80
            });

            let host = url.host().unwrap_or_default();
            let socket_addr = format!("{}:{}", host, port);
        
            let reachable =match tokio::time::timeout(Duration::from_secs(3), tokio::net::TcpStream::connect(socket_addr)).await {
                Ok(Ok(_)) => true,
                _ => false
            };

            println!("{} {}: {} - {}", match reachable { 
                true => green_check(),
                false => red_cross()
            }, "Proxy".grey(), url.to_string(), match reachable {
                true => "reachable".green(),
                false => "unreachable".red()
            });
        },
        None => println!("{} {}: {}", green_check(), "Proxy".grey(), "<not set>".light_grey())
    }
}

fn inspect_cluster(name: String, cluster: Option<Cluster>) {
    match cluster {
        Some(cluster) => {
            if let Some(cert) = &cluster.certificate_authority {
                let exists = PathBuf::from(cert).exists();
                println!("{} {} {} - {}", match exists {
                    true => green_check(),
                    false => red_cross()
                }, "Cluster Certificate:".grey(), cert, match exists {
                    true => "exists".red(),
                    false => "file not found".red()
                });
            } else if let Some(cert) = &cluster.certificate_authority_data {
                println!("{} {} <REDACTED len={}>", green_check(), "Cluster Certificate Data:".grey(), cert.len());
            } else {
                println!("{} {} {}", red_cross(), "Cluster Certificate:".grey(), "<not set>".light_grey());
            }
        },
        None => println!("{} {}", red_cross(), format!("Cluster {} not found", name).red())
    }
}

fn inspect_authinfo(user: String, info: Option<AuthInfo>) {
    match info {
        Some(info) => {
            if let Some(exec) = &info.exec  {
                println!("{} {} {} {}", green_check(), "Auth Exec:".grey(), exec.command.clone().unwrap_or_default(), exec.args.clone().unwrap_or_default().join(" "));
            }
            if let Some(token) = &info.token {
                println!("{} {} <REDACTED len={}>", green_check(), "Auth Token:".grey(), token.expose_secret().len());
            }
            if let Some(username) = &info.username {
                println!("{} {} {}", green_check(), "Auth Username:".grey(), username);
                let empty_pwd = SecretString::from(String::from(""));
                let password = info.password.clone().unwrap_or(empty_pwd);
                println!("{} {} <REDACTED len={}>", green_check(), "Auth Password:".grey(), &password.expose_secret().len());
            }
            if let Some(cert) = &info.client_certificate {
                let exists = PathBuf::from(cert).exists();
                println!("{} {} {} - {}", match exists {
                    true => green_check(),
                    false => red_cross()
                }, "Auth Client Certificate:".grey(), cert, match exists {
                    true => "exists".green(),
                    false => "file not found".red()
                });
            }
            if let Some(cert) = &info.client_certificate_data {
                println!("{} {} <REDACTED len={}>", green_check(), "Auth Client Certificate Data:".grey(), cert.len());
            }
        },
        None => println!("{} {}", red_cross(), format!("User {} not found", user).red())
    }
}

fn find_context(config: &Kubeconfig, context: &String) -> (String, Option<Cluster>, String, Option<AuthInfo>) {
    let current_context = config
        .contexts
        .iter()
        .find(|named_context| &named_context.name == context)
        .and_then(|named_context| named_context.context.clone())
        .unwrap_or_default();

    let cluster = config
        .clusters
        .iter()
        .find(|x| &x.name == &current_context.cluster)
        .and_then(|x| x.cluster.clone());

    let auth_info = config
        .auth_infos
        .iter()
        .find(|x| &x.name == &current_context.user)
        .and_then(|x| x.auth_info.clone());

    return (current_context.cluster, cluster, current_context.user, auth_info);
}