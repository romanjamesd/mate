use crate::messages::SignedEnvelope;
use tokio::io::{AsyncRead, AsyncWrite};
use anyhow::Result;

pub struct FramedMessage;

impl FramedMessage {
    pub async fn write_message(
        writer: &mut (impl AsyncWrite + Unpin),
        envelope: &SignedEnvelope
    ) -> Result<()> {
        // Implementation placeholder
        todo!()
    }
    
    pub async fn read_message(
        reader: &mut (impl AsyncRead + Unpin)
    ) -> Result<SignedEnvelope> {
        // Implementation placeholder
        todo!()
    }
}
