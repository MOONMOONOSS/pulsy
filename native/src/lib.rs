#[macro_use]
extern crate serde;

use neon::prelude::*;
use neon::{declare_types, register_module};
use pulsar::{
  Consumer as Pconsumer, DeserializeMessage, Error as PulsarError,
  Pulsar, TokioExecutor, message::Payload,
  message::proto::command_subscribe::SubType,
};
use tokio::runtime::Runtime;

#[derive(Serialize, Deserialize)]
pub struct PulsarMessage {
  #[serde(rename(serialize = "type", deserialize = "subject"))]
  pub subject: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub uuid: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub msg: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none", rename(serialize = "serverIp", deserialize = "server_ip"))]
  pub server_ip: Option<String>,
}

impl DeserializeMessage for PulsarMessage {
  type Output = Result<PulsarMessage, serde_json::Error>;

  fn deserialize_message(payload: &Payload) -> Self::Output {
    serde_json::from_slice(&payload.data)
  }
}

pub struct PulsarConsumer {
  runtime: Runtime,
  pulsar: Pulsar<TokioExecutor>,
  consumer: Pconsumer<PulsarMessage>,
}

declare_types! {
  pub class JsConsumer for PulsarConsumer {
    init(mut cx) {
      // Get strings from JS Land
      let address: Handle<JsString> = cx.argument::<JsString>(0)?;
      let uuid: Handle<JsString> = cx.argument::<JsString>(1)?;

      println!("{}", address.value());
      println!("{}", uuid.value());

      // Spawn the Tokio runtime
      let mut runtime = Runtime::new().unwrap();

      let pulsar: Result<
        Pulsar<TokioExecutor>,
        PulsarError,
      > = runtime.block_on(Pulsar::builder(address.value()).build());

      // Yes I really have to do this.
      let pulsar = pulsar.unwrap();

      // Create the consumer
      let mut consumer = runtime.block_on(pulsar
        .consumer()
        .with_topic("non-persistent://moonmoon/lottery/drawings")
        .with_consumer_name(uuid.value())
        .with_subscription_type(SubType::Exclusive)
        .with_subscription(uuid.value())
        .build()
      ).unwrap();

      Ok(PulsarConsumer {
        runtime,
        pulsar,
        consumer,
      })
    }
  }
}

register_module!(mut cx, {
  cx.export_class::<JsConsumer>("Consumer")
});
