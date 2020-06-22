const ConsumerEventEmitter = require('./emitter');

function run() {
  const emitter = new ConsumerEventEmitter("pulsar://panel.moonmoon.live:6650", "cum");

  emitter.on('tick', ({ msg }) => {
    console.log(msg);
  });

  return new Promise(resolve => setTimeout(resolve, 15000)).then(() => emitter.close());
}

if (process.env.NODE_ENV !== 'test')
  run();

module.exports = ConsumerEventEmitter;
