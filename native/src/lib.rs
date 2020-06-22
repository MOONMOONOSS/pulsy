#[macro_use]
extern crate serde;

use futures::TryStreamExt;
use neon::prelude::*;
use neon::{declare_types, register_module};
use neon::types::{JsFunction, JsUndefined, JsValue};
use pulsar::{
  DeserializeMessage, Error as PulsarError,
  Pulsar, TokioExecutor,message::Payload,
  message::proto::command_subscribe::SubType,
};
use std::{
  sync::{
    Arc, Mutex,
    mpsc::{self, RecvTimeoutError, TryRecvError},
  },
  thread, time::Duration,
};
use tokio::runtime::Runtime;

pub enum Event {
  Tick { msg: String },
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PulsarMessage {
  #[serde(rename(serialize = "type", deserialize = "subject"))]
  pub subject: Option<String>,
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

fn event_thread(
  address: Handle<JsString>,
  uuid: Handle<JsString>,
  shutdown_rx: mpsc::Receiver<()>
) -> mpsc::Receiver<Event> {
  let (tx, events_rx) = mpsc::channel();
  let address = address.value();
  let uuid = uuid.value();

  thread::spawn(move || {
    let mut runtime = Runtime::new().unwrap();

    let pulsar: Result<
      Pulsar<TokioExecutor>,
      PulsarError,
    > = runtime.block_on(Pulsar::builder(address)
      .build()
    );

    // Yes I really have to do this.
    let pulsar = pulsar.unwrap();

    // Create the consumer
    let mut consumer = runtime.block_on(pulsar
      .consumer()
      .with_topic("non-persistent://moonmoon/lottery/drawings")
      .with_consumer_name(&uuid)
      .with_subscription_type(SubType::Exclusive)
      .with_subscription(&uuid)
      .build()
    ).unwrap();

    loop {
      match shutdown_rx.try_recv() {
        Ok(_) | Err(TryRecvError::Disconnected) => {
          break;
        },
        Err(TryRecvError::Empty) => {},
      }

      match runtime.block_on(consumer.try_next()).unwrap() {
        Some::<pulsar::consumer::Message<PulsarMessage>>(msg) => {
          let data = msg.deserialize().unwrap();

          tx.send(Event::Tick { msg: serde_json::to_string(&data).unwrap() }).expect("Send failed");
        },
        None => {},
      }
    }
  });

  events_rx
}

pub struct EventEmitterTask(Arc<Mutex<mpsc::Receiver<Event>>>);

impl Task for EventEmitterTask {
  type Output = Option<Event>;
  type Error = String;
  type JsEvent = JsValue;

  fn perform(&self) -> Result<Self::Output, Self::Error> {
    let rx = self
      .0
      .lock()
      .map_err(|_| "Could not obtain lock on receiver".to_string())?;

    match rx.recv_timeout(Duration::from_millis(100)) {
      Ok(event) => Ok(Some(event)),
      Err(RecvTimeoutError::Timeout) => Ok(None),
      Err(RecvTimeoutError::Disconnected) => Err("Failed to receive event".to_string()),
    }
  }

  fn complete(
    self,
    mut cx: TaskContext,
    event: Result<Self::Output, Self::Error>,
  ) -> JsResult<Self::JsEvent> {
    let event = event.or_else(|err| cx.throw_error(&err.to_string()))?;

    let event = match event {
      Some(event) => event,
      None => return Ok(JsUndefined::new().upcast()),
    };

    let o = cx.empty_object();

    match event {
      Event::Tick { msg } => {
        let event_name = cx.string("tick");
        let event_msg = cx.string(msg);

        o.set(&mut cx, "event", event_name)?;
        o.set(&mut cx, "payload", event_msg)?;
      }
    }

    Ok(o.upcast())
  }
}

pub struct PulsarConsumer {
  events: Arc<Mutex<mpsc::Receiver<Event>>>,
  shutdown: mpsc::Sender<()>,
}

declare_types! {
  pub class JsConsumer for PulsarConsumer {
    init(mut cx) {
      // Get strings from JS Land
      let address: Handle<JsString> = cx.argument::<JsString>(0)?;
      let uuid: Handle<JsString> = cx.argument::<JsString>(1)?;
      let (shutdown, shutdown_rx) = mpsc::channel();

      // Start worker thread
      let rx = event_thread(address, uuid, shutdown_rx);

      Ok(PulsarConsumer {
        events: Arc::new(Mutex::new(rx)),
        shutdown,
      })
    }

    method poll(mut cx) {
      let cb = cx.argument::<JsFunction>(0)?;
      let this = cx.this();

      let events = cx.borrow(&this, |emitter| Arc::clone(&emitter.events));
      let emitter = EventEmitterTask(events);

      emitter.schedule(cb);

      Ok(JsUndefined::new().upcast())
    }

    method close(mut cx) {
      let this = cx.this();

      cx.borrow(&this, |emitter| emitter.shutdown.send(()))
        .or_else(|err| cx.throw_error(&err.to_string()))?;
      
      Ok(JsUndefined::new().upcast())
    }
  }
}

register_module!(mut cx, {
  cx.export_class::<JsConsumer>("Consumer")?;

  Ok(())
});
