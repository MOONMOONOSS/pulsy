const ConsumerEventEmitter = require('./emitter');

function run() {
  const emitter = new ConsumerEventEmitter();

  emitter.on('tick', ({ msg }) => {
    console.log(msg);
  });

  return new Promise(resolve => setTimeout(resolve, 60000)).then(() => emitter.close());
}

if (process.env.NODE_ENV !== 'test')
  run();

module.exports = run;
