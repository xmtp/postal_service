mod integration_util;

use anyhow::Error;

use ethers::signers::LocalWallet;
use lib_didethresolver::{
    did_registry::RegistrySignerExt,
    types::{DidUrl, KeyEncoding, XmtpAttribute, XmtpKeyPurpose},
};
use xps_gateway::rpc::XpsClient;

use ethers::types::U256;
use gateway_types::Message;

use integration_util::*;

#[tokio::test]
async fn test_say_hello() -> Result<(), Error> {
    with_xps_client(None, |client, _, _, _| async move {
        let result = client.status().await?;
        assert_eq!(result, "OK");
        Ok(())
    })
    .await
}

#[tokio::test]
async fn test_fail_send_message() -> Result<(), Error> {
    with_xps_client(None, |client, _, _, _| async move {
        let message = Message {
            conversation_id: (b"abcdefg").to_vec(),
            payload: (b"Hello World").to_vec(),
            v: vec![],
            r: vec![],
            s: vec![],
        };
        let result = client.send_message(message).await;
        assert!(result.is_err());
        Ok(())
    })
    .await
}

#[tokio::test]
async fn test_revoke_installation() -> Result<(), Error> {
    with_xps_client(None, |client, context, resolver, anvil| async move {
        let wallet: LocalWallet = anvil.keys()[3].clone().into();
        let me = get_user(&anvil, 3).await;
        let name = *b"xmtp/installation/hex           ";
        let value = b"02b97c30de767f084ce3080168ee293053ba33b235d7116a3263d29f1450936b71";
        let validity = U256::from(604_800);
        let signature = wallet
            .sign_attribute(&context.registry, name, value.to_vec(), validity)
            .await?;

        let attr = context.registry.set_attribute_signed(
            me.address(),
            signature.v.try_into().unwrap(),
            signature.r.into(),
            signature.s.into(),
            name,
            value.into(),
            validity,
        );
        attr.send().await?.await?;

        let doc = resolver
            .resolve_did(me.address(), None)
            .await
            .unwrap()
            .document;
        assert_eq!(
            doc.verification_method[1].id,
            DidUrl::parse(format!(
                "did:ethr:0x{}?meta=installation#xmtp-0",
                hex::encode(me.address())
            ))
            .unwrap()
        );

        let signature = wallet
            .sign_revoke_attribute(&context.registry, name, value.to_vec())
            .await?;

        let attribute = XmtpAttribute {
            purpose: XmtpKeyPurpose::Installation,
            encoding: KeyEncoding::Hex,
        };

        client
            .revoke_installation(
                format!("0x{}", hex::encode(me.address())),
                attribute,
                value.to_vec(),
                signature,
            )
            .await?;

        let doc = resolver
            .resolve_did(me.address(), None)
            .await
            .unwrap()
            .document;

        log::debug!("{}", serde_json::to_string_pretty(&doc).unwrap());

        assert_eq!(
            doc.verification_method[0].id,
            DidUrl::parse(format!(
                "did:ethr:0x{}#controller",
                hex::encode(me.address())
            ))
            .unwrap()
        );
        assert_eq!(doc.verification_method.len(), 1);

        Ok(())
    })
    .await
}
