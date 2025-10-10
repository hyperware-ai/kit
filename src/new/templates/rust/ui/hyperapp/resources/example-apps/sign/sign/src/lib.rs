use anyhow::anyhow;

use hyperware_process_lib::logging::{init_logging, Level};
use hyperware_process_lib::net::{NetAction, NetResponse};
use hyperware_process_lib::{last_blob, our, LazyLoadBlob, Request};

use hyperware_app_common::{send_rmp, source};
use hyperprocess_macro::hyperprocess;

#[derive(Default, Debug, serde::Serialize, serde::Deserialize)]
struct SignState {}

async fn sign(message: Vec<u8>) -> anyhow::Result<Vec<u8>> {
    let message = make_message(&message);
    let body = rmp_serde::to_vec(&NetAction::Sign)?;

    let req = Request::to(("our", "net", "distro", "sys"))
        .expects_response(5)
        .blob(LazyLoadBlob {
            mime: None,
            bytes: message,
        })
        .body(body);

    let _resp: NetResponse = send_rmp(req).await?;

    let Some(signature) = last_blob() else {
        return Err(anyhow!("no blob"));
    };

    Ok(signature.bytes)
}

async fn verify(message: Vec<u8>, signature: Vec<u8>) -> anyhow::Result<bool> {
    let message = make_message(&message);
    let body = rmp_serde::to_vec(&NetAction::Verify {
        from: our(),
        signature,
    })?;

    let req = Request::to(("our", "net", "distro", "sys"))
        .expects_response(5)
        .blob(LazyLoadBlob {
            mime: None,
            bytes: message,
        })
        .body(body);

    let resp: NetResponse = send_rmp(req).await?;

    match resp {
        NetResponse::Verified(is_good) => {
            Ok(is_good)
        }
        _ => Err(anyhow!("weird response")),
    }
}

/// net:distro:sys prepends the message to sign with the sender of the request
///
/// since any sign requests passed through sign:sign:sys will look to net:distro:sys
///  like they come from sign:sign:sys, we additionally prepend the message with
///  source here
///
/// so final message to be signed looks like
///
/// [sign-address, source, bytes].concat()
fn make_message(bytes: &Vec<u8>) -> Vec<u8> {
    [source().to_string().as_bytes(), &bytes].concat()
}

#[hyperprocess(
    name = "sign",
    ui = None,
    endpoints = vec![],
    save_config = SaveOptions::Never,
    wit_world = "sign-sys-v0",
)]
impl SignState {
    #[init]
    async fn init(&mut self) {
        init_logging(Level::DEBUG, Level::INFO, None, None, None).unwrap();
    }

    #[local]
    async fn sign(&mut self, message: Vec<u8>) -> Result<Vec<u8>, String> {
        sign(message).await.map_err(|e| e.to_string())
    }

    #[local]
    async fn verify(&mut self, message: Vec<u8>, signature: Vec<u8>) -> Result<bool, String> {
        verify(message, signature).await.map_err(|e| e.to_string())
    }
}
