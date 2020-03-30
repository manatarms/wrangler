mod server;
mod setup;

use server::serve;

use crate::commands;
use crate::commands::dev::{socket, ServerConfig};
use crate::settings::global_user::GlobalUser;
use crate::settings::toml::{DeployConfig, Target};

use http::Request;
use tokio::runtime::Runtime as TokioRuntime;

pub fn dev(
    target: Target,
    deploy_config: DeployConfig,
    user: GlobalUser,
    server_config: ServerConfig,
) -> Result<(), failure::Error> {
    commands::build(&target)?;
    let init = setup::init(&deploy_config, &user)?;
    let mut target = target.clone();
    let host = match deploy_config {
        DeployConfig::Zoned(_) => init.exchange_host.clone(),
        DeployConfig::Zoneless(_) => {
            let namespaces: Vec<&str> = init.exchange_host.split('.').collect();
            let subdomain = namespaces[1];
            format!("{}.{}.workers.dev", target.name, subdomain)
        }
    };

    // TODO: replace asset manifest parameter
    let preview_token =
        setup::upload(&mut target, None, &deploy_config, &user, init.preview_token)?;
    // TODO: ws://{your_zone}/cdn-cgi/workers/preview/inspector
    // also need to send init.ws_token as cf-workers-preview-token on init
    let socket_url = format!(
        "ws://{}/cdn-cgi/workers/preview/inspector",
        init.exchange_host
    );
    println!("{}", socket_url);
    let socket_request = Request::builder()
        .uri(socket_url)
        .header("cf-workers-preview-token", init.ws_token)
        .body(())
        .unwrap();
    let devtools_listener = socket::listen(socket_request);

    let server = serve(server_config, preview_token, host);

    let runners = futures::future::join(devtools_listener, server);

    let mut runtime = TokioRuntime::new()?;
    runtime.block_on(async {
        let (devtools_listener, server) = runners.await;
        devtools_listener?;
        server
    })
}
